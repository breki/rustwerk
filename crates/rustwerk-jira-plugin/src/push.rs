//! Per-task push orchestration: dispatch a [`TaskDto`]
//! through the create / probe-then-update / recreate
//! ladder, build the resulting [`TaskPushResult`], and
//! delegate workflow transitions to [`crate::transition`].
//!
//! HTTP plumbing lives in [`crate::jira_client`]; payload
//! construction lives in [`crate::mapping`]. This module
//! owns the *flow* — when to create vs. update, how to
//! read prior state, how to fold per-task outcomes into
//! the final result.

use rustwerk_plugin_api::{PluginResult, TaskDto, TaskPushResult};
use serde_json::json;

use crate::config::JiraConfig;
use crate::jira_client::{
    create_issue, get_issue, parse_created_issue, update_issue, CreatedIssue,
    HttpClient, IssueKey, JiraOpOutcome, ParseIssueError, ProbeOutcome,
};
use crate::mapping::{self, IssuePayload};
use crate::transition::{append_warnings, maybe_transition_after_write};
use crate::warnings::MappingWarning;

/// Abstraction over "what is the current wall-clock UTC
/// instant". The jira plugin needs to stamp
/// `last_pushed_at` into per-task plugin state;
/// returning a typed [`chrono::DateTime<chrono::Utc>`]
/// rather than a preformatted `String` keeps the
/// allocation / format choice in one place
/// ([`build_created_state`]) and avoids forcing a fresh
/// `String` allocation on every successful push.
pub(crate) trait Clock {
    /// Return the current UTC instant.
    fn now(&self) -> chrono::DateTime<chrono::Utc>;
}

/// Production [`Clock`] backed by the OS wall clock via
/// `chrono::Utc::now`.
pub(crate) struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }
}

/// Walk every task, attempt to create its Jira issue,
/// collect per-task results, and aggregate into a
/// [`PluginResult`].
pub(crate) fn push_all<C: HttpClient, K: Clock>(
    http: &C,
    clock: &K,
    cfg: &JiraConfig,
    tasks: &[TaskDto],
) -> PluginResult {
    let results: Vec<TaskPushResult> =
        tasks.iter().map(|t| push_one(http, clock, cfg, t)).collect();
    let all_ok = results.iter().all(|r| r.success);
    let message = if all_ok {
        format!("{} task(s) pushed to Jira", results.len())
    } else {
        let failed = results.iter().filter(|r| !r.success).count();
        format!("{failed} of {} task(s) failed", results.len())
    };
    PluginResult {
        success: all_ok,
        message,
        task_results: Some(results),
    }
}

/// Push a single task. Dispatches on whether the task
/// already carries per-plugin state from a previous
/// push:
///
/// | incoming `plugin_state.jira.key` | probe result       | action                                 |
/// |----------------------------------|--------------------|----------------------------------------|
/// | `None`                           | —                  | `POST /issue` (create)                 |
/// | `Some(key)`                      | 2xx                | `PUT /issue/{key}` (update)            |
/// | `Some(key)`                      | 404 after fallback | `POST /issue` (recreate + overwrite)   |
///
/// HTTP-client errors and non-2xx Jira responses turn
/// into a failed [`TaskPushResult`] rather than
/// propagating, so one bad task does not abort the batch.
fn push_one<C: HttpClient, K: Clock>(
    http: &C,
    clock: &K,
    cfg: &JiraConfig,
    task: &TaskDto,
) -> TaskPushResult {
    let IssuePayload {
        body,
        warnings: mut all_warnings,
    } = mapping::build_issue_payload(task, cfg);
    let body_str = body.to_string();
    let result = match existing_issue_key_validated(task) {
        ExistingKey::None => push_one_create(
            http,
            clock,
            cfg,
            task,
            &body_str,
            &mut all_warnings,
        ),
        ExistingKey::Valid(key) => push_one_update(
            http,
            clock,
            cfg,
            task,
            &key,
            &body_str,
            &mut all_warnings,
        ),
        ExistingKey::Invalid(raw) => TaskPushResult::fail(
            task.id.clone(),
            format!(
                "stored Jira key {raw:?} is not a valid issue key — refusing to \
                 splice it into a URL; fix or clear plugin_state.jira.key and \
                 re-push"
            ),
        ),
    };
    append_warnings(result, &all_warnings)
}

/// Outcome of reading `plugin_state.jira.key`.
///
/// - `None` → no prior push, go create.
/// - `Valid(IssueKey)` → prior push, go probe+update.
/// - `Invalid(raw)` → *something* was stored but it does
///   not match Jira's issue-key grammar. Fail the task
///   loudly rather than silently recreating: a poisoned
///   `project.json` must not be able to coerce a
///   duplicate issue by failing a validation check
///   (RT-121).
#[derive(Debug)]
pub(crate) enum ExistingKey {
    None,
    Valid(IssueKey),
    Invalid(String),
}

/// Extract and validate the Jira issue key stored under
/// `plugin_state.jira.key` after a previous successful
/// push. Empty state or a non-string `key` is treated as
/// "never pushed" (create-path); a present-but-malformed
/// string is treated as a poisoned / corrupted state
/// entry and surfaced as `Invalid` so the caller can
/// fail the task.
pub(crate) fn existing_issue_key_validated(task: &TaskDto) -> ExistingKey {
    let Some(state) = task.plugin_state.as_ref() else {
        return ExistingKey::None;
    };
    let Some(key_value) = state.get("key") else {
        return ExistingKey::None;
    };
    let Some(key_str) = key_value.as_str() else {
        return ExistingKey::None;
    };
    match IssueKey::parse(key_str) {
        Some(valid) => ExistingKey::Valid(valid),
        None => ExistingKey::Invalid(key_str.to_string()),
    }
}

/// Create path. Exists as a separate function so the
/// recreate-on-404 branch inside `push_one_update` can
/// reuse the exact same success / parse / warning
/// semantics. `warnings` accumulates non-fatal hints
/// across both mapping and transition steps — the
/// dispatcher renders them all at the end.
fn push_one_create<C: HttpClient, K: Clock>(
    http: &C,
    clock: &K,
    cfg: &JiraConfig,
    task: &TaskDto,
    body: &str,
    warnings: &mut Vec<MappingWarning>,
) -> TaskPushResult {
    match create_issue(http, cfg, body) {
        Ok(outcome) => {
            let base = task_result_from_create_outcome(task, clock, &outcome);
            if base.success {
                maybe_transition_after_write(http, cfg, task, base, None, warnings)
            } else {
                base
            }
        }
        Err(e) => TaskPushResult::fail(task.id.clone(), e.to_string()),
    }
}

/// Update path: probe via `GET /issue/{key}`, dispatch
/// on the probe outcome.
///
/// - `Exists` → `PUT` to update.
/// - `MissingConfirmed` → recreate (safe: direct URL
///   authoritatively said the issue is gone).
/// - `MissingAmbiguous` → fail the task. Direct-URL
///   401 plus gateway 404 is *not* proof of absence,
///   and silently recreating would duplicate a live
///   issue whose read scope the current token cannot
///   see (RT-122).
/// - `OtherStatus` → fail the task with the response
///   body so the operator can diagnose.
fn push_one_update<C: HttpClient, K: Clock>(
    http: &C,
    clock: &K,
    cfg: &JiraConfig,
    task: &TaskDto,
    key: &IssueKey,
    body: &str,
    warnings: &mut Vec<MappingWarning>,
) -> TaskPushResult {
    let probe = match get_issue(http, cfg, key) {
        Ok(o) => o,
        Err(e) => return TaskPushResult::fail(task.id.clone(), e.to_string()),
    };
    let prior_status = task
        .plugin_state
        .as_ref()
        .and_then(|s| s.get("last_status"))
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    match probe {
        ProbeOutcome::Exists {
            used_gateway: probe_used_gateway,
            ..
        } => match update_issue(http, cfg, key, body) {
            Ok(outcome) => {
                let base = task_result_from_update_outcome(
                    task,
                    clock,
                    key,
                    &outcome,
                    probe_used_gateway,
                );
                if base.success {
                    maybe_transition_after_write(
                        http,
                        cfg,
                        task,
                        base,
                        prior_status.as_deref(),
                        warnings,
                    )
                } else {
                    base
                }
            }
            Err(e) => TaskPushResult::fail(task.id.clone(), e.to_string()),
        },
        ProbeOutcome::MissingConfirmed { .. } => {
            // Direct + gateway both 404 → issue is truly
            // gone. Recreate and overwrite state. The
            // recreate path reuses the same warnings
            // accumulator so mapping + transition
            // warnings from *this* push still surface.
            push_one_create(http, clock, cfg, task, body, warnings)
        }
        ProbeOutcome::MissingAmbiguous => TaskPushResult::fail(
            task.id.clone(),
            format!(
                "Jira probe of issue {key} was ambiguous (direct HTTP 401, \
                 gateway HTTP 404) — refusing to recreate: the issue may be \
                 alive but unreadable with this token. Verify token scope, \
                 then retry"
            ),
        ),
        ProbeOutcome::OtherStatus { status, body, .. } => TaskPushResult::fail(
            task.id.clone(),
            format!("Jira probe of issue {key} returned HTTP {status}: {body}"),
        ),
    }
}

/// Translate a low-level [`JiraOpOutcome`] from a create
/// call into the per-task public result DTO. On a 2xx
/// whose body parses into a [`CreatedIssue`], attaches
/// the external key and the `plugin_state_update` blob
/// the host persists under `plugin_state.jira`. On a 2xx
/// whose body fails to parse for any reason **other than**
/// being empty, appends a visible warning to the message
/// — a silent skip would let a malformed Jira response
/// cause unbounded duplicate issues on repeat pushes.
pub(crate) fn task_result_from_create_outcome<K: Clock>(
    task: &TaskDto,
    clock: &K,
    outcome: &JiraOpOutcome,
) -> TaskPushResult {
    if (200..300).contains(&outcome.status) {
        let mut message = if outcome.used_gateway {
            format!("created (HTTP {}, via gateway)", outcome.status)
        } else {
            format!("created (HTTP {})", outcome.status)
        };
        match parse_created_issue(&outcome.body) {
            Ok(created) => {
                let state = build_created_state(&created, clock.now());
                return TaskPushResult::ok(task.id.clone(), message)
                    .with_external_key(created.key.as_str())
                    .with_plugin_state_update(state);
            }
            Err(ParseIssueError::EmptyBody) => {
                // 204 / no-body success — nothing to
                // anchor state against, silent skip.
            }
            Err(e) => {
                message = format!(
                    "{message} (WARNING: {e}; plugin state not recorded — \
                     next push may create a duplicate Jira issue)"
                );
            }
        }
        TaskPushResult::ok(task.id.clone(), message)
    } else {
        TaskPushResult::fail(
            task.id.clone(),
            format!("Jira returned HTTP {}: {}", outcome.status, outcome.body),
        )
    }
}

/// Translate a low-level [`JiraOpOutcome`] from an update
/// call into the per-task public result DTO. Jira
/// typically returns `204 No Content` for a successful
/// `PUT`, so we cannot re-derive the state blob from the
/// response body — instead we reuse the previously
/// stored `key`/`self` and only refresh `last_pushed_at`.
/// On a non-2xx response the stored state is left
/// untouched (`plugin_state_update: None`) so a transient
/// update failure does not discard the idempotency anchor.
pub(crate) fn task_result_from_update_outcome<K: Clock>(
    task: &TaskDto,
    clock: &K,
    key: &IssueKey,
    outcome: &JiraOpOutcome,
    probe_used_gateway: bool,
) -> TaskPushResult {
    if (200..300).contains(&outcome.status) {
        // Gateway flag reflects the *whole* push, not
        // just the PUT, so the operator-facing message
        // stays honest when only the probe (or only the
        // PUT) had to fall back (RT-125).
        let via_gateway = outcome.used_gateway || probe_used_gateway;
        let message = if via_gateway {
            format!("updated (HTTP {}, via gateway)", outcome.status)
        } else {
            format!("updated (HTTP {})", outcome.status)
        };
        let mut result = TaskPushResult::ok(task.id.clone(), message)
            .with_external_key(key.as_str());
        if let Some(refreshed) =
            build_refreshed_state(task.plugin_state.as_ref(), clock.now())
        {
            result = result.with_plugin_state_update(refreshed);
        }
        result
    } else {
        TaskPushResult::fail(
            task.id.clone(),
            format!(
                "Jira update of issue {key} returned HTTP {}: {}",
                outcome.status, outcome.body
            ),
        )
    }
}

/// Build the `plugin_state.jira` blob persisted under the
/// task after a successful **create**. Fields are additive
/// — future iterations (`last_hash`, `last_response_etag`,
/// …) can extend without breaking the host, which treats
/// the value opaquely. `last_pushed_at` is formatted
/// ISO-8601 UTC with seconds precision so the stored
/// timestamp stays diff-stable across hosts with differing
/// default serializers.
pub(crate) fn build_created_state(
    created: &CreatedIssue,
    now: chrono::DateTime<chrono::Utc>,
) -> serde_json::Value {
    json!({
        "key": created.key.as_str(),
        "self": created.self_url,
        "last_pushed_at": format_last_pushed_at(now),
    })
}

/// Build the `plugin_state.jira` blob persisted under the
/// task after a successful **update**. `PUT /issue/{key}`
/// returns 204 so we carry over the previously stored
/// blob verbatim — only mutating `last_pushed_at` — which
/// preserves any additive fields a future plugin version
/// may have recorded (`last_hash`, `last_response_etag`,
/// …). Closes RT-123: wholesale reconstruction dropped
/// those fields on every successful update.
///
/// Returns `None` if the stored state is missing the
/// minimum expected `key`/`self` string fields — in that
/// case the caller leaves `plugin_state_update` at `None`
/// rather than writing a broken blob.
fn build_refreshed_state(
    existing: Option<&serde_json::Value>,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<serde_json::Value> {
    let existing_obj = existing?.as_object()?;
    // Integrity check: the idempotency-anchor fields must
    // be present and string-typed. A malformed stored
    // blob is not refreshed; the host keeps the prior
    // state intact.
    existing_obj.get("key")?.as_str()?;
    existing_obj.get("self")?.as_str()?;
    let mut refreshed = existing_obj.clone();
    refreshed.insert(
        "last_pushed_at".into(),
        serde_json::Value::String(format_last_pushed_at(now)),
    );
    Some(serde_json::Value::Object(refreshed))
}

/// Single format authority for the `last_pushed_at` wire
/// value so create and refresh paths stay byte-identical.
pub(crate) fn format_last_pushed_at(now: chrono::DateTime<chrono::Utc>) -> String {
    now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

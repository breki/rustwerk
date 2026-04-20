//! Jira workflow transition + `last_status` bookkeeping.
//!
//! Status changes cannot be sent in the create/update
//! body — Jira rejects `status` there. Instead they go
//! through `POST /rest/api/3/issue/{key}/transitions`
//! with a transition ID. This module wraps that call,
//! reconciles the resulting state blob
//! (`plugin_state.jira.last_status`) so repeated pushes
//! stay quiet, and accumulates non-fatal transition
//! warnings alongside mapping warnings.

use rustwerk_plugin_api::{TaskDto, TaskPushResult};
use serde_json::json;

use crate::config::JiraConfig;
use crate::jira_client::{transition, HttpClient, IssueKey};
use crate::warnings::MappingWarning;

/// If `status_map` has a transition ID for the task's
/// status AND the prior `last_status` (`None` on create)
/// differs from the current wire name, fire the transition
/// and splice the outcome into the result.
///
/// Outcomes:
/// - transition 2xx → append `" + transitioned (HTTP …)"`
///   to the message; set `last_status` in the state blob.
/// - transition non-2xx or transport error → push a
///   typed [`MappingWarning`] onto `warnings`; leave
///   `last_status` unset so the next push retries.
/// - no mapping OR status unchanged since `last_status` →
///   no HTTP call; record `last_status = current` in state
///   so a no-op stays a no-op next time.
pub(crate) fn maybe_transition_after_write<C: HttpClient>(
    http: &C,
    cfg: &JiraConfig,
    task: &TaskDto,
    result: TaskPushResult,
    prior_status: Option<&str>,
    warnings: &mut Vec<MappingWarning>,
) -> TaskPushResult {
    let current = task.status.as_wire();
    // RT-X1: gate on external_key, not plugin_state_update.
    // A successful update whose stored state was malformed
    // arrives here with `plugin_state_update = None` — we
    // still want to fire the transition because the key is
    // known-good and the PUT succeeded.
    let Some(ref key_str) = result.external_key else {
        return result;
    };
    let Some(key) = IssueKey::parse(key_str) else {
        return result;
    };
    if prior_status == Some(current) {
        // No-op — already aligned. Still record last_status
        // so future pushes can short-circuit.
        return record_last_status(result, current);
    }
    let Some(transition_id) = cfg.transition_id_for_status(current) else {
        // Status not configured for transitions. Record
        // last_status anyway so a later change triggers an
        // explicit transition attempt.
        return record_last_status(result, current);
    };
    match transition(http, cfg, &key, transition_id) {
        Ok(outcome) if (200..300).contains(&outcome.status) => {
            let suffix = if outcome.used_gateway {
                format!(
                    "+ transitioned to {transition_id} (HTTP {}, via gateway)",
                    outcome.status
                )
            } else {
                format!("+ transitioned to {transition_id} (HTTP {})", outcome.status)
            };
            record_last_status(result.with_appended_message(&suffix), current)
        }
        Ok(outcome) => {
            warnings.push(MappingWarning::TransitionHttp {
                transition_id: transition_id.to_owned(),
                status: outcome.status,
                body: outcome.body,
            });
            result
        }
        Err(e) => {
            warnings.push(MappingWarning::TransitionTransport {
                transition_id: transition_id.to_owned(),
                message: e.to_string(),
            });
            result
        }
    }
}

/// Render `warnings` (if any) into a parenthesised
/// `(WARNING: …)` suffix on the task message. Each
/// warning is rendered via its [`Display`] impl. The
/// separator `" | "` is chosen because no
/// [`MappingWarning`] [`Display`] contract emits that
/// sequence, so downstream parsers can reliably re-split
/// (AQ-X6).
pub(crate) fn append_warnings(
    r: TaskPushResult,
    warnings: &[MappingWarning],
) -> TaskPushResult {
    if warnings.is_empty() {
        return r;
    }
    let rendered: Vec<String> = warnings.iter().map(ToString::to_string).collect();
    let suffix = format!("(WARNING: {})", rendered.join(" | "));
    r.with_appended_message(&suffix)
}

/// Splice `last_status` into the existing state blob
/// (or synthesize a minimal one if the caller had no
/// state update to attach — see RT-X1). Uses the
/// `with_plugin_state_update` public-API builder so
/// [`TaskPushResult`] remains the single authority over
/// state mutation (AQ-X4).
fn record_last_status(result: TaskPushResult, wire: &str) -> TaskPushResult {
    let base = result
        .plugin_state_update
        .clone()
        .unwrap_or_else(|| json!({}));
    let new_state = with_last_status(base, wire);
    result.with_plugin_state_update(new_state)
}

/// Return a copy of `state` with `last_status` inserted
/// / overwritten. The input is always an object at every
/// current call site — the plugin controls every state
/// producer — so a non-object is a real bug we want to
/// catch in tests rather than silently paper over (AQ-X3
/// / RT-X3). In release builds the function wraps the
/// non-object value in a fresh object so the idempotency
/// anchor still lands.
fn with_last_status(mut state: serde_json::Value, wire: &str) -> serde_json::Value {
    debug_assert!(
        state.is_object(),
        "plugin_state.jira must be a JSON object; got {state:?}"
    );
    if let Some(obj) = state.as_object_mut() {
        obj.insert(
            "last_status".into(),
            serde_json::Value::String(wire.into()),
        );
    } else {
        state = json!({ "last_status": wire });
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_last_status_inserts_into_existing_object() {
        let state = json!({ "key": "PROJ-1" });
        let updated = with_last_status(state, "done");
        assert_eq!(updated["key"], "PROJ-1");
        assert_eq!(updated["last_status"], "done");
    }

    #[test]
    fn with_last_status_overwrites_existing_value() {
        let state = json!({ "last_status": "todo" });
        let updated = with_last_status(state, "in_progress");
        assert_eq!(updated["last_status"], "in_progress");
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "must be a JSON object")]
    fn with_last_status_panics_on_non_object_in_debug() {
        // Production code never produces a non-object
        // state blob, so hitting this branch is a bug.
        let _ = with_last_status(json!([1, 2, 3]), "done");
    }

    #[test]
    fn append_warnings_returns_unchanged_when_empty() {
        let base = TaskPushResult::ok("A", "created (HTTP 201)");
        let out = append_warnings(base.clone(), &[]);
        assert_eq!(out.message, base.message);
    }

    #[test]
    fn append_warnings_uses_pipe_separator_and_wraps_in_parens() {
        let base = TaskPushResult::ok("A", "created (HTTP 201)");
        let warnings = vec![
            MappingWarning::UnmappedAssignee("bob@x".into()),
            MappingWarning::UnmappedPriority(9),
        ];
        let out = append_warnings(base, &warnings);
        assert!(out.message.contains("(WARNING:"));
        assert!(out.message.contains(" | "));
        assert!(out.message.contains("bob@x"));
        assert!(out.message.contains("complexity 9"));
    }
}

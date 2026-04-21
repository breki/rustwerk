//! Live-Jira smoke test (PLG-JIRA-E2E).
//!
//! This integration test exercises the real wire contract
//! end-to-end: it invokes the plugin's FFI
//! `rustwerk_plugin_push_tasks` entry point (same path the
//! host uses), hits an actual Jira Cloud instance, and
//! tears down the created issue in a Drop guard so the
//! tenant stays clean even if the test panics mid-way.
//!
//! # Why not unit tests?
//!
//! The existing `MockHttp` fake catches logic bugs, but
//! it can't catch *wiring* bugs — TLS configuration,
//! header casing, gateway-fallback cloud-id extraction,
//! ADF validator quirks, or Jira renaming a field. An
//! opt-in live test, gated on env vars, makes that check
//! reproducible without forcing CI to carry Jira
//! credentials.
//!
//! # Running
//!
//! ```bash
//! RUSTWERK_JIRA_TEST_URL=https://foo.atlassian.net \
//! RUSTWERK_JIRA_TEST_TOKEN=... \
//! RUSTWERK_JIRA_TEST_USERNAME=you@example.com \
//! RUSTWERK_JIRA_TEST_PROJECT=RUST \
//! cargo xtask test -- --ignored jira_live
//! ```
//!
//! Both tests carry `#[ignore]` so `cargo test` and
//! `cargo xtask validate` never reach Jira.

// The crate denies `unsafe_code` globally, but this test
// calls into the FFI entry points which require unsafe
// blocks around the C-string round-trip.
#![allow(unsafe_code)]

use std::env;
use std::ffi::{CStr, CString};
use std::fmt;
use std::os::raw::c_char;
use std::ptr;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rustwerk_jira_plugin::{
    rustwerk_plugin_free_string, rustwerk_plugin_push_tasks,
};

/// Credentials and target project for the live test.
/// Sourced from `RUSTWERK_JIRA_TEST_*` env vars via
/// [`live_env`]; cloned into [`TeardownGuard`] so the
/// drop path can authenticate the DELETE independently
/// of the test body's scope.
///
/// `Debug` is hand-written to redact the token — a
/// derived impl would print it verbatim and any future
/// `dbg!` / panic message would leak the credential to
/// CI logs (AQ-Z3).
#[derive(Clone)]
struct LiveEnv {
    url: String,
    token: String,
    username: String,
    project_key: String,
}

impl fmt::Debug for LiveEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LiveEnv")
            .field("url", &self.url)
            .field("token", &"***")
            .field("username", &self.username)
            .field("project_key", &self.project_key)
            .finish()
    }
}

/// Read the four required env vars. Returns `None` if any
/// is absent, empty, or whitespace-only — the caller
/// prints a skip message and returns early, so
/// `cargo test --ignored jira_live` on a dev machine
/// with no credentials produces a clean no-op rather
/// than an error.
fn live_env() -> Option<LiveEnv> {
    let url = non_empty_env("RUSTWERK_JIRA_TEST_URL")?;
    let token = non_empty_env("RUSTWERK_JIRA_TEST_TOKEN")?;
    let username = non_empty_env("RUSTWERK_JIRA_TEST_USERNAME")?;
    let project_key = non_empty_env("RUSTWERK_JIRA_TEST_PROJECT")?;
    Some(LiveEnv {
        url,
        token,
        username,
        project_key,
    })
}

fn non_empty_env(name: &str) -> Option<String> {
    // Trim guard (RT-Z2): a trailing-space token from a
    // heredoc or `.env` file would otherwise reach Jira
    // and produce a baffling 401 instead of a clean skip.
    env::var(name).ok().filter(|s| !s.trim().is_empty())
}

/// RAII guard that DELETEs the created Jira issue on
/// drop — including when the test panics (which
/// otherwise leaks an issue into the target project).
///
/// Uses `ureq` standalone rather than the plugin's
/// internal HTTP client so a bug in the plugin that
/// somehow broke DELETE wiring still gets caught.
struct TeardownGuard {
    env: LiveEnv,
    key: String,
}

impl Drop for TeardownGuard {
    fn drop(&mut self) {
        let panicking = std::thread::panicking();
        let url = format!(
            "{}/rest/api/3/issue/{}",
            self.env.url.trim_end_matches('/'),
            self.key
        );
        let auth = basic_auth(&self.env.username, &self.env.token);
        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(30))
            .build();
        let result = agent
            .delete(&url)
            .set("Authorization", &auth)
            .set("Accept", "application/json")
            .call();
        match (result, panicking) {
            (Ok(_), false) => {
                eprintln!("jira_live: deleted test issue {}", self.key);
            }
            (Ok(_), true) => {
                eprintln!(
                    "jira_live: test panicked; teardown still deleted {}",
                    self.key
                );
            }
            (Err(e), _) => {
                // RT-Z1: log the error *kind* rather than
                // full Display — future ureq variants or
                // refactors that embed auth in the URL
                // would otherwise leak the token into
                // stderr / CI logs.
                eprintln!(
                    "jira_live: WARNING failed to delete test issue {} \
                     (project may contain residue): {}",
                    self.key,
                    redact_ureq_error(&e),
                );
            }
        }
    }
}

/// Render a `ureq::Error` as a short, auth-safe string.
/// We deliberately do NOT propagate `e.to_string()` —
/// that may embed the request URL (and, in some future
/// ureq refactor, the auth header context).
fn redact_ureq_error(e: &ureq::Error) -> String {
    match e {
        ureq::Error::Status(s, _) => format!("HTTP {s}"),
        ureq::Error::Transport(t) => format!("transport ({:?})", t.kind()),
    }
}

fn basic_auth(user: &str, token: &str) -> String {
    format!("Basic {}", B64.encode(format!("{user}:{token}")))
}

/// Summary suffix that is stable within a test run and
/// globally unique across concurrent runs — process id +
/// nanos timestamp are sufficient for a free-Jira test
/// tenant's needs.
fn unique_suffix() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{}-{}", std::process::id(), nanos)
}

/// Outcome of a successful plugin push — named fields
/// self-document intent at call sites (AQ-Z1).
struct PushedIssue {
    key: String,
    raw_response: String,
}

/// Invoke the plugin's FFI push entry point with one
/// task, extract the Jira issue key as early as
/// possible, and return a [`TeardownGuard`] paired with
/// the parsed [`PushedIssue`].
///
/// **Ordering matters** (RT-Z5 / RT-Z6 / AQ-Z2): the key
/// is harvested from the JSON *before* any shape
/// assertion fires. The caller receives the guard
/// already wrapped around the created issue, so any
/// downstream panic — on a malformed response body, a
/// non-zero return code that slipped through, or a
/// deliberate `panic!` in the panic-path test — still
/// triggers teardown.
fn push_one_issue(env: &LiveEnv) -> (TeardownGuard, PushedIssue) {
    let config = serde_json::json!({
        "jira_url": env.url,
        "jira_token": env.token,
        "username": env.username,
        "project_key": env.project_key,
    })
    .to_string();
    let suffix = unique_suffix();
    let task_id = format!("E2E-{suffix}");
    let tasks = serde_json::json!([{
        "id": task_id,
        "title": format!("rustwerk live-test {suffix}"),
        "description": "Created by PLG-JIRA-E2E; deleted automatically.",
        "status": "todo",
        "dependencies": [],
        "tags": [],
    }])
    .to_string();

    let config_c = CString::new(config).expect("config JSON has no NUL");
    let tasks_c = CString::new(tasks).expect("tasks JSON has no NUL");
    let mut out: *mut c_char = ptr::null_mut();
    let code = unsafe {
        rustwerk_plugin_push_tasks(
            config_c.as_ptr(),
            tasks_c.as_ptr(),
            &raw mut out,
        )
    };
    // Harvest the response payload (if any) before any
    // assertion, so we can still look for a partially-
    // created issue key when the plugin reports failure.
    let raw_response = if out.is_null() {
        String::new()
    } else {
        let s = unsafe { CStr::from_ptr(out) }
            .to_str()
            .unwrap_or("")
            .to_owned();
        unsafe { rustwerk_plugin_free_string(out) };
        s
    };
    let parsed: Option<serde_json::Value> = if raw_response.is_empty() {
        None
    } else {
        serde_json::from_str(&raw_response).ok()
    };
    // Extract the key opportunistically — valid even on
    // partial failure — so the guard can always cover
    // the issue that actually landed in Jira.
    let key_opt = parsed
        .as_ref()
        .and_then(|v| v["task_results"][0]["external_key"].as_str())
        .map(str::to_owned);
    let guard = key_opt
        .clone()
        .map(|key| TeardownGuard {
            env: env.clone(),
            key,
        })
        .unwrap_or_else(|| {
            panic!(
                "plugin returned no external_key; code={code} \
                 response={raw_response}"
            )
        });

    // Shape assertions happen *after* the guard is
    // attached — a failure here still cleans up.
    assert_eq!(
        code, 0,
        "plugin push returned error code {code}; response={raw_response}"
    );
    let parsed =
        parsed.unwrap_or_else(|| panic!("plugin payload is not JSON: {raw_response}"));
    assert_eq!(parsed["success"], true, "push failed: {raw_response}");
    (
        guard,
        PushedIssue {
            key: key_opt.expect("key extracted above"),
            raw_response,
        },
    )
}

/// Probe an issue to see whether Jira still considers it
/// alive. Used by the panic-path test to confirm the
/// guard actually deleted the issue.
///
/// Returns `Ok(true)` for 2xx, `Ok(false)` for 404 — the
/// only two outcomes the caller cares about. Other
/// statuses (401/403/5xx) become an `Err` with a
/// classification so a rate-limit or credential
/// expiration during teardown verification doesn't
/// present as "unexpected status" (AQ-Z4).
fn issue_exists(env: &LiveEnv, key: &str) -> Result<bool, String> {
    let url = format!(
        "{}/rest/api/3/issue/{}",
        env.url.trim_end_matches('/'),
        key
    );
    let auth = basic_auth(&env.username, &env.token);
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(30))
        .build();
    match agent
        .get(&url)
        .set("Authorization", &auth)
        .set("Accept", "application/json")
        .call()
    {
        Ok(_) => Ok(true),
        Err(ureq::Error::Status(404, _)) => Ok(false),
        Err(ureq::Error::Status(s @ (401 | 403), _)) => {
            Err(format!("credentials rejected while probing {key}: HTTP {s}"))
        }
        Err(ureq::Error::Status(s, _)) if (500..600).contains(&s) => {
            Err(format!("Jira server error while probing {key}: HTTP {s}"))
        }
        Err(ureq::Error::Status(s, _)) => {
            Err(format!("unexpected HTTP {s} while probing {key}"))
        }
        Err(e) => Err(format!(
            "transport error while probing {key}: {}",
            redact_ureq_error(&e),
        )),
    }
}

#[test]
#[ignore = "live Jira credentials required; opt in with RUSTWERK_JIRA_TEST_* env vars"]
fn jira_live_create_and_delete_issue() {
    let Some(env) = live_env() else {
        eprintln!(
            "skipping: RUSTWERK_JIRA_TEST_* env vars not set \
             (URL / TOKEN / USERNAME / PROJECT)"
        );
        return;
    };
    let (_guard, issue) = push_one_issue(&env);
    assert!(
        issue.key.starts_with(&format!("{}-", env.project_key)),
        "returned key {} should begin with project prefix {} (response: {})",
        issue.key,
        env.project_key,
        issue.raw_response,
    );
    // Sanity: the issue must actually exist before we
    // rely on the guard to delete it.
    match issue_exists(&env, &issue.key) {
        Ok(true) => {}
        Ok(false) => panic!("just-created issue {} not reachable", issue.key),
        Err(msg) => panic!("{msg}"),
    }
}

// Drop-based teardown depends on unwinding panics;
// `panic = "abort"` would skip the guard entirely
// (RT-Z3). The workspace uses the default `unwind`
// strategy today; this cfg guard fails the build loudly
// if a future profile flip silently invalidates the
// assertion below.
#[cfg(panic = "unwind")]
#[test]
#[ignore = "live Jira credentials required; opt in with RUSTWERK_JIRA_TEST_* env vars"]
fn jira_live_teardown_runs_on_panic() {
    // Acceptance: verify the RAII guard runs DELETE even
    // when the holding scope panics — otherwise a real
    // test failure would leak an issue into the tenant.
    let Some(env) = live_env() else {
        eprintln!(
            "skipping: RUSTWERK_JIRA_TEST_* env vars not set \
             (URL / TOKEN / USERNAME / PROJECT)"
        );
        return;
    };
    let (guard, issue) = push_one_issue(&env);
    let probe_env = env.clone();
    let probe_key = issue.key.clone();
    let outcome = std::panic::catch_unwind(move || {
        // Move the guard into the closure so it drops
        // during unwinding.
        let _g = guard;
        panic!("deliberate failure to exercise teardown");
    });
    assert!(outcome.is_err(), "expected the closure to panic");
    // Poll up to ~10s (RT-Z4) so a slow tenant's
    // DELETE-propagation window doesn't flake the test.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut wait = std::time::Duration::from_millis(250);
    loop {
        match issue_exists(&probe_env, &probe_key) {
            Ok(false) => break,
            Ok(true) => {}
            Err(msg) => panic!("{msg}"),
        }
        if std::time::Instant::now() >= deadline {
            panic!(
                "teardown guard should have deleted {} on panic (waited 10s)",
                probe_key
            );
        }
        std::thread::sleep(wait);
        wait = (wait * 2).min(std::time::Duration::from_secs(2));
    }
}

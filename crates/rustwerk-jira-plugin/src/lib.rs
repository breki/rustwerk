//! Jira integration plugin for rustwerk.
//!
//! This crate is a `cdylib` that exports the four
//! `extern "C"` functions required by
//! [`rustwerk_plugin_api`]. The host discovers the built
//! dynamic library (`.dll` / `.so` / `.dylib`) under
//! `.rustwerk/plugins/` or `~/.rustwerk/plugins/` and
//! calls into it to push rustwerk tasks as Jira Cloud
//! issues.
//!
//! High-level flow of
//! [`rustwerk_plugin_push_tasks`](crate::rustwerk_plugin_push_tasks):
//!
//! 1. Parse and validate the plugin config JSON
//!    ([`config::JiraConfig`]).
//! 2. Parse the tasks array ([`TaskDto`]s).
//! 3. For each task, build a Jira issue payload
//!    ([`mapping::build_issue_payload`]) and POST it via
//!    [`jira_client::create_issue`] — which handles
//!    gateway fallback on 401/404.
//! 4. Collect a [`TaskPushResult`] per task and return a
//!    top-level [`PluginResult`] whose `success` flag is
//!    the AND of all per-task outcomes.
//!
//! Body size of the config payload is capped via
//! [`deserialize_from_cstr_bounded`] to prevent a
//! misbehaving host from forcing unbounded allocation
//! inside the plugin.

// Unsafe code is confined to the FFI exports in this
// file; all other modules in this crate compile under
// the crate-level `unsafe_code = "deny"` lint.
#![allow(unsafe_code)]

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use rustwerk_plugin_api::{
    deserialize_from_cstr_bounded, serialize_to_cstring, PluginInfo,
    PluginResult, TaskDto, API_VERSION, ERR_GENERIC,
    ERR_INVALID_INPUT, ERR_OK,
};

use crate::config::JiraConfig;
use crate::jira_client::UreqClient;
use crate::push::{push_all, SystemClock};

mod config;
mod jira_client;
mod mapping;
mod push;
#[cfg(test)]
mod test_support;
mod transition;
mod warnings;

/// Maximum accepted size, in bytes, of the plugin config
/// and tasks payloads supplied by the host. Matches the
/// host's own cap so round-trip budgets align.
const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024;

/// Return the API version this plugin was built
/// against. Host must call this first and refuse to load
/// if it does not match its own [`API_VERSION`].
///
/// # Safety
///
/// Takes no pointers; trivially safe to call from any
/// host thread state.
#[no_mangle]
pub extern "C" fn rustwerk_plugin_api_version() -> u32 {
    API_VERSION
}

/// Write plugin metadata as a JSON [`PluginInfo`] into
/// `*out` (plugin-allocated).
///
/// # Safety
///
/// `out` must be a valid, non-null pointer to a
/// `*mut c_char` that the host initialised to null. On
/// return, `*out` is either null (on error) or a
/// plugin-allocated C string the host must release via
/// [`rustwerk_plugin_free_string`].
#[no_mangle]
pub unsafe extern "C" fn rustwerk_plugin_info(
    out: *mut *mut c_char,
) -> i32 {
    if out.is_null() {
        return ERR_GENERIC;
    }
    let info = PluginInfo {
        name: "jira".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        description: "Push rustwerk tasks to Jira Cloud".into(),
        capabilities: vec!["push_tasks".into()],
    };
    write_json(out, &info)
}

/// Push a JSON array of tasks to Jira Cloud.
///
/// # Safety
///
/// `config` and `tasks` must be valid, non-null,
/// NUL-terminated C strings. `out` must be a valid,
/// non-null pointer to a `*mut c_char` initialised to
/// null. The plugin owns `*out` on return; the host
/// releases it via [`rustwerk_plugin_free_string`].
#[no_mangle]
pub unsafe extern "C" fn rustwerk_plugin_push_tasks(
    config: *const c_char,
    tasks: *const c_char,
    out: *mut *mut c_char,
) -> i32 {
    if config.is_null() || tasks.is_null() || out.is_null() {
        return ERR_INVALID_INPUT;
    }

    let config_cstr = unsafe { CStr::from_ptr(config) };
    let tasks_cstr = unsafe { CStr::from_ptr(tasks) };

    let Ok(config_json) = config_cstr.to_str() else {
        return error_payload(out, ERR_INVALID_INPUT, "config is not valid UTF-8");
    };
    let cfg = match JiraConfig::from_json(config_json) {
        Ok(c) => c,
        Err(e) => return error_payload(out, ERR_INVALID_INPUT, &e.to_string()),
    };

    let task_list: Vec<TaskDto> =
        match deserialize_from_cstr_bounded(tasks_cstr, MAX_INPUT_BYTES) {
            Ok(t) => t,
            Err(e) => {
                return error_payload(
                    out,
                    ERR_INVALID_INPUT,
                    &format!("tasks payload invalid: {e}"),
                )
            }
        };

    let result = push_all(
        &UreqClient::default(),
        &SystemClock,
        &cfg,
        &task_list,
    );
    write_json(out, &result)
}

/// Release a C string previously returned by this plugin
/// via an out-pointer. Null input is a no-op.
///
/// # Safety
///
/// `ptr` must be either null or a pointer previously
/// returned by this plugin via `CString::into_raw`.
#[no_mangle]
pub unsafe extern "C" fn rustwerk_plugin_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    // Reconstruct the `CString` so its destructor frees
    // the allocation.
    let _ = unsafe { CString::from_raw(ptr) };
}

/// Serialize `value` as JSON and write it to `*out` as a
/// plugin-allocated C string. Returns [`ERR_OK`] on
/// success or [`ERR_GENERIC`] on serialization failure.
fn write_json<T: serde::Serialize>(out: *mut *mut c_char, value: &T) -> i32 {
    match serialize_to_cstring(value) {
        Ok(cstr) => {
            unsafe { *out = cstr.into_raw() };
            ERR_OK
        }
        Err(_) => ERR_GENERIC,
    }
}

/// Write a [`PluginResult`] error payload to `*out` and
/// return `code`. Used by the `push_tasks` entry point
/// so the host sees a well-formed JSON error instead of
/// a raw status code with a null buffer.
fn error_payload(out: *mut *mut c_char, code: i32, message: &str) -> i32 {
    let result = PluginResult {
        success: false,
        message: message.into(),
        task_results: None,
    };
    // If serialization fails we still return `code` so
    // the host sees the original failure class; the out-
    // pointer stays null in that case.
    let _ = write_json(out, &result);
    code
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::jira_client::{CreatedIssue, IssueKey, JiraOpOutcome};
    use crate::push::{
        build_created_state, existing_issue_key_validated, push_all,
        task_result_from_create_outcome, Clock, ExistingKey, SystemClock,
    };
    use crate::test_support::{ok, transport_err, MockHttp};
    use chrono::TimeZone;
    use rustwerk_plugin_api::TaskStatusDto;
    use serde_json::json;
    use std::ptr;

    struct FixedClock(chrono::DateTime<chrono::Utc>);

    impl Clock for FixedClock {
        fn now(&self) -> chrono::DateTime<chrono::Utc> {
            self.0
        }
    }

    const FIXED_NOW_STR: &str = "2026-04-22T09:14:07Z";

    fn clock() -> FixedClock {
        FixedClock(
            chrono::Utc
                .with_ymd_and_hms(2026, 4, 22, 9, 14, 7)
                .unwrap(),
        )
    }

    fn cfg() -> JiraConfig {
        JiraConfig {
            jira_url: "https://x.atlassian.net".into(),
            jira_token: "tok".into(),
            username: "u@example.com".into(),
            project_key: "PROJ".into(),
            default_issue_type: None,
            issue_type_map: std::collections::HashMap::new(),
            status_map: std::collections::HashMap::new(),
            assignee_map: std::collections::HashMap::new(),
            priority_map: std::collections::HashMap::new(),
            labels_from_tags: false,
        }
    }

    fn task(id: &str) -> TaskDto {
        TaskDto {
            id: id.into(),
            title: "title".into(),
            description: "desc".into(),
            status: TaskStatusDto::Todo,
            dependencies: vec![],
            effort_estimate: None,
            complexity: None,
            assignee: None,
            tags: vec![],
            issue_type: None,
            plugin_state: None,
        }
    }

    /// Task that already carries a `plugin_state.jira`
    /// blob from a previous push — the dispatch path
    /// should update, not create.
    fn task_with_state(id: &str, key: &str) -> TaskDto {
        let mut t = task(id);
        t.plugin_state = Some(json!({
            "key": key,
            "self": format!("https://x.atlassian.net/rest/api/3/issue/{key}"),
            "last_pushed_at": "2026-04-21T10:00:00Z",
        }));
        t
    }

    #[test]
    fn api_version_matches_crate_constant() {
        assert_eq!(rustwerk_plugin_api_version(), API_VERSION);
    }

    fn created_body(id: &str, key: &str) -> String {
        format!(
            r#"{{"id":"{id}","key":"{key}","self":"https://x.atlassian.net/rest/api/3/issue/{id}"}}"#
        )
    }

    #[test]
    fn push_all_reports_all_success() {
        let http = MockHttp::new(vec![
            ok(201, &created_body("1", "PROJ-1")),
            ok(201, &created_body("2", "PROJ-2")),
        ]);
        let result =
            push_all(&http, &clock(), &cfg(), &[task("A"), task("B")]);
        assert!(result.success);
        let rs = result.task_results.unwrap();
        assert_eq!(rs.len(), 2);
        assert_eq!(rs[0].external_key.as_deref(), Some("PROJ-1"));
        assert_eq!(rs[1].external_key.as_deref(), Some("PROJ-2"));
    }

    #[test]
    fn push_all_marks_partial_failure() {
        let http = MockHttp::new(vec![
            ok(201, &created_body("1", "PROJ-1")),
            ok(500, "boom"),
        ]);
        let result =
            push_all(&http, &clock(), &cfg(), &[task("A"), task("B")]);
        assert!(!result.success);
        assert!(result.message.contains("1 of 2"));
        let rs = result.task_results.unwrap();
        assert!(rs[0].success);
        assert!(!rs[1].success);
        assert!(rs[1].message.contains("500"));
    }

    #[test]
    fn push_all_reports_gateway_use_in_message() {
        let http = MockHttp::new(vec![
            ok(401, ""),
            ok(200, r#"{"cloudId":"cid"}"#),
            ok(201, &created_body("9", "PROJ-9")),
        ]);
        let result = push_all(&http, &clock(), &cfg(), &[task("A")]);
        let rs = result.task_results.unwrap();
        assert!(rs[0].success);
        assert!(rs[0].message.contains("gateway"));
        assert_eq!(rs[0].external_key.as_deref(), Some("PROJ-9"));
    }

    #[test]
    fn push_all_handles_http_errors() {
        let http = MockHttp::new(vec![transport_err("dns")]);
        let result = push_all(&http, &clock(), &cfg(), &[task("A")]);
        assert!(!result.success);
        let rs = result.task_results.unwrap();
        assert!(!rs[0].success);
        // Clean one-prefix Display from HttpError — no
        // longer accumulates "HTTP error: HTTP transport
        // error: …".
        assert!(rs[0].message.contains("HTTP transport error"));
        assert!(rs[0].message.contains("dns"));
        assert!(!rs[0].message.contains("HTTP error: HTTP"));
    }

    #[test]
    fn push_all_empty_task_list_is_success() {
        let http = MockHttp::new(vec![]);
        let result = push_all(&http, &clock(), &cfg(), &[]);
        assert!(result.success);
        assert!(result.message.contains("0 task"));
        assert_eq!(result.task_results.unwrap().len(), 0);
    }

    #[test]
    fn successful_push_emits_plugin_state_update_with_key_self_and_timestamp() {
        let http =
            MockHttp::new(vec![ok(201, &created_body("10042", "PROJ-142"))]);
        let result = push_all(&http, &clock(), &cfg(), &[task("A")]);
        let rs = result.task_results.unwrap();
        let state = rs[0].plugin_state_update.as_ref().unwrap();
        assert_eq!(state["key"], "PROJ-142");
        assert_eq!(
            state["self"],
            "https://x.atlassian.net/rest/api/3/issue/10042"
        );
        assert_eq!(state["last_pushed_at"], FIXED_NOW_STR);
    }

    #[test]
    fn failed_push_leaves_plugin_state_update_none() {
        let http = MockHttp::new(vec![ok(500, "boom")]);
        let result = push_all(&http, &clock(), &cfg(), &[task("A")]);
        let rs = result.task_results.unwrap();
        assert!(!rs[0].success);
        assert!(rs[0].plugin_state_update.is_none());
    }

    #[test]
    fn http_error_leaves_plugin_state_update_none() {
        let http = MockHttp::new(vec![transport_err("dns")]);
        let result = push_all(&http, &clock(), &cfg(), &[task("A")]);
        let rs = result.task_results.unwrap();
        assert!(rs[0].plugin_state_update.is_none());
    }

    #[test]
    fn success_with_empty_body_silently_omits_plugin_state_update() {
        // 204 No Content is a legitimate Jira outcome —
        // no body to anchor state against, so silently
        // skip. Still reported as success.
        let http = MockHttp::new(vec![ok(204, "")]);
        let result = push_all(&http, &clock(), &cfg(), &[task("A")]);
        let rs = result.task_results.unwrap();
        assert!(rs[0].success);
        assert!(rs[0].plugin_state_update.is_none());
        assert!(rs[0].external_key.is_none());
        assert!(!rs[0].message.contains("WARNING"));
    }

    #[test]
    fn success_with_malformed_body_warns_in_message() {
        // 2xx with a non-empty but unparseable body is a
        // schema-drift signal — the push counts as a
        // success, but without the issue key the NEXT
        // push would create a duplicate. Surface that
        // loudly in the task message so the operator
        // notices before duplicate issues pile up.
        let http = MockHttp::new(vec![ok(201, "not json at all")]);
        let result = push_all(&http, &clock(), &cfg(), &[task("A")]);
        let rs = result.task_results.unwrap();
        assert!(rs[0].success);
        assert!(rs[0].plugin_state_update.is_none());
        assert!(rs[0].external_key.is_none());
        assert!(rs[0].message.contains("WARNING"));
        assert!(rs[0].message.contains("duplicate"));
    }

    #[test]
    fn success_with_empty_key_warns_in_message() {
        let body = r#"{"key":"","self":"https://x.atlassian.net/x"}"#;
        let http = MockHttp::new(vec![ok(201, body)]);
        let result = push_all(&http, &clock(), &cfg(), &[task("A")]);
        let rs = result.task_results.unwrap();
        assert!(rs[0].success);
        assert!(rs[0].plugin_state_update.is_none());
        assert!(rs[0].message.contains("WARNING"));
    }

    #[test]
    fn success_with_non_http_self_url_warns_in_message() {
        let body = r#"{"key":"P-1","self":"javascript:steal()"}"#;
        let http = MockHttp::new(vec![ok(201, body)]);
        let result = push_all(&http, &clock(), &cfg(), &[task("A")]);
        let rs = result.task_results.unwrap();
        assert!(rs[0].success);
        assert!(rs[0].plugin_state_update.is_none());
        assert!(rs[0].message.contains("WARNING"));
    }

    #[test]
    fn system_clock_returns_utc_instant() {
        // Formatting is the concern of build_created_state;
        // SystemClock.now() just returns a DateTime<Utc>.
        // Sanity-check it's plausibly "now" (within the
        // last minute) and in UTC.
        let now = SystemClock.now();
        let wall = chrono::Utc::now();
        assert!((wall - now).num_seconds().abs() < 60);
        assert_eq!(now.offset(), &chrono::Utc);
    }

    #[test]
    fn build_created_state_formats_timestamp_as_utc_iso8601_seconds() {
        let created = CreatedIssue {
            key: IssueKey::parse("PROJ-1").unwrap(),
            self_url: "https://x.atlassian.net/rest/api/3/issue/1".into(),
        };
        let state = build_created_state(&created, clock().now());
        let stamp = state["last_pushed_at"].as_str().unwrap();
        assert_eq!(stamp, FIXED_NOW_STR);
        assert_eq!(stamp.len(), 20);
        assert!(stamp.ends_with('Z'));
        assert!(!stamp.contains('.'));
    }

    #[test]
    fn task_result_from_non_2xx_is_failure() {
        let outcome = JiraOpOutcome {
            status: 400,
            body: r#"{"errorMessages":["nope"]}"#.into(),
            used_gateway: false,
        };
        let r = task_result_from_create_outcome(&task("A"), &clock(), &outcome);
        assert!(!r.success);
        assert!(r.message.contains("400"));
        assert!(r.plugin_state_update.is_none());
    }

    #[test]
    fn error_payload_writes_plugin_result_json() {
        let mut out: *mut c_char = ptr::null_mut();
        let code = error_payload(&raw mut out, ERR_INVALID_INPUT, "bad cfg");
        assert_eq!(code, ERR_INVALID_INPUT);
        assert!(!out.is_null());
        let cstr = unsafe { CStr::from_ptr(out) };
        let parsed: PluginResult =
            serde_json::from_slice(cstr.to_bytes()).unwrap();
        assert!(!parsed.success);
        assert_eq!(parsed.message, "bad cfg");
        unsafe { rustwerk_plugin_free_string(out) };
    }

    #[test]
    fn info_writes_jira_plugin_metadata() {
        let mut out: *mut c_char = ptr::null_mut();
        let code = unsafe { rustwerk_plugin_info(&raw mut out) };
        assert_eq!(code, ERR_OK);
        let info: PluginInfo =
            serde_json::from_slice(unsafe { CStr::from_ptr(out) }.to_bytes())
                .unwrap();
        assert_eq!(info.name, "jira");
        assert!(info.capabilities.contains(&"push_tasks".to_string()));
        unsafe { rustwerk_plugin_free_string(out) };
    }

    #[test]
    fn info_rejects_null_out() {
        let code = unsafe { rustwerk_plugin_info(ptr::null_mut()) };
        assert_eq!(code, ERR_GENERIC);
    }

    #[test]
    fn free_string_accepts_null() {
        unsafe { rustwerk_plugin_free_string(ptr::null_mut()) };
    }

    #[test]
    fn push_tasks_rejects_null_pointers() {
        let mut out: *mut c_char = ptr::null_mut();
        let code = unsafe {
            rustwerk_plugin_push_tasks(
                ptr::null(),
                ptr::null(),
                &raw mut out,
            )
        };
        assert_eq!(code, ERR_INVALID_INPUT);
    }

    #[test]
    fn push_tasks_rejects_invalid_config_json() {
        let config = CString::new("not json").unwrap();
        let tasks = CString::new("[]").unwrap();
        let mut out: *mut c_char = ptr::null_mut();
        let code = unsafe {
            rustwerk_plugin_push_tasks(
                config.as_ptr(),
                tasks.as_ptr(),
                &raw mut out,
            )
        };
        assert_eq!(code, ERR_INVALID_INPUT);
        assert!(!out.is_null());
        unsafe { rustwerk_plugin_free_string(out) };
    }

    #[test]
    fn push_tasks_rejects_missing_required_config_field() {
        let config = CString::new(
            serde_json::json!({
                "jira_url": "",
                "jira_token": "t",
                "username": "u",
                "project_key": "P",
            })
            .to_string(),
        )
        .unwrap();
        let tasks = CString::new("[]").unwrap();
        let mut out: *mut c_char = ptr::null_mut();
        let code = unsafe {
            rustwerk_plugin_push_tasks(
                config.as_ptr(),
                tasks.as_ptr(),
                &raw mut out,
            )
        };
        assert_eq!(code, ERR_INVALID_INPUT);
        let parsed: PluginResult = serde_json::from_slice(
            unsafe { CStr::from_ptr(out) }.to_bytes(),
        )
        .unwrap();
        assert!(parsed.message.contains("jira_url"));
        unsafe { rustwerk_plugin_free_string(out) };
    }

    #[test]
    fn push_tasks_rejects_invalid_tasks_json() {
        let config = CString::new(
            serde_json::json!({
                "jira_url": "https://x",
                "jira_token": "t",
                "username": "u",
                "project_key": "P",
            })
            .to_string(),
        )
        .unwrap();
        let tasks = CString::new("not a json array").unwrap();
        let mut out: *mut c_char = ptr::null_mut();
        let code = unsafe {
            rustwerk_plugin_push_tasks(
                config.as_ptr(),
                tasks.as_ptr(),
                &raw mut out,
            )
        };
        assert_eq!(code, ERR_INVALID_INPUT);
        unsafe { rustwerk_plugin_free_string(out) };
    }

    // ------------------------------------------------
    // PLG-JIRA-UPDATE: dispatch + state-update semantics
    // ------------------------------------------------

    use crate::test_support::Call;

    #[test]
    fn existing_issue_key_reads_and_validates_jira_key_from_plugin_state() {
        let t = task_with_state("A", "PROJ-7");
        match existing_issue_key_validated(&t) {
            ExistingKey::Valid(k) => assert_eq!(k.as_str(), "PROJ-7"),
            other => panic!("expected Valid, got {other:?}"),
        }
    }

    #[test]
    fn existing_issue_key_missing_state_returns_none_variant() {
        assert!(matches!(
            existing_issue_key_validated(&task("A")),
            ExistingKey::None
        ));
    }

    #[test]
    fn existing_issue_key_wrong_shape_returns_none_variant() {
        let mut t = task("A");
        t.plugin_state = Some(json!({ "key": 42 })); // not a string
        assert!(matches!(
            existing_issue_key_validated(&t),
            ExistingKey::None
        ));
    }

    #[test]
    fn existing_issue_key_malformed_value_returns_invalid_variant() {
        // Path-traversal attempt in stored state —
        // must NOT be accepted (RT-121).
        let mut t = task("A");
        t.plugin_state = Some(json!({ "key": "../../admin" }));
        assert!(matches!(
            existing_issue_key_validated(&t),
            ExistingKey::Invalid(k) if k == "../../admin"
        ));
    }

    #[test]
    fn push_fails_loudly_when_stored_key_is_invalid() {
        // Poisoned project.json → the whole task must
        // fail, not silently recreate (RT-121).
        let mut t = task("A");
        t.plugin_state = Some(json!({ "key": "../../admin" }));
        // No HTTP calls should be made at all.
        let http = MockHttp::new(vec![]);
        let result = push_all(&http, &clock(), &cfg(), &[t]);
        assert!(!result.success);
        let rs = result.task_results.unwrap();
        assert!(rs[0].message.contains("not a valid issue key"));
        assert!(http.calls().is_empty());
    }

    #[test]
    fn issue_key_parse_accepts_valid_keys() {
        assert!(IssueKey::parse("PROJ-1").is_some());
        assert!(IssueKey::parse("PROJ-142").is_some());
        assert!(IssueKey::parse("A-1").is_some());
        assert!(IssueKey::parse("MY_PROJECT-123").is_some());
        assert!(IssueKey::parse("ABC2-9").is_some());
    }

    #[test]
    fn issue_key_parse_rejects_path_traversal_and_other_garbage() {
        assert!(IssueKey::parse("").is_none());
        assert!(IssueKey::parse("../../admin").is_none());
        assert!(IssueKey::parse("PROJ").is_none());
        assert!(IssueKey::parse("-1").is_none());
        assert!(IssueKey::parse("PROJ-").is_none());
        assert!(IssueKey::parse("proj-1").is_none()); // lowercase
        assert!(IssueKey::parse("PROJ-1a").is_none()); // non-digit suffix
        assert!(IssueKey::parse("PROJ/1").is_none());
        assert!(IssueKey::parse("PROJ-1?expand=X").is_none());
        assert!(IssueKey::parse("PROJ-1#frag").is_none());
        assert!(IssueKey::parse("PROJ-1 ").is_none());
        assert!(IssueKey::parse("PROJ-1\n").is_none());
        assert!(IssueKey::parse(&"A".repeat(65)).is_none()); // length cap
    }

    #[test]
    fn second_push_of_pushed_task_sends_put_not_post() {
        // Probe GET /issue/PROJ-7 → 200, then PUT → 204.
        let http = MockHttp::new(vec![
            ok(200, r#"{"key":"PROJ-7"}"#), // probe
            ok(204, ""),                    // PUT success
        ]);
        let result =
            push_all(&http, &clock(), &cfg(), &[task_with_state("A", "PROJ-7")]);
        assert!(result.success);
        let calls = http.calls();
        assert_eq!(calls.len(), 2, "calls: {calls:?}");
        assert!(matches!(&calls[0], Call::Get { url, .. }
            if url == "https://x.atlassian.net/rest/api/3/issue/PROJ-7"));
        assert!(matches!(&calls[1], Call::Put { url, .. }
            if url == "https://x.atlassian.net/rest/api/3/issue/PROJ-7"));
        // No POST — the create verb must not be used.
        assert!(calls.iter().all(|c| !matches!(c, Call::Post { .. })));
        let rs = result.task_results.unwrap();
        assert_eq!(rs[0].message, "updated (HTTP 204)");
        assert_eq!(rs[0].external_key.as_deref(), Some("PROJ-7"));
    }

    #[test]
    fn successful_put_refreshes_last_pushed_at_and_preserves_key_self() {
        let http = MockHttp::new(vec![ok(200, r#"{"key":"PROJ-7"}"#), ok(204, "")]);
        let result =
            push_all(&http, &clock(), &cfg(), &[task_with_state("A", "PROJ-7")]);
        let rs = result.task_results.unwrap();
        let state = rs[0].plugin_state_update.as_ref().unwrap();
        assert_eq!(state["key"], "PROJ-7");
        assert_eq!(
            state["self"],
            "https://x.atlassian.net/rest/api/3/issue/PROJ-7"
        );
        // Refreshed to the fixed-clock value, not the
        // stale 2026-04-21 value baked into
        // task_with_state.
        assert_eq!(state["last_pushed_at"], FIXED_NOW_STR);
    }

    #[test]
    fn second_push_after_deletion_recreates_and_overwrites_state() {
        // Direct GET → 404, tenant_info 200, gateway GET
        // also 404 → caller treats as deleted, recreates
        // via POST. The new create returns PROJ-99 which
        // must overwrite the stored PROJ-7.
        let http = MockHttp::new(vec![
            ok(404, "gone"),                 // direct GET
            ok(200, r#"{"cloudId":"cid"}"#), // tenant_info
            ok(404, "still gone"),           // gateway GET
            ok(
                201,
                r#"{"id":"99","key":"PROJ-99","self":"https://x.atlassian.net/rest/api/3/issue/99"}"#,
            ), // recreate POST
        ]);
        let result =
            push_all(&http, &clock(), &cfg(), &[task_with_state("A", "PROJ-7")]);
        assert!(result.success);
        let rs = result.task_results.unwrap();
        let state = rs[0].plugin_state_update.as_ref().unwrap();
        assert_eq!(state["key"], "PROJ-99");
        assert_eq!(
            state["self"],
            "https://x.atlassian.net/rest/api/3/issue/99"
        );
        assert_eq!(rs[0].external_key.as_deref(), Some("PROJ-99"));
        assert!(rs[0].message.contains("created"));
    }

    #[test]
    fn failed_put_leaves_stored_key_and_self_unchanged() {
        // Probe 200, PUT 500 → fail with message; state
        // update stays None so host keeps the prior
        // key/self.
        let http = MockHttp::new(vec![
            ok(200, r#"{"key":"PROJ-7"}"#),
            ok(500, "broken"),
        ]);
        let result =
            push_all(&http, &clock(), &cfg(), &[task_with_state("A", "PROJ-7")]);
        assert!(!result.success);
        let rs = result.task_results.unwrap();
        assert!(!rs[0].success);
        assert!(rs[0].plugin_state_update.is_none());
        assert!(rs[0].message.contains("500"));
        assert!(rs[0].message.contains("PROJ-7"));
    }

    #[test]
    fn put_transport_error_leaves_state_untouched() {
        let http = MockHttp::new(vec![
            ok(200, r#"{"key":"PROJ-7"}"#),
            transport_err("connection reset"),
        ]);
        let result =
            push_all(&http, &clock(), &cfg(), &[task_with_state("A", "PROJ-7")]);
        let rs = result.task_results.unwrap();
        assert!(!rs[0].success);
        assert!(rs[0].plugin_state_update.is_none());
        assert!(rs[0].message.contains("connection reset"));
    }

    #[test]
    fn probe_non_2xx_non_404_fails_without_touching_state() {
        // 500 on probe → fail immediately, never PUT.
        let http = MockHttp::new(vec![ok(500, "oops")]);
        let result =
            push_all(&http, &clock(), &cfg(), &[task_with_state("A", "PROJ-7")]);
        let rs = result.task_results.unwrap();
        assert!(!rs[0].success);
        assert!(rs[0].plugin_state_update.is_none());
        assert!(rs[0].message.contains("probe"));
        let calls = http.calls();
        assert!(calls.iter().all(|c| !matches!(c, Call::Put { .. })));
    }

    #[test]
    fn ambiguous_probe_404_fails_without_recreating_duplicate() {
        // RT-122: direct GET 401 + gateway GET 404 is
        // NOT proof the issue is deleted — the direct
        // read was blocked, so the gateway's "missing"
        // answer could just mean scoped-token
        // restriction. Must fail, not recreate.
        let http = MockHttp::new(vec![
            ok(401, ""),                     // direct GET
            ok(200, r#"{"cloudId":"cid"}"#), // tenant_info
            ok(404, ""),                     // gateway GET → AMBIGUOUS
        ]);
        let result =
            push_all(&http, &clock(), &cfg(), &[task_with_state("A", "PROJ-7")]);
        assert!(!result.success);
        let rs = result.task_results.unwrap();
        assert!(!rs[0].success);
        assert!(rs[0].message.contains("ambiguous"));
        assert!(rs[0].plugin_state_update.is_none());
        // No POST — we must NOT have created a duplicate.
        assert!(http
            .calls()
            .iter()
            .all(|c| !matches!(c, Call::Post { .. })));
    }

    #[test]
    fn refresh_preserves_additive_state_fields() {
        // RT-123: future plugin versions may write
        // additional fields (last_hash,
        // last_response_etag, …) into created state.
        // The update path must carry them verbatim —
        // wholesale reconstruction silently dropped
        // them before this fix.
        let mut t = task("A");
        t.plugin_state = Some(json!({
            "key": "PROJ-7",
            "self": "https://x.atlassian.net/rest/api/3/issue/7",
            "last_pushed_at": "2026-04-21T10:00:00Z",
            "last_hash": "deadbeef",
            "custom_field": { "nested": true },
        }));
        let http = MockHttp::new(vec![ok(200, r#"{"key":"PROJ-7"}"#), ok(204, "")]);
        let result = push_all(&http, &clock(), &cfg(), &[t]);
        let rs = result.task_results.unwrap();
        let state = rs[0].plugin_state_update.as_ref().unwrap();
        assert_eq!(state["last_pushed_at"], FIXED_NOW_STR);
        assert_eq!(state["last_hash"], "deadbeef");
        assert_eq!(state["custom_field"]["nested"], true);
        assert_eq!(state["key"], "PROJ-7");
    }

    #[test]
    fn update_message_reports_gateway_when_probe_alone_used_it() {
        // RT-125 (fold-in): if only the probe went
        // through the gateway, the task message must
        // still say "via gateway" — previously it
        // silently reported direct-only.
        let http = MockHttp::new(vec![
            ok(401, ""),                     // direct GET
            ok(200, r#"{"cloudId":"cid"}"#), // tenant_info
            ok(200, r#"{"key":"PROJ-7"}"#), // gateway GET → exists
            ok(204, ""),                   // direct PUT → no fallback
        ]);
        let result =
            push_all(&http, &clock(), &cfg(), &[task_with_state("A", "PROJ-7")]);
        let rs = result.task_results.unwrap();
        assert!(rs[0].success);
        assert!(
            rs[0].message.contains("via gateway"),
            "message should note gateway, got: {}",
            rs[0].message
        );
    }

    #[test]
    fn gateway_fallback_applies_to_probe_and_update() {
        // Direct GET 401 → tenant_info 200 → gateway GET
        // 200 (exists) → direct PUT 401 → tenant_info
        // 200 (reused — but here we model a second
        // lookup for simplicity since update_issue does
        // its own fallback) → gateway PUT 204.
        let http = MockHttp::new(vec![
            ok(401, ""),
            ok(200, r#"{"cloudId":"cid"}"#),
            ok(200, r#"{"key":"PROJ-7"}"#),
            ok(401, ""),
            ok(200, r#"{"cloudId":"cid"}"#),
            ok(204, ""),
        ]);
        let result =
            push_all(&http, &clock(), &cfg(), &[task_with_state("A", "PROJ-7")]);
        assert!(result.success);
        let rs = result.task_results.unwrap();
        assert!(rs[0].message.contains("via gateway"));
        let calls = http.calls();
        // Verify both probe retry and update retry went
        // through the gateway URL.
        let gateway_hits = calls
            .iter()
            .filter(|c| match c {
                Call::Get { url, .. } | Call::Put { url, .. } => {
                    url.starts_with("https://api.atlassian.com/ex/jira/cid/")
                }
                Call::Post { .. } => false,
            })
            .count();
        assert_eq!(gateway_hits, 2);
    }

    #[test]
    fn recreate_message_does_not_claim_update() {
        // After 404/404 recreate, the operator-facing
        // message should say "created", not "updated".
        let http = MockHttp::new(vec![
            ok(404, ""),
            ok(200, r#"{"cloudId":"cid"}"#),
            ok(404, ""),
            ok(
                201,
                r#"{"id":"99","key":"PROJ-99","self":"https://x.atlassian.net/rest/api/3/issue/99"}"#,
            ),
        ]);
        let result =
            push_all(&http, &clock(), &cfg(), &[task_with_state("A", "PROJ-7")]);
        let rs = result.task_results.unwrap();
        assert!(rs[0].message.starts_with("created"));
        assert!(!rs[0].message.contains("updated"));
    }

    // ------------------------------------------------
    // PLG-JIRA-FIELDS: transition + mapping integration
    // ------------------------------------------------

    fn cfg_with_in_progress_mapped() -> JiraConfig {
        let mut c = cfg();
        c.status_map.insert("in_progress".into(), "11".into());
        c
    }

    #[test]
    fn create_with_unmapped_status_records_last_status_and_fires_no_transition() {
        let http = MockHttp::new(vec![ok(201, &created_body("1", "PROJ-1"))]);
        let mut t = task("A");
        t.status = TaskStatusDto::InProgress;
        let result = push_all(&http, &clock(), &cfg(), &[t]);
        let rs = result.task_results.unwrap();
        assert!(rs[0].success);
        assert_eq!(http.calls().len(), 1); // only the POST, no transition
        let state = rs[0].plugin_state_update.as_ref().unwrap();
        assert_eq!(state["last_status"], "in_progress");
    }

    #[test]
    fn create_with_mapped_status_fires_transition_and_records_last_status() {
        let http = MockHttp::new(vec![
            ok(201, &created_body("1", "PROJ-1")),
            ok(204, ""), // transition
        ]);
        let mut t = task("A");
        t.status = TaskStatusDto::InProgress;
        let result = push_all(&http, &clock(), &cfg_with_in_progress_mapped(), &[t]);
        let rs = result.task_results.unwrap();
        assert!(rs[0].success);
        assert!(rs[0].message.contains("transitioned to 11"));
        assert!(rs[0].message.contains("HTTP 204"));
        assert_eq!(http.calls().len(), 2);
        match &http.calls()[1] {
            Call::Post { url, body, .. } => {
                assert!(url.ends_with("/issue/PROJ-1/transitions"));
                assert!(body.contains(r#""id":"11""#));
            }
            other => panic!("expected POST transition, got {other:?}"),
        }
        let state = rs[0].plugin_state_update.as_ref().unwrap();
        assert_eq!(state["last_status"], "in_progress");
    }

    #[test]
    fn create_transition_failure_warns_without_failing_task_and_omits_last_status() {
        let http = MockHttp::new(vec![
            ok(201, &created_body("1", "PROJ-1")),
            ok(400, r#"{"errorMessages":["bad transition"]}"#),
        ]);
        let mut t = task("A");
        t.status = TaskStatusDto::InProgress;
        let result = push_all(&http, &clock(), &cfg_with_in_progress_mapped(), &[t]);
        let rs = result.task_results.unwrap();
        // Task push still succeeds — the issue exists,
        // only the workflow sync failed. Operator sees
        // the warning and can retry.
        assert!(rs[0].success);
        assert!(rs[0].message.contains("WARNING"));
        assert!(rs[0].message.contains("transition to 11"));
        assert!(rs[0].message.contains("400"));
        let state = rs[0].plugin_state_update.as_ref().unwrap();
        assert!(
            state.get("last_status").is_none(),
            "last_status must stay absent so next push retries the transition"
        );
    }

    #[test]
    fn update_with_status_unchanged_since_last_push_skips_transition() {
        // plugin_state.last_status == current status →
        // no transition call expected.
        let mut t = task_with_state("A", "PROJ-7");
        t.status = TaskStatusDto::InProgress;
        if let Some(obj) = t.plugin_state.as_mut().and_then(|s| s.as_object_mut()) {
            obj.insert("last_status".into(), json!("in_progress"));
        }
        let http = MockHttp::new(vec![
            ok(200, r#"{"key":"PROJ-7"}"#), // probe
            ok(204, ""),                    // PUT
        ]);
        let result = push_all(&http, &clock(), &cfg_with_in_progress_mapped(), &[t]);
        let rs = result.task_results.unwrap();
        assert!(rs[0].success);
        assert!(!rs[0].message.contains("transitioned"));
        let calls = http.calls();
        assert_eq!(calls.len(), 2, "probe + PUT only, no transition");
    }

    #[test]
    fn update_with_changed_status_fires_transition_and_updates_last_status() {
        let mut t = task_with_state("A", "PROJ-7");
        t.status = TaskStatusDto::InProgress;
        if let Some(obj) = t.plugin_state.as_mut().and_then(|s| s.as_object_mut()) {
            obj.insert("last_status".into(), json!("todo"));
        }
        let http = MockHttp::new(vec![
            ok(200, r#"{"key":"PROJ-7"}"#), // probe
            ok(204, ""),                    // PUT
            ok(204, ""),                    // transition
        ]);
        let result = push_all(&http, &clock(), &cfg_with_in_progress_mapped(), &[t]);
        let rs = result.task_results.unwrap();
        assert!(rs[0].success);
        assert!(rs[0].message.contains("transitioned to 11"));
        let state = rs[0].plugin_state_update.as_ref().unwrap();
        assert_eq!(state["last_status"], "in_progress");
        // Additive fields from the prior state are preserved.
        assert_eq!(state["key"], "PROJ-7");
    }

    #[test]
    fn mapping_warnings_surface_in_task_message_even_on_success() {
        // Configured assignee_map but the task's assignee
        // isn't in it → payload omits the field, message
        // carries a WARNING.
        let http = MockHttp::new(vec![ok(201, &created_body("1", "PROJ-1"))]);
        let mut c = cfg();
        c.assignee_map.insert("alice@example.com".into(), "A".into());
        let mut t = task("A");
        t.assignee = Some("bob@example.com".into());
        let result = push_all(&http, &clock(), &c, &[t]);
        let rs = result.task_results.unwrap();
        assert!(rs[0].success);
        assert!(rs[0].message.contains("WARNING"));
        assert!(rs[0].message.contains("bob@example.com"));
    }

    #[test]
    fn payload_carries_mapped_fields_on_create() {
        // Spot-check that assignee + priority + labels
        // reach the outbound body when mapped.
        let http = MockHttp::new(vec![ok(201, &created_body("1", "PROJ-1"))]);
        let mut c = cfg();
        c.assignee_map.insert("a@x.com".into(), "acct-1".into());
        c.priority_map.insert("1".into(), "Highest".into());
        c.labels_from_tags = true;
        let mut t = task("A");
        t.assignee = Some("a@x.com".into());
        t.complexity = Some(1);
        t.tags = vec!["alpha".into()];
        let _ = push_all(&http, &clock(), &c, &[t]);
        match &http.calls()[0] {
            Call::Post { body, .. } => {
                let v: serde_json::Value = serde_json::from_str(body).unwrap();
                assert_eq!(v["fields"]["assignee"]["accountId"], "acct-1");
                assert_eq!(v["fields"]["priority"]["name"], "Highest");
                assert_eq!(v["fields"]["labels"][0], "alpha");
            }
            other => panic!("expected POST, got {other:?}"),
        }
    }

    #[test]
    fn update_still_fires_transition_when_stored_state_malformed() {
        // RT-X1: a prior push wrote a state blob missing
        // `self`; PUT still succeeds but
        // build_refreshed_state returned None, so the
        // earlier gating on `plugin_state_update.is_some()`
        // silently skipped the transition. Now we gate on
        // `external_key` — transition fires, state is
        // synthesized to record `last_status`.
        let mut t = task("A");
        t.status = TaskStatusDto::InProgress;
        t.plugin_state = Some(json!({ "key": "PROJ-7" })); // missing "self"
        let http = MockHttp::new(vec![
            ok(200, r#"{"key":"PROJ-7"}"#), // probe
            ok(204, ""),                    // PUT
            ok(204, ""),                    // transition
        ]);
        let result = push_all(&http, &clock(), &cfg_with_in_progress_mapped(), &[t]);
        let rs = result.task_results.unwrap();
        assert!(rs[0].success);
        assert!(
            rs[0].message.contains("transitioned to 11"),
            "message should note transition, got {}",
            rs[0].message
        );
        let state = rs[0].plugin_state_update.as_ref().unwrap();
        assert_eq!(state["last_status"], "in_progress");
    }

    #[test]
    fn dto_as_wire_is_reachable_from_plugin() {
        // Smoke test: the plugin relies on the DTO-side
        // `as_wire()` helper for status_map lookups. This
        // test exists so a breaking change to the helper
        // surfaces here rather than at runtime.
        assert_eq!(TaskStatusDto::InProgress.as_wire(), "in_progress");
    }

    #[test]
    fn transition_transport_error_does_not_fail_task() {
        let http = MockHttp::new(vec![
            ok(201, &created_body("1", "PROJ-1")),
            transport_err("reset"),
        ]);
        let mut t = task("A");
        t.status = TaskStatusDto::InProgress;
        let result = push_all(&http, &clock(), &cfg_with_in_progress_mapped(), &[t]);
        let rs = result.task_results.unwrap();
        assert!(rs[0].success);
        assert!(rs[0].message.contains("WARNING"));
        assert!(rs[0].message.contains("reset"));
    }
}

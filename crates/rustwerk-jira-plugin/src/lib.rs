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
    PluginResult, TaskDto, TaskPushResult, API_VERSION, ERR_GENERIC,
    ERR_INVALID_INPUT, ERR_OK,
};
use serde_json::Value;

use crate::config::JiraConfig;
use crate::jira_client::{create_issue, CreateIssueOutcome, HttpClient, UreqClient};

mod config;
mod jira_client;
mod mapping;

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

    let result = push_all(&UreqClient::default(), &cfg, &task_list);
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

/// Walk every task, attempt to create its Jira issue,
/// collect per-task results, and aggregate into a
/// [`PluginResult`].
fn push_all<C: HttpClient>(
    http: &C,
    cfg: &JiraConfig,
    tasks: &[TaskDto],
) -> PluginResult {
    let results: Vec<TaskPushResult> =
        tasks.iter().map(|t| push_one(http, cfg, t)).collect();
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

/// Push a single task. Converts HTTP-client errors and
/// non-2xx responses into a failed [`TaskPushResult`]
/// rather than propagating so one bad task does not
/// abort the batch.
fn push_one<C: HttpClient>(
    http: &C,
    cfg: &JiraConfig,
    task: &TaskDto,
) -> TaskPushResult {
    let payload = mapping::build_issue_payload(task, &cfg.project_key);
    let body = payload.to_string();
    match create_issue(http, cfg, &body) {
        Ok(outcome) => task_result_from_outcome(task, &outcome),
        Err(e) => TaskPushResult::fail(task.id.clone(), format!("HTTP error: {e}")),
    }
}

/// Translate a low-level [`CreateIssueOutcome`] into the
/// per-task public result DTO.
fn task_result_from_outcome(
    task: &TaskDto,
    outcome: &CreateIssueOutcome,
) -> TaskPushResult {
    if (200..300).contains(&outcome.status) {
        let key = extract_issue_key(&outcome.body);
        let message = if outcome.used_gateway {
            format!(
                "created (HTTP {}, via gateway)",
                outcome.status
            )
        } else {
            format!("created (HTTP {})", outcome.status)
        };
        let mut result = TaskPushResult::ok(task.id.clone(), message);
        if let Some(k) = key {
            result = result.with_external_key(k);
        }
        result
    } else {
        TaskPushResult::fail(
            task.id.clone(),
            format!("Jira returned HTTP {}: {}", outcome.status, outcome.body),
        )
    }
}

/// Pull the `key` field (e.g. `"PROJ-123"`) out of the
/// Jira issue-creation response body, if present.
fn extract_issue_key(body: &str) -> Option<String> {
    let v: Value = serde_json::from_str(body).ok()?;
    v.get("key")?.as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustwerk_plugin_api::TaskStatusDto;
    use std::cell::RefCell;
    use std::ptr;

    struct FakeHttp {
        responses: RefCell<Vec<Result<jira_client::HttpResponse, String>>>,
    }

    impl FakeHttp {
        fn new(r: Vec<Result<jira_client::HttpResponse, String>>) -> Self {
            Self {
                responses: RefCell::new(r),
            }
        }

        fn pop(&self) -> Result<jira_client::HttpResponse, String> {
            self.responses
                .borrow_mut()
                .drain(..1)
                .next()
                .expect("FakeHttp exhausted")
        }
    }

    impl HttpClient for FakeHttp {
        fn get(
            &self,
            _url: &str,
            _auth: &str,
        ) -> Result<jira_client::HttpResponse, String> {
            self.pop()
        }
        fn post_json(
            &self,
            _url: &str,
            _auth: &str,
            _body: &str,
        ) -> Result<jira_client::HttpResponse, String> {
            self.pop()
        }
    }

    fn ok(status: u16, body: &str) -> Result<jira_client::HttpResponse, String> {
        Ok(jira_client::HttpResponse {
            status,
            body: body.into(),
        })
    }

    fn cfg() -> JiraConfig {
        JiraConfig {
            jira_url: "https://x.atlassian.net".into(),
            jira_token: "tok".into(),
            username: "u@example.com".into(),
            project_key: "PROJ".into(),
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
            plugin_state: None,
        }
    }

    #[test]
    fn api_version_matches_crate_constant() {
        assert_eq!(rustwerk_plugin_api_version(), API_VERSION);
    }

    #[test]
    fn push_all_reports_all_success() {
        let http = FakeHttp::new(vec![
            ok(201, r#"{"key":"PROJ-1"}"#),
            ok(201, r#"{"key":"PROJ-2"}"#),
        ]);
        let result = push_all(&http, &cfg(), &[task("A"), task("B")]);
        assert!(result.success);
        let rs = result.task_results.unwrap();
        assert_eq!(rs.len(), 2);
        assert_eq!(rs[0].external_key.as_deref(), Some("PROJ-1"));
        assert_eq!(rs[1].external_key.as_deref(), Some("PROJ-2"));
    }

    #[test]
    fn push_all_marks_partial_failure() {
        let http = FakeHttp::new(vec![
            ok(201, r#"{"key":"PROJ-1"}"#),
            ok(500, "boom"),
        ]);
        let result = push_all(&http, &cfg(), &[task("A"), task("B")]);
        assert!(!result.success);
        assert!(result.message.contains("1 of 2"));
        let rs = result.task_results.unwrap();
        assert!(rs[0].success);
        assert!(!rs[1].success);
        assert!(rs[1].message.contains("500"));
    }

    #[test]
    fn push_all_reports_gateway_use_in_message() {
        let http = FakeHttp::new(vec![
            ok(401, ""),
            ok(200, r#"{"cloudId":"cid"}"#),
            ok(201, r#"{"key":"PROJ-9"}"#),
        ]);
        let result = push_all(&http, &cfg(), &[task("A")]);
        let rs = result.task_results.unwrap();
        assert!(rs[0].success);
        assert!(rs[0].message.contains("gateway"));
        assert_eq!(rs[0].external_key.as_deref(), Some("PROJ-9"));
    }

    #[test]
    fn push_all_handles_http_errors() {
        let http = FakeHttp::new(vec![Err("dns".into())]);
        let result = push_all(&http, &cfg(), &[task("A")]);
        assert!(!result.success);
        let rs = result.task_results.unwrap();
        assert!(!rs[0].success);
        assert!(rs[0].message.contains("HTTP error"));
    }

    #[test]
    fn push_all_empty_task_list_is_success() {
        let http = FakeHttp::new(vec![]);
        let result = push_all(&http, &cfg(), &[]);
        assert!(result.success);
        assert!(result.message.contains("0 task"));
        assert_eq!(result.task_results.unwrap().len(), 0);
    }

    #[test]
    fn extract_issue_key_reads_field() {
        assert_eq!(
            extract_issue_key(r#"{"id":"1","key":"PROJ-1"}"#),
            Some("PROJ-1".into())
        );
    }

    #[test]
    fn extract_issue_key_missing_returns_none() {
        assert_eq!(extract_issue_key(r#"{"id":"1"}"#), None);
        assert_eq!(extract_issue_key("not json"), None);
    }

    #[test]
    fn task_result_from_non_2xx_is_failure() {
        let outcome = CreateIssueOutcome {
            status: 400,
            body: r#"{"errorMessages":["nope"]}"#.into(),
            used_gateway: false,
        };
        let r = task_result_from_outcome(&task("A"), &outcome);
        assert!(!r.success);
        assert!(r.message.contains("400"));
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
}

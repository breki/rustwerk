//! Plugin API for rustwerk.
//!
//! This crate defines the stable contract between the
//! rustwerk host binary and plugin dynamic libraries.
//! It has a minimal dependency surface (`serde`,
//! `serde_json`, `thiserror`) so plugins can link
//! against it without pulling in the full rustwerk
//! tree.
//!
//! # FFI contract
//!
//! A rustwerk plugin is a dynamic library (`cdylib`)
//! that exports four `extern "C"` functions. Data
//! crosses the FFI boundary as JSON-encoded,
//! null-terminated C strings.
//!
//! ```text
//! rustwerk_plugin_api_version() -> u32
//! rustwerk_plugin_info(out: *mut *mut c_char) -> i32
//! rustwerk_plugin_push_tasks(
//!     config: *const c_char,
//!     tasks:  *const c_char,
//!     out:    *mut *mut c_char,
//! ) -> i32
//! rustwerk_plugin_free_string(ptr: *mut c_char)
//! ```
//!
//! ## Call order
//!
//! The host **must** call `rustwerk_plugin_api_version`
//! **first**, before invoking any other export. If the
//! returned value is not equal to [`API_VERSION`] the
//! host must unload the plugin without calling
//! `rustwerk_plugin_info` or any other entry point.
//!
//! ## Return codes
//!
//! Integer return codes use the `ERR_*` constants in
//! this crate:
//!
//! - [`ERR_OK`] — success.
//! - [`ERR_GENERIC`] — unspecified plugin-side error.
//! - [`ERR_INVALID_INPUT`] — host-supplied JSON failed
//!   validation.
//! - [`ERR_VERSION_MISMATCH`] — plugin detected an
//!   incompatible host version.
//!
//! Additional positive codes are reserved for future
//! use; hosts must treat unknown non-zero codes as
//! [`ERR_GENERIC`].
//!
//! ## Out-pointer ownership
//!
//! For every function that accepts an `out: *mut *mut
//! c_char`:
//!
//! 1. The host **must** initialize `*out = null` before
//!    the call.
//! 2. The plugin **must** leave `*out` either null or
//!    pointing to a plugin-allocated C string —
//!    regardless of the return code. On error the
//!    string may carry a JSON error payload or be null.
//! 3. The host **must** call
//!    `rustwerk_plugin_free_string(*out)` after reading
//!    the value, for both success and error returns.
//!    Passing a null pointer to `rustwerk_plugin_free_string`
//!    is a no-op.
//!
//! These rules guarantee that allocations cross the FFI
//! boundary using the plugin's allocator on both sides
//! and that the host never passes uninitialized memory
//! to the free function.
//!
//! ## Capability matching
//!
//! [`PluginInfo::capabilities`] entries are matched
//! case-sensitively against the lowercase ASCII
//! identifiers recognized by the host (e.g.
//! `"push_tasks"`). Unknown capabilities are ignored.
//! Plugins should emit only lowercase identifiers to
//! avoid silent mismatches.
//!
//! ## String content
//!
//! Plugin-returned strings ([`PluginInfo`],
//! [`PluginResult`], [`TaskPushResult`]) may contain
//! arbitrary UTF-8 including control characters. The
//! host is responsible for sanitizing them before
//! writing to a terminal or other interpreter.
//!
//! # Safety
//!
//! This crate contains no `unsafe` code — helper
//! functions operate on safe types (`&CStr` and
//! `CString`) and the plugin author is responsible for
//! the pointer conversions in their `extern "C"`
//! wrappers.

use std::ffi::{CStr, CString, NulError};
use std::os::raw::c_char;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Current plugin API version. Plugins must export
/// `rustwerk_plugin_api_version()` returning this
/// value. The host refuses to load plugins whose API
/// version does not match.
///
/// ## Version history
///
/// - **v1**: initial wire contract.
/// - **v2**: `TaskDto.plugin_state` and
///   `TaskPushResult.plugin_state_update` added to
///   let plugins persist opaque per-task state
///   (e.g. a Jira issue key) across pushes.
pub const API_VERSION: u32 = 2;

/// Return code: success.
pub const ERR_OK: i32 = 0;
/// Return code: unspecified plugin-side error.
pub const ERR_GENERIC: i32 = 1;
/// Return code: host-supplied JSON failed validation.
pub const ERR_INVALID_INPUT: i32 = 2;
/// Return code: plugin detected an incompatible host
/// version.
pub const ERR_VERSION_MISMATCH: i32 = 3;

// ---------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------

/// Metadata describing a plugin. Returned by
/// `rustwerk_plugin_info`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInfo {
    /// Human-readable plugin name (e.g. `"jira"`).
    pub name: String,
    /// Plugin version string (e.g. `"0.1.0"`).
    pub version: String,
    /// Free-form description shown to users.
    pub description: String,
    /// Declared capabilities — lowercase ASCII
    /// identifiers. Well-known value: `"push_tasks"`.
    /// Unknown capabilities are ignored by the host.
    pub capabilities: Vec<String>,
}

/// Result of pushing a single task to an external
/// system.
///
/// Prefer [`TaskPushResult::ok`] and
/// [`TaskPushResult::fail`] over struct-literal
/// construction so future optional fields land
/// without mass-editing every call site.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskPushResult {
    /// Rustwerk task ID (e.g. `"PLG-API"`).
    pub task_id: String,
    /// Whether the push succeeded.
    pub success: bool,
    /// Human-readable message (success or error).
    pub message: String,
    /// External-system identifier assigned to the
    /// task, if any (e.g. `"PROJ-123"`).
    pub external_key: Option<String>,
    /// Opaque plugin-specific state the host should
    /// persist under `plugin_state.<plugin-name>` for
    /// this task. `None` (or field absent on the wire)
    /// means "leave the stored state unchanged";
    /// `Some(v)` replaces it with `v`. There is no
    /// "clear the entry" variant in API v2 — if a
    /// future plugin needs one, the contract extends
    /// in v3.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_state_update: Option<Value>,
}

impl TaskPushResult {
    /// Build a successful per-task result. Defaults
    /// `external_key` and `plugin_state_update` to
    /// `None`; use the fluent setters below to attach
    /// them.
    pub fn ok(task_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            success: true,
            message: message.into(),
            external_key: None,
            plugin_state_update: None,
        }
    }

    /// Build a failed per-task result. `external_key`
    /// and `plugin_state_update` default to `None` —
    /// a failure by definition has no external
    /// identifier and should not overwrite stored
    /// state.
    pub fn fail(task_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            success: false,
            message: error.into(),
            external_key: None,
            plugin_state_update: None,
        }
    }

    /// Attach an external-system identifier (e.g. a
    /// Jira issue key). Consumes and returns `self`
    /// so setters chain fluently.
    #[must_use]
    pub fn with_external_key(mut self, key: impl Into<String>) -> Self {
        self.external_key = Some(key.into());
        self
    }

    /// Attach a plugin-state update blob. The host
    /// persists this under `plugin_state.<plugin-name>`
    /// for this task; see [`TaskPushResult::plugin_state_update`]
    /// for the semantics.
    #[must_use]
    pub fn with_plugin_state_update(mut self, state: Value) -> Self {
        self.plugin_state_update = Some(state);
        self
    }
}

/// Aggregate result returned by a plugin operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginResult {
    /// Whether the operation as a whole succeeded.
    pub success: bool,
    /// Human-readable top-level message.
    pub message: String,
    /// Per-task results. `None` when the operation
    /// does not produce per-task output;
    /// `Some(vec![])` when it does but no tasks were
    /// processed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_results: Option<Vec<TaskPushResult>>,
}

/// Task status, mirroring the host domain `Status`
/// enum with a stable wire format.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatusDto {
    /// Not yet started.
    Todo,
    /// Currently being worked on.
    InProgress,
    /// Waiting on dependencies or external input.
    Blocked,
    /// Completed.
    Done,
    /// Intentionally deferred.
    OnHold,
}

/// Portable representation of a rustwerk task. Mirrors
/// the host domain `Task` but uses plain strings for
/// portability so plugins need not depend on host
/// types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDto {
    /// Task ID.
    pub id: String,
    /// Short title.
    pub title: String,
    /// Long-form description (may be empty).
    pub description: String,
    /// Task status.
    pub status: TaskStatusDto,
    /// IDs of tasks this task depends on.
    pub dependencies: Vec<String>,
    /// Effort estimate as the host's serialized string
    /// form (e.g. `"2d"`, `"4h"`), if set.
    pub effort_estimate: Option<String>,
    /// Complexity score, if set.
    pub complexity: Option<u32>,
    /// Assigned developer, if any.
    pub assignee: Option<String>,
    /// Free-form tags.
    pub tags: Vec<String>,
    /// Opaque state previously returned by THIS plugin
    /// for THIS task via `plugin_state_update`. `None`
    /// on first push or when no prior state was
    /// recorded. The host slices this in from
    /// `project.json` per-plugin-name namespace before
    /// the call, so a plugin only ever sees its own
    /// entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_state: Option<Value>,
}

// ---------------------------------------------------------------
// FFI function type aliases
// ---------------------------------------------------------------

/// Signature of `rustwerk_plugin_api_version`. Must
/// return [`API_VERSION`]. Not marked `unsafe` because
/// it accepts no pointers and returns a scalar.
pub type PluginApiVersionFn = extern "C" fn() -> u32;

/// Signature of `rustwerk_plugin_info`. Writes a
/// JSON-encoded [`PluginInfo`] to `*out` as a
/// plugin-allocated C string. See the crate-level
/// docs for out-pointer ownership rules.
pub type PluginInfoFn = unsafe extern "C" fn(out: *mut *mut c_char) -> i32;

/// Signature of `rustwerk_plugin_push_tasks`. `config`
/// is a plugin-specific JSON object; `tasks` is a JSON
/// array of [`TaskDto`]. On return, `*out` points to a
/// plugin-allocated JSON [`PluginResult`].
pub type PluginPushTasksFn = unsafe extern "C" fn(
    config: *const c_char,
    tasks: *const c_char,
    out: *mut *mut c_char,
) -> i32;

/// Signature of `rustwerk_plugin_free_string`. Frees a
/// C string previously returned by the plugin via an
/// out-pointer. Passing a null pointer is a no-op.
pub type PluginFreeStringFn = unsafe extern "C" fn(ptr: *mut c_char);

// ---------------------------------------------------------------
// Helper functions for plugin authors
// ---------------------------------------------------------------

/// Errors produced by the helper functions. Inner
/// library error types are held as `#[source]` only so
/// they do not appear in the public API surface.
#[derive(Debug, thiserror::Error)]
pub enum HelperError {
    /// JSON (de)serialization failed.
    #[error("failed to (de)serialize plugin payload as JSON")]
    Json(#[source] serde_json::Error),
    /// Serialized JSON contained an interior null
    /// byte, so it cannot be converted to a `CString`.
    #[error("plugin payload contained an interior null byte")]
    Nul(#[source] NulError),
    /// Input exceeded the caller-supplied size cap.
    #[error("plugin payload exceeds the {limit}-byte size cap")]
    TooLarge {
        /// Size limit that was exceeded.
        limit: usize,
    },
}

impl From<serde_json::Error> for HelperError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

impl From<NulError> for HelperError {
    fn from(e: NulError) -> Self {
        Self::Nul(e)
    }
}

/// Serialize a value to a `CString` suitable for
/// writing into a plugin out-pointer.
///
/// The plugin author converts the resulting `CString`
/// to a raw pointer with `CString::into_raw` inside
/// their `extern "C"` wrapper and assigns it to the
/// host-provided out-pointer.
pub fn serialize_to_cstring<T: Serialize>(
    value: &T,
) -> Result<CString, HelperError> {
    let json = serde_json::to_string(value)?;
    Ok(CString::new(json)?)
}

/// Deserialize a `CStr` (typically reconstructed from
/// a host-provided `*const c_char`) as JSON of type
/// `T`.
///
/// This helper does not cap input size. Use
/// [`deserialize_from_cstr_bounded`] when deserializing
/// buffers that come from a less-trusted side of the
/// FFI boundary to bound memory use.
pub fn deserialize_from_cstr<T: for<'de> Deserialize<'de>>(
    s: &CStr,
) -> Result<T, HelperError> {
    let bytes = s.to_bytes();
    Ok(serde_json::from_slice(bytes)?)
}

/// Deserialize a `CStr` as JSON, rejecting inputs
/// whose byte length exceeds `max_bytes` before
/// attempting to parse. `max_bytes` counts the JSON
/// payload only — the trailing NUL is not included.
pub fn deserialize_from_cstr_bounded<T: for<'de> Deserialize<'de>>(
    s: &CStr,
    max_bytes: usize,
) -> Result<T, HelperError> {
    let bytes = s.to_bytes();
    if bytes.len() > max_bytes {
        return Err(HelperError::TooLarge { limit: max_bytes });
    }
    Ok(serde_json::from_slice(bytes)?)
}

// ---------------------------------------------------------------
// Tests
// ---------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_version_is_two() {
        assert_eq!(API_VERSION, 2);
    }

    #[test]
    fn return_codes_are_stable() {
        assert_eq!(ERR_OK, 0);
        assert_eq!(ERR_GENERIC, 1);
        assert_eq!(ERR_INVALID_INPUT, 2);
        assert_eq!(ERR_VERSION_MISMATCH, 3);
    }

    #[test]
    fn plugin_info_round_trips() {
        let info = PluginInfo {
            name: "jira".into(),
            version: "0.1.0".into(),
            description: "Push tasks to Jira".into(),
            capabilities: vec!["push_tasks".into()],
        };
        let json = serde_json::to_string(&info).unwrap();
        let back: PluginInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, back);
    }

    #[test]
    fn task_push_result_round_trips() {
        let r = TaskPushResult {
            task_id: "PLG-API".into(),
            success: true,
            message: "created".into(),
            external_key: Some("PROJ-123".into()),
            plugin_state_update: None,
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: TaskPushResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn task_push_result_with_no_external_key() {
        let r = TaskPushResult {
            task_id: "X".into(),
            success: false,
            message: "denied".into(),
            external_key: None,
            plugin_state_update: None,
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: TaskPushResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn task_push_result_ok_defaults_optional_fields_to_none() {
        let r = TaskPushResult::ok("T-1", "created");
        assert_eq!(r.task_id, "T-1");
        assert!(r.success);
        assert_eq!(r.message, "created");
        assert!(r.external_key.is_none());
        assert!(r.plugin_state_update.is_none());
    }

    #[test]
    fn task_push_result_fail_defaults_optional_fields_to_none() {
        let r = TaskPushResult::fail("T-1", "HTTP 500");
        assert_eq!(r.task_id, "T-1");
        assert!(!r.success);
        assert_eq!(r.message, "HTTP 500");
        assert!(r.external_key.is_none());
        assert!(r.plugin_state_update.is_none());
    }

    #[test]
    fn task_push_result_with_external_key_chains() {
        let r = TaskPushResult::ok("T-1", "created").with_external_key("PROJ-1");
        assert_eq!(r.external_key.as_deref(), Some("PROJ-1"));
    }

    #[test]
    fn task_push_result_with_plugin_state_update_chains() {
        let r = TaskPushResult::ok("T-1", "created")
            .with_plugin_state_update(serde_json::json!({ "key": "PROJ-1" }));
        assert_eq!(
            r.plugin_state_update,
            Some(serde_json::json!({ "key": "PROJ-1" }))
        );
    }

    #[test]
    fn task_push_result_carries_plugin_state_update() {
        let r = TaskPushResult {
            task_id: "PLG-API".into(),
            success: true,
            message: "created".into(),
            external_key: Some("PROJ-1".into()),
            plugin_state_update: Some(serde_json::json!({
                "key": "PROJ-1",
                "last_pushed_at": "2026-04-20T12:00:00Z"
            })),
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: TaskPushResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
        assert!(json.contains("plugin_state_update"));
    }

    #[test]
    fn task_push_result_omits_plugin_state_update_when_none() {
        let r = TaskPushResult {
            task_id: "X".into(),
            success: true,
            message: "m".into(),
            external_key: None,
            plugin_state_update: None,
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("plugin_state_update"), "got: {json}");
    }

    #[test]
    fn task_push_result_accepts_missing_plugin_state_update() {
        let json = r#"{"task_id":"X","success":true,"message":"m","external_key":null}"#;
        let r: TaskPushResult = serde_json::from_str(json).unwrap();
        assert!(r.plugin_state_update.is_none());
    }

    #[test]
    fn plugin_result_round_trips_with_results() {
        let r = PluginResult {
            success: true,
            message: "ok".into(),
            task_results: Some(vec![TaskPushResult {
                task_id: "A".into(),
                success: true,
                message: "m".into(),
                external_key: None,
                plugin_state_update: None,
            }]),
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: PluginResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn plugin_result_omits_absent_task_results() {
        let r = PluginResult {
            success: true,
            message: "ok".into(),
            task_results: None,
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("task_results"));
        let back: PluginResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn plugin_result_distinguishes_empty_from_absent() {
        let empty = PluginResult {
            success: true,
            message: "ran".into(),
            task_results: Some(vec![]),
        };
        let absent = PluginResult {
            success: true,
            message: "no-op".into(),
            task_results: None,
        };
        assert_ne!(
            serde_json::to_string(&empty).unwrap(),
            serde_json::to_string(&absent).unwrap()
        );
    }

    #[test]
    fn task_status_dto_round_trips_all_variants() {
        for (variant, expected) in [
            (TaskStatusDto::Todo, "\"todo\""),
            (TaskStatusDto::InProgress, "\"in_progress\""),
            (TaskStatusDto::Blocked, "\"blocked\""),
            (TaskStatusDto::Done, "\"done\""),
            (TaskStatusDto::OnHold, "\"on_hold\""),
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            assert_eq!(json, expected);
            let back: TaskStatusDto = serde_json::from_str(&json).unwrap();
            assert_eq!(back, variant);
        }
    }

    #[test]
    fn task_dto_round_trips() {
        let t = TaskDto {
            id: "PLG-API".into(),
            title: "Plugin API".into(),
            description: "desc".into(),
            status: TaskStatusDto::InProgress,
            dependencies: vec!["PLG-WORKSPACE".into()],
            effort_estimate: Some("5d".into()),
            complexity: Some(5),
            assignee: Some("igor".into()),
            tags: vec!["plugin".into(), "api".into()],
            plugin_state: None,
        };
        let json = serde_json::to_string(&t).unwrap();
        let back: TaskDto = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn task_dto_with_optional_fields_none() {
        let t = TaskDto {
            id: "T".into(),
            title: "t".into(),
            description: String::new(),
            status: TaskStatusDto::Todo,
            dependencies: vec![],
            effort_estimate: None,
            complexity: None,
            assignee: None,
            tags: vec![],
            plugin_state: None,
        };
        let json = serde_json::to_string(&t).unwrap();
        let back: TaskDto = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn task_dto_carries_plugin_state() {
        let t = TaskDto {
            id: "PLG-JIRA".into(),
            title: "t".into(),
            description: String::new(),
            status: TaskStatusDto::Done,
            dependencies: vec![],
            effort_estimate: None,
            complexity: None,
            assignee: None,
            tags: vec![],
            plugin_state: Some(serde_json::json!({ "key": "PROJ-7" })),
        };
        let json = serde_json::to_string(&t).unwrap();
        let back: TaskDto = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
        assert!(json.contains("plugin_state"));
    }

    #[test]
    fn task_dto_omits_plugin_state_when_none() {
        let t = TaskDto {
            id: "T".into(),
            title: "t".into(),
            description: String::new(),
            status: TaskStatusDto::Todo,
            dependencies: vec![],
            effort_estimate: None,
            complexity: None,
            assignee: None,
            tags: vec![],
            plugin_state: None,
        };
        let json = serde_json::to_string(&t).unwrap();
        assert!(!json.contains("plugin_state"), "got: {json}");
    }

    #[test]
    fn task_dto_accepts_missing_plugin_state() {
        let json = r#"{
            "id":"T","title":"t","description":"",
            "status":"todo","dependencies":[],
            "effort_estimate":null,"complexity":null,
            "assignee":null,"tags":[]
        }"#;
        let t: TaskDto = serde_json::from_str(json).unwrap();
        assert!(t.plugin_state.is_none());
    }

    #[test]
    fn serialize_to_cstring_produces_valid_json() {
        let info = PluginInfo {
            name: "n".into(),
            version: "v".into(),
            description: "d".into(),
            capabilities: vec![],
        };
        let cs = serialize_to_cstring(&info).unwrap();
        let back: PluginInfo =
            serde_json::from_slice(cs.as_bytes()).unwrap();
        assert_eq!(info, back);
    }

    #[test]
    fn deserialize_from_cstr_reads_json() {
        let src = PluginInfo {
            name: "n".into(),
            version: "v".into(),
            description: "d".into(),
            capabilities: vec!["push_tasks".into()],
        };
        let cs = serialize_to_cstring(&src).unwrap();
        let back: PluginInfo = deserialize_from_cstr(cs.as_c_str()).unwrap();
        assert_eq!(src, back);
    }

    #[test]
    fn deserialize_from_cstr_rejects_invalid_json() {
        let cs = CString::new("not json").unwrap();
        let err = deserialize_from_cstr::<PluginInfo>(cs.as_c_str());
        assert!(matches!(err, Err(HelperError::Json(_))));
    }

    #[test]
    fn bounded_deserialize_accepts_within_limit() {
        let info = PluginInfo {
            name: "n".into(),
            version: "v".into(),
            description: "d".into(),
            capabilities: vec![],
        };
        let cs = serialize_to_cstring(&info).unwrap();
        let len = cs.as_bytes().len();
        let back: PluginInfo =
            deserialize_from_cstr_bounded(cs.as_c_str(), len).unwrap();
        assert_eq!(back, info);
    }

    #[test]
    fn bounded_deserialize_rejects_over_limit() {
        let info = PluginInfo {
            name: "n".into(),
            version: "v".into(),
            description: "d".into(),
            capabilities: vec![],
        };
        let cs = serialize_to_cstring(&info).unwrap();
        let len = cs.as_bytes().len();
        let err = deserialize_from_cstr_bounded::<PluginInfo>(
            cs.as_c_str(),
            len - 1,
        );
        assert!(matches!(err, Err(HelperError::TooLarge { limit }) if limit == len - 1));
    }

    #[test]
    fn helper_error_display_is_stable() {
        let err = HelperError::TooLarge { limit: 42 };
        assert_eq!(
            format!("{err}"),
            "plugin payload exceeds the 42-byte size cap"
        );
    }
}

# PLG-API: Plugin API crate with DTOs and FFI contract

## Why

Plugins (dynamic libraries) and the host binary need a
shared interface. A separate crate avoids the plugin
depending on the full rustwerk crate. The FFI contract
uses JSON strings over C-compatible function pointers,
keeping the unsafe surface minimal and the ABI stable.

## What

Create `crates/rustwerk-plugin-api/src/lib.rs` with:

### DTOs (serde-serializable)

- **`PluginInfo`**: name, version, description,
  capabilities (Vec<String>)
- **`PluginResult`**: success (bool), message,
  task_results (Vec<TaskPushResult>)
- **`TaskPushResult`**: task_id, success, message,
  external_key (Option — e.g. "PROJ-123")
- **`TaskDto`**: id, title, description, status,
  dependencies, effort_estimate, complexity, assignee,
  tags — mirrors the domain Task but as plain strings
  for portability

### Constants

- `API_VERSION: u32 = 1`

### FFI function type aliases

Document the 4 `extern "C"` functions a plugin must
export:

```
rustwerk_plugin_api_version() -> u32
rustwerk_plugin_info(out: *mut *mut c_char) -> i32
rustwerk_plugin_push_tasks(
    config: *const c_char,
    tasks: *const c_char,
    out: *mut *mut c_char,
) -> i32
rustwerk_plugin_free_string(ptr: *mut c_char)
```

Return code convention: 0 = success, non-zero = error.
On error, `out` may contain a JSON error message or be
null.

### Helper functions for plugin authors

Provide safe wrappers that plugin crates call from
their `extern "C"` exports to reduce boilerplate:
- `write_json_to_out_ptr(value, out)` — serializes a
  serde value to a CString and writes to the out
  pointer
- `read_json_from_ptr<T>(ptr)` — reads a C string
  pointer and deserializes to T

## How

- Single file: `crates/rustwerk-plugin-api/src/lib.rs`
- Dependencies: serde, serde_json only
- No unsafe in this crate (helpers use raw pointers
  but are called from unsafe contexts in the plugin)

## Acceptance criteria

- [ ] All DTOs serialize/deserialize round-trip
      (unit tests)
- [ ] `API_VERSION` is 1
- [ ] Doc comments on all public items
- [ ] `cargo xtask clippy` passes
- [ ] No unsafe code in this crate

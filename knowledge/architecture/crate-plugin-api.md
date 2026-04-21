+++
title = "crate: rustwerk-plugin-api"
date = 2026-04-21
description = "The stable FFI contract between host and plugins."

[taxonomies]
tags = ["crate", "ffi", "plugin"]

[extra]
note_type = "architecture"
links = [
  { relation = "part-of", target = "architecture/workspace" },
  { relation = "implements", target = "decisions/ffi-plugin-boundary" },
  { relation = "relates-to", target = "architecture/plugin-host" },
  { relation = "relates-to", target = "architecture/crate-jira-plugin" },
  { relation = "relates-to", target = "concepts/ffi" },
]
+++

A tiny, dependency-light crate (`serde`, `serde_json`,
`thiserror`) that defines the wire format and the four
`extern "C"` functions a plugin must export:

```text
rustwerk_plugin_api_version()    -> u32
rustwerk_plugin_info(out)        -> i32
rustwerk_plugin_push_tasks(...)  -> i32
rustwerk_plugin_free_string(ptr)
```

Data crosses the FFI boundary as JSON-encoded,
null-terminated C strings — never Rust types. This is
the contract that makes cross-compiler and cross-build
compatibility possible.

## Invariants enforced by the API

- **Version-first.** The host must call
  `rustwerk_plugin_api_version()` before anything else
  and unload the plugin on mismatch.
- **Symmetric allocation.** Strings returned from a
  plugin are freed by the plugin via
  `rustwerk_plugin_free_string`, so allocations never
  cross allocators.
- **Null-on-error allowed.** On a non-zero return the
  out-pointer may be null or carry a JSON error body;
  the host must handle both.

See [FFI plugin boundary](@/decisions/ffi-plugin-boundary.md)
for the rationale and
[plugin host](@/architecture/plugin-host.md) for how
the invariants are checked at runtime.

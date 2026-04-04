# PLG-HOST: Plugin host with libloading dynamic loader

## Why

The host binary needs to discover, load, and call
plugins at runtime without compile-time coupling.
Dynamic loading via `libloading` lets the binary work
with or without plugins present.

## What

New file: `crates/rustwerk/src/bin/rustwerk/plugin_host.rs`

### Plugin discovery

Scan these directories for dynamic libraries:
1. `<project_root>/.rustwerk/plugins/`
2. `~/.rustwerk/plugins/` (user-global)
3. `target/debug/` and `target/release/` (dev
   convenience)

Filter by platform extension:
- Windows: `.dll`
- Linux: `.so`
- macOS: `.dylib`

### Loading sequence

For each discovered library:
1. Load with `libloading::Library::new()`
2. Call `rustwerk_plugin_api_version()` — reject if
   != `API_VERSION`
3. Call `rustwerk_plugin_info()` — parse JSON to
   `PluginInfo`
4. Index by `PluginInfo.name`

### `LoadedPlugin` struct

```rust
pub struct LoadedPlugin {
    _library: Library,       // prevent drop
    info: PluginInfo,
    // Cached function pointers
    push_tasks_fn: ...,
    free_string_fn: ...,
}
```

### Public API

- `discover_plugins(project_root) -> Vec<LoadedPlugin>`
- `LoadedPlugin::info() -> &PluginInfo`
- `LoadedPlugin::push_tasks(config_json, tasks_json)
    -> Result<PluginResult>`

### Memory safety

All strings returned by plugins are freed via the
plugin's own `rustwerk_plugin_free_string`. This is
critical on Windows where each DLL has its own heap.

### Unsafe scoping

Only this single module has `#[allow(unsafe_code)]`.
The rest of the binary remains `forbid(unsafe_code)`.

## How

- `plugin_host.rs` with `#[allow(unsafe_code)]`
  attribute at module level
- Feature-gated behind `#[cfg(feature = "plugins")]`
- Error handling: `anyhow::Result` wrapping load
  failures, version mismatches, JSON parse errors

## Acceptance criteria

- [ ] Discovers plugins from all 3 directories
- [ ] Rejects plugins with wrong API version
      (test with a mock or version check)
- [ ] Binary compiles and runs with no plugins present
- [ ] Binary compiles with `--no-default-features`
      (plugins feature disabled)
- [ ] All unsafe confined to `plugin_host.rs`
- [ ] Strings from plugins freed correctly (no leaks)
- [ ] `cargo xtask clippy` passes

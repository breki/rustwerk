+++
title = "Plugin Host"
date = 2026-04-21
description = "The only unsafe module — loads cdylibs and calls their exports."

[taxonomies]
tags = ["plugin", "ffi", "unsafe"]

[extra]
note_type = "architecture"
links = [
  { relation = "part-of", target = "architecture/crate-rustwerk" },
  { relation = "depends-on", target = "architecture/crate-plugin-api" },
  { relation = "implements", target = "decisions/ffi-plugin-boundary" },
  { relation = "relates-to", target = "concepts/ffi" },
  { relation = "relates-to", target = "concepts/cdylib" },
]
+++

`src/bin/rustwerk/plugin_host.rs` is the sole location
where rustwerk opts into `unsafe_code`. It is gated
behind `#[cfg(feature = "plugins")]` and its
responsibilities are:

1. Discover `.dll`/`.so`/`.dylib` files under
   `.rustwerk/plugins/` (project) and
   `~/.rustwerk/plugins/` (user).
2. `dlopen` each via `libloading`.
3. Call `rustwerk_plugin_api_version()` first — unload
   on mismatch before touching any other symbol.
4. Serialize config + tasks to JSON and invoke
   `rustwerk_plugin_push_tasks`.
5. Free the returned string with the plugin's own
   `rustwerk_plugin_free_string`.

Everything above the unsafe boundary sees safe Rust
types. Everything below sees null-terminated JSON
strings. Nothing in between.

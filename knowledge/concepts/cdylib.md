+++
title = "cdylib (C-compatible dynamic library)"
date = 2026-04-21
description = "The Cargo crate type rustwerk plugins are built as — a dynamic library that exposes a C ABI."

[taxonomies]
tags = ["ffi", "plugin", "cargo", "glossary"]

[extra]
note_type = "concept"
links = [
  { relation = "relates-to", target = "concepts/ffi" },
  { relation = "relates-to", target = "architecture/crate-plugin-api" },
  { relation = "relates-to", target = "architecture/plugin-host" },
  { relation = "relates-to", target = "architecture/crate-jira-plugin" },
  { relation = "relates-to", target = "decisions/ffi-plugin-boundary" },
]
+++

A **cdylib** is a Cargo crate type — a **C-compatible
dynamic library**. It is one of the five values the
`crate-type` key in `Cargo.toml` can take
(`bin`, `lib`, `rlib`, `dylib`, `cdylib`, `staticlib`),
and it is the one rustwerk plugins are built as.

```toml
[lib]
crate-type = ["cdylib"]
```

## What Cargo actually produces

| Platform | Output filename |
|---|---|
| Windows | `rustwerk_jira_plugin.dll` |
| Linux | `librustwerk_jira_plugin.so` |
| macOS | `librustwerk_jira_plugin.dylib` |

These files live under `target/<profile>/`. The
rustwerk host discovers them under
`.rustwerk/plugins/` (project-local) or
`~/.rustwerk/plugins/` (user-global) and loads them
with the OS's dynamic linker (`dlopen` on Unix,
`LoadLibrary` on Windows).

## cdylib vs dylib — the key distinction

Both produce dynamic libraries, but:

- **`dylib`** is Rust's native dynamic-library format.
  It can export Rust types, generics, and trait
  objects — but only if the consumer is compiled by
  the *same* rustc version. There is no stable Rust
  ABI, so `dylib` outputs are brittle across builds.
- **`cdylib`** is a dynamic library with the
  **C ABI** on its boundary. It exports only the
  symbols you mark `#[no_mangle] pub extern "C"`,
  trims the Rust runtime, and can be consumed by any
  language that can talk C — including a future
  rustwerk host built with a newer rustc.

For a plugin that must survive compiler upgrades and
potentially be written in a non-Rust language, cdylib
is the only safe choice. See
[FFI plugin boundary](@/decisions/ffi-plugin-boundary.md).

## Implications on the plugin code

- Exports must be `#[no_mangle] pub extern "C"` — if
  you forget `no_mangle`, Rust's name-mangling
  scheme hides the symbol and the host's
  `dlsym`/`GetProcAddress` lookup returns null.
- Panics must not unwind across the boundary. The
  rustwerk plugin crates set
  `panic = "abort"` (or equivalent) so a bug in
  a plugin terminates the process rather than
  producing undefined behavior.
- The host calls the plugin's
  `rustwerk_plugin_free_string` for any allocation
  the plugin returned — see
  [FFI concept note](@/concepts/ffi.md).

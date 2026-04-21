+++
title = "Plugins are cdylibs that talk JSON over C strings"
date = 2026-04-21
description = "Dynamic libraries, minimal FFI contract, symmetric allocation."

[taxonomies]
tags = ["plugin", "ffi", "unsafe"]

[extra]
note_type = "decision"
links = [
  { relation = "implements", target = "architecture/crate-plugin-api" },
  { relation = "implements", target = "architecture/plugin-host" },
  { relation = "relates-to", target = "architecture/crate-jira-plugin" },
  { relation = "relates-to", target = "concepts/ffi" },
  { relation = "relates-to", target = "concepts/cdylib" },
]
+++

**Decision.** Plugins are `cdylib` dynamic libraries
that export four `extern "C"` functions. All data
crosses the boundary as JSON-encoded,
null-terminated C strings. Each side frees what it
allocated.

## Why not a Rust trait object?

- **ABI instability.** Rust has no stable ABI. A
  plugin built against one rustc would break against
  another. A C ABI is forever.
- **Language neutrality.** A Go or C++ plugin can
  implement the contract without linking to Rust.
- **Version negotiation.** A single exported
  `rustwerk_plugin_api_version()` lets the host
  reject incompatible plugins before it calls any
  risky entry point.

## Why JSON, not a bespoke binary format?

- **Debuggable.** A failing plugin call can be
  reproduced by piping the captured JSON to a test.
- **Agent-friendly.** The same JSON shape appears in
  `--json` CLI output, `batch` files, and plugin
  invocations.

## Cost

One unsafe module (`plugin_host.rs`), gated behind a
feature flag, with allocation symmetry and null-safety
checks in place. See
[Plugin host](@/architecture/plugin-host.md).

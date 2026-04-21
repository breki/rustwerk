+++
title = "Cargo Workspace"
date = 2026-04-21
description = "Three production crates plus xtask, glued by workspace lints."

[taxonomies]
tags = ["cargo", "workspace", "build"]

[extra]
note_type = "architecture"
links = [
  { relation = "part-of", target = "architecture/crate-rustwerk" },
  { relation = "part-of", target = "architecture/crate-plugin-api" },
  { relation = "part-of", target = "architecture/crate-jira-plugin" },
  { relation = "part-of", target = "architecture/xtask" },
]
+++

The workspace is declared in `Cargo.toml` at the repo
root and has four members:

| Member | Kind | Purpose |
|---|---|---|
| `crates/rustwerk` | binary + lib | CLI entry point and domain library |
| `crates/rustwerk-plugin-api` | lib | FFI contract between host and plugins |
| `crates/rustwerk-jira-plugin` | `cdylib` | Jira integration plugin |
| `xtask` | binary | Build/test/lint runner |

## Workspace lints

`[workspace.lints]` enforces repo-wide policy:

- `warnings = "deny"` — no warnings may land on main.
- `unsafe_code = "forbid"` at the workspace level, and
  re-declared as `deny` inside `crates/rustwerk` so the
  single unsafe module (`plugin_host.rs`) can opt in
  with `#![allow(unsafe_code)]`.
- `clippy::pedantic` at `warn` with a short deny-list
  of lints that the project explicitly accepts.

This split matters because the plugin host needs
`libloading`, which transitively requires unsafe calls.
See [FFI plugin boundary](@/decisions/ffi-plugin-boundary.md)
for why that isolation strategy was chosen.

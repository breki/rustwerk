+++
title = "crate: rustwerk-jira-plugin"
date = 2026-04-21
description = "The reference plugin: push rustwerk tasks into Jira Cloud."

[taxonomies]
tags = ["crate", "plugin", "jira", "integration"]

[extra]
note_type = "architecture"
links = [
  { relation = "part-of", target = "architecture/workspace" },
  { relation = "depends-on", target = "architecture/crate-plugin-api" },
  { relation = "implements", target = "integrations/jira-plugin" },
  { relation = "relates-to", target = "concepts/ffi" },
  { relation = "relates-to", target = "concepts/cdylib" },
]
+++

A `cdylib` that exports the four API functions from
[rustwerk-plugin-api](@/architecture/crate-plugin-api.md).
The host discovers the built dynamic library under
`.rustwerk/plugins/` (project-local) or
`~/.rustwerk/plugins/` (user-global) and calls into it
from `rustwerk plugin push jira`.

## Source layout

| File | Responsibility |
|---|---|
| `lib.rs` | FFI exports, version check, error plumbing |
| `config.rs` | Parse and validate the plugin config JSON |
| `mapping.rs` | Build a Jira issue payload from a `Task` |
| `jira_client.rs` | HTTP client with gateway fallback |
| `transition.rs` | Status → Jira workflow transitions |
| `push.rs` | Orchestrates per-task push, level by level |
| `warnings.rs` | Collect non-fatal warnings for the host |
| `test_support.rs` | Shared fixtures for integration tests |

## Level-by-level push

Parent and epic links must exist before children
reference them. The plugin builds a topological order
by parent depth and pushes one level at a time. This
gives deterministic output and avoids partially-linked
state on failure. See
[jira plugin](@/integrations/jira-plugin.md).

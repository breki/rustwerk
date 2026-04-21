+++
title = "CLI Layer"
date = 2026-04-21
description = "clap parser, subcommands, and human/JSON rendering."

[taxonomies]
tags = ["cli", "clap"]

[extra]
note_type = "architecture"
links = [
  { relation = "part-of", target = "architecture/crate-rustwerk" },
  { relation = "depends-on", target = "architecture/domain-layer" },
  { relation = "depends-on", target = "architecture/persistence-layer" },
  { relation = "implements", target = "decisions/json-io-agent-friendly" },
]
+++

`src/bin/rustwerk/` owns everything the user sees and
nothing they do not: argument parsing, subcommand
dispatch, and output formatting.

## Layout

| File/dir | Purpose |
|---|---|
| `main.rs` | `clap` `Cli` struct and top-level dispatch |
| `commands/` | One file per subcommand family (task, dev, effort, report, plugin) |
| `batch.rs` | Replays a JSON file of commands as a single transaction |
| `gantt.rs` | ASCII Gantt rendering |
| `tree.rs` | Dependency tree rendering |
| `render.rs` | Shared human/JSON output helpers |
| `git.rs` | Locate the `.rustwerk/` directory relative to the git root |
| `plugin_host.rs` | `#[cfg(feature = "plugins")]` dynamic loader |

## The `--json` flag

`Cli.json` is declared `global = true` so every
subcommand accepts it. Agents opt in to structured
output by passing `--json`; humans get the default
table/tree rendering. Every command must support both
paths — see
[JSON I/O is agent-friendly](@/decisions/json-io-agent-friendly.md).

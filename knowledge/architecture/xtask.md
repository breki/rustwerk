+++
title = "xtask build runner"
date = 2026-04-21
description = "The single entry point for all build, test, and lint actions."

[taxonomies]
tags = ["build", "tooling"]

[extra]
note_type = "architecture"
links = [
  { relation = "part-of", target = "architecture/workspace" },
  { relation = "implements", target = "decisions/xtask-build-wrapper" },
]
+++

`xtask/` is a plain Rust binary that wraps `cargo` with
project-specific defaults. CLAUDE.md mandates it:

> Never use raw `cargo test` or `cargo clippy` — always
> go through `xtask`.

## Commands

| Command | What it does |
|---|---|
| `cargo xtask check` | Fast compile, no tests |
| `cargo xtask test [filter]` | `cargo test` with the workspace filter |
| `cargo xtask clippy` | Pedantic clippy, treat warnings as errors |
| `cargo xtask validate` | clippy + tests + coverage (≥90%) |
| `cargo xtask coverage` | Coverage via `cargo-llvm-cov` |
| `cargo xtask fmt` | Format check |

This wrapper is what lets CI and local runs stay
identical: the flags, feature sets, and thresholds live
in one Rust file instead of scattered across shell
scripts and `.github/workflows/*.yml`.

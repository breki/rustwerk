+++
title = "Use xtask as the only build entry point"
date = 2026-04-21
description = "One Rust binary wraps cargo; CI and local runs stay identical."

[taxonomies]
tags = ["build", "tooling"]

[extra]
note_type = "decision"
links = [
  { relation = "implements", target = "architecture/xtask" },
]
+++

**Decision.** Developers and CI invoke build actions
only via `cargo xtask <cmd>`, never `cargo test` or
`cargo clippy` directly. CLAUDE.md calls this out
explicitly.

## Why

- **Single source of truth.** Flag sets (`--all-features`,
  `-- --nocapture`), thresholds (coverage ≥ 90%), and
  the order of checks live in one Rust file rather
  than scattered across shell scripts, `Makefile`,
  and CI YAML.
- **Cross-platform.** The wrapper is Rust, so it runs
  identically on Windows, Linux, and macOS, avoiding
  shell-ism drift.
- **Refactorable.** Because `xtask` is itself a Cargo
  binary in the workspace, changes ship via the same
  review process as production code.

## Consequence

Anyone tempted to run `cargo test` must add the use
case to `xtask` instead. Agents that shell out to Rust
tooling must go through `cargo xtask ...`.

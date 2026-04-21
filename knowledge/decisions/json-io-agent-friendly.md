+++
title = "Every command supports --json"
date = 2026-04-21
description = "Agents read and write structured data; humans read tables."

[taxonomies]
tags = ["cli", "ai-agents"]

[extra]
note_type = "decision"
links = [
  { relation = "relates-to", target = "architecture/cli-layer" },
  { relation = "relates-to", target = "concepts/wbs" },
]
+++

**Decision.** A global `--json` flag toggles every
subcommand between a human-facing table/tree and a
machine-readable JSON payload. Input via `batch` is
also JSON.

## Why

The primary integration target is an AI coding agent.
Agents are far more reliable when they can:

- **Parse** structured output (no regex over
  table layout).
- **Produce** structured input (one schema, one
  validator).

Humans remain first-class — the default output is
still a terminal-friendly table — but the data path
for tools is always JSON.

## Consequence

Every new subcommand must render both paths, and every
field the human version shows must appear in the JSON
version as well. Reviewers should reject PRs that add
a command without a `--json` mode.

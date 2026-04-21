+++
title = "Store project state as JSON in git"
date = 2026-04-21
description = "A single .rustwerk/project.json, versioned alongside the code."

[taxonomies]
tags = ["persistence", "git"]

[extra]
note_type = "decision"
links = [
  { relation = "implements", target = "architecture/persistence-layer" },
  { relation = "relates-to", target = "decisions/json-io-agent-friendly" },
]
+++

**Decision.** Project state lives in one pretty-printed
JSON file (`.rustwerk/project.json`) that is committed
to the same repo as the code.

## Why

- **State travels with code.** When you check out a
  branch or old commit, you get the WBS as it existed
  then — without a separate "project DB" migration
  dance.
- **Review-friendly.** A PR that adds tasks shows
  up as a reviewable diff. Stable field ordering
  (`BTreeMap` in the domain model) makes those diffs
  minimal.
- **Zero infrastructure.** No database daemon, no
  cloud account, no auth story. This keeps the CLI a
  single portable binary.
- **Plays with git.** `git log .rustwerk/project.json`
  is a free audit trail.

## Trade-offs

- Merge conflicts on concurrent edits are likely. The
  remedy is discipline — one person owns the WBS at a
  time — not a locking protocol.
- Very large projects (thousands of tasks) will strain
  JSON parsing. If that becomes real, the next
  iteration is to shard by parent, not to adopt a
  binary format.

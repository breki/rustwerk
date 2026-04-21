+++
title = "Task"
date = 2026-04-21
description = "The atomic unit of work in a rustwerk project."

[taxonomies]
tags = ["domain", "core"]

[extra]
note_type = "concept"
links = [
  { relation = "part-of", target = "concepts/wbs" },
  { relation = "relates-to", target = "concepts/dependencies" },
  { relation = "relates-to", target = "concepts/effort" },
  { relation = "relates-to", target = "concepts/developer" },
  { relation = "implements", target = "architecture/domain-layer" },
]
+++

A **Task** has an identity (`TaskId`), a human title,
optional parent, a list of `dependencies` on other task
IDs, an optional `assignee`, a `Status`, an effort
estimate, a log of effort entries, and a
free-form metadata bag.

## TaskId

IDs are user-supplied mnemonics (e.g. `AUTH-LOGIN`).
`TaskId::new` enforces:

- Non-empty.
- ASCII alphanumerics, hyphens, and underscores only.
- Not a Windows reserved device name (`CON`, `NUL`,
  `COM1`..`COM9`, `LPT1`..`LPT9`, ...), because the
  filesystem cannot host a regular file by that name.

That last constraint applies on all platforms so a
project created on Linux still checks out cleanly on
Windows.

## Status

`TODO`, `IN_PROGRESS`, `BLOCKED`, `DONE` — a small
fixed enum, serialized as strings. Transition rules are
enforced on the domain side; the CLI is a thin
pass-through.

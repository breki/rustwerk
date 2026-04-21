+++
title = "Work Breakdown Structure (WBS)"
date = 2026-04-21
description = "The tree/graph of tasks that makes up a project."

[taxonomies]
tags = ["domain", "core"]

[extra]
note_type = "concept"
links = [
  { relation = "relates-to", target = "concepts/task" },
  { relation = "relates-to", target = "concepts/dependencies" },
  { relation = "relates-to", target = "concepts/critical-path" },
  { relation = "relates-to", target = "concepts/gantt" },
]
+++

A **WBS** in rustwerk is a `Project` plus its tasks,
arranged as a tree by `parent_id` and as a DAG by
dependencies. These two relations coexist:

- **Parent/child** models hierarchy — an epic contains
  stories, a story contains tasks.
- **Dependencies** model ordering — task B must wait
  for task A.

A leaf (no children) is an executable unit. An internal
node rolls up status, estimated effort, and actual
effort from its descendants.

## Incremental building

The WBS is built command-by-command, not generated in
one shot. An AI agent typically uses `rustwerk batch
--file wbs.json` to add many tasks at once, but each
line of that file is one CLI command with the same
semantics as a manual invocation. This is deliberate —
see [JSON I/O is agent-friendly](@/decisions/json-io-agent-friendly.md).

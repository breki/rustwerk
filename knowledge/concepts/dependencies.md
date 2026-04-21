+++
title = "Dependencies"
date = 2026-04-21
description = "Directed edges between tasks — 'B needs A to be DONE first'."

[taxonomies]
tags = ["domain", "scheduling"]

[extra]
note_type = "concept"
links = [
  { relation = "relates-to", target = "concepts/task" },
  { relation = "relates-to", target = "concepts/critical-path" },
  { relation = "relates-to", target = "concepts/gantt" },
]
+++

A dependency is a `TaskId` in another task's
`dependencies` vector. The set of all such edges forms
a **directed acyclic graph** over the project.

The domain layer enforces acyclicity on every mutation:
adding an edge that would close a cycle is rejected
with a `DomainError`. This is what lets scheduling and
critical-path code assume a DAG without defensive cycle
detection at every step.

## Practical implications

- Status rollup: a task is **blocked** if any of its
  direct or transitive dependencies is not `DONE`.
- Scheduling: the forward pass (see
  [Critical Path](@/concepts/critical-path.md)) walks
  the DAG in topological order.

+++
title = "Critical Path"
date = 2026-04-21
description = "The longest path through the dependency DAG — what sets the project's end date."

[taxonomies]
tags = ["scheduling", "analysis"]

[extra]
note_type = "concept"
links = [
  { relation = "relates-to", target = "concepts/dependencies" },
  { relation = "relates-to", target = "concepts/gantt" },
  { relation = "implements", target = "architecture/domain-layer" },
]
+++

The **critical path** is the longest-duration path from
the DAG's sources (tasks with no dependencies) to its
sinks (tasks no one depends on). Slipping any task on
this path slips the whole project; slipping tasks off
the path only consumes slack.

rustwerk computes it by:

1. Topologically sorting tasks (DAG invariant holds by
   construction — see
   [Dependencies](@/concepts/dependencies.md)).
2. Forward pass: earliest start/finish.
3. Backward pass: latest start/finish from a target
   end date.
4. Tasks where earliest == latest are on the critical
   path.

The algorithm lives in `domain/project/critical_path.rs`
and its siblings. The CLI exposes it via
`rustwerk report`, `rustwerk gantt`, and status dashboards.

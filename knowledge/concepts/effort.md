+++
title = "Effort"
date = 2026-04-21
description = "Estimates and actuals, expressed in a compact string form."

[taxonomies]
tags = ["domain"]

[extra]
note_type = "concept"
links = [
  { relation = "relates-to", target = "concepts/task" },
  { relation = "relates-to", target = "concepts/developer" },
  { relation = "relates-to", target = "concepts/gantt" },
]
+++

Effort is stored as a strongly-typed `Effort` value
parsed from strings like `"30M"`, `"2.5H"`, or
`"3D"`. A task carries at most one `effort_estimate`
and an append-only log of `EffortEntry` actuals, each
tagged with the developer and an optional note.

## Why strings

The CLI takes `--effort 2.5H` rather than separate
numeric+unit flags because agents produce and consume
strings more reliably than numeric tuples, and humans
find `2.5H` unambiguous at a glance. Parsing lives on
the domain side so the JSON format stays compact and
portable.

## Rollup

`ProjectSummary` aggregates estimated vs logged effort
at any subtree by walking children. See
[Domain layer](@/architecture/domain-layer.md).

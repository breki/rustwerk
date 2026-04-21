+++
title = "Gantt Chart"
date = 2026-04-21
description = "ASCII schedule visualization driven by the dependency DAG."

[taxonomies]
tags = ["tui", "scheduling"]

[extra]
note_type = "concept"
links = [
  { relation = "relates-to", target = "concepts/critical-path" },
  { relation = "relates-to", target = "concepts/effort" },
  { relation = "relates-to", target = "concepts/dependencies" },
]
+++

`rustwerk gantt` renders a horizontal ASCII Gantt chart
with one row per task. Input: the project's DAG,
per-task effort estimates, assignees, and (optionally)
per-developer daily capacity. Output: terminal-width
bars colored by status with critical-path tasks
highlighted.

## Pipeline

```
Project ──▶ scheduling ──▶ GanttSchedule ──▶ gantt_schedule ──▶ GanttRow[] ──▶ render
```

- `scheduling.rs` is the forward pass over the DAG.
- `gantt_schedule.rs` aligns the pass to calendar days.
- `gantt_row.rs` is the per-task presentation type.
- `bin/rustwerk/gantt.rs` owns the ASCII drawing.

## Flags that change the view

- `--remaining` — hide DONE tasks so the chart reflects
  only work still to come.
- Color auto-detection respects
  `NO_COLOR`, `CLICOLOR`, and TTY state.

See [Gantt Chart in the manual](https://github.com/breki/rustwerk/blob/main/docs/manual.md#gantt-chart).

+++
title = "Domain Layer"
date = 2026-04-21
description = "Pure, I/O-free model for project, task, developer, schedule."

[taxonomies]
tags = ["domain", "ddd"]

[extra]
note_type = "architecture"
links = [
  { relation = "part-of", target = "architecture/crate-rustwerk" },
  { relation = "relates-to", target = "concepts/task" },
  { relation = "relates-to", target = "concepts/wbs" },
  { relation = "relates-to", target = "concepts/developer" },
  { relation = "relates-to", target = "concepts/critical-path" },
  { relation = "relates-to", target = "concepts/gantt" },
]
+++

`crates/rustwerk/src/domain/` is strictly pure: no file
I/O, no network, no clock-reading outside of what the
caller passes in. Everything is a function of arguments
and the `Project` aggregate.

## Module map

```
domain/
├── task.rs           # Task, TaskId, Status, Effort, EffortEntry
├── developer.rs      # Developer, DeveloperId
├── error.rs          # DomainError (thiserror)
└── project/
    ├── mod.rs            # Project aggregate + metadata
    ├── tree.rs           # parent/child tree traversal
    ├── tree_node.rs      # TreeNode view type
    ├── parent.rs         # PushLevels (topological ordering)
    ├── scheduling.rs     # forward-pass schedule calculation
    ├── gantt_schedule.rs # build Gantt rows from schedule
    ├── gantt_row.rs      # per-task schedule row
    ├── critical_path.rs  # longest-path analysis
    ├── bottleneck.rs     # congestion detection
    ├── queries.rs        # common lookups used by CLI
    └── summary.rs        # ProjectSummary for dashboard
```

## Why pure

Because every CLI command reads the project file,
mutates or queries the model, and writes it back,
keeping the domain I/O-free means the CLI layer is the
*only* place that touches the disk, the network, or
the clock. Tests can construct `Project` values in
memory and assert against them directly.

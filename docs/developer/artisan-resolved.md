# Artisan Findings — Resolved

Archive of fixed Artisan code quality findings, newest
first. See [artisan-log.md](artisan-log.md) for open
findings.

---

### AQ-030 — `commands.rs` exceeds 500-line threshold

- **Date:** 2026-04-04
- **Category:** Module Size
- **Commit context:** refactor after v0.32.0
- **Resolution:** Split `commands.rs` (652 lines) into
  five focused modules: `task.rs` (290), `project.rs`
  (145), `report.rs` (177), `dev.rs` (61), `effort.rs`
  (51), with `mod.rs` re-exports. Added error-path
  integration tests and per-module coverage floor (85%).

### AQ-029 — Test does not assert on error message

- **Date:** 2026-04-04
- **Category:** Error Handling
- **Commit context:** v0.32.0 `RUSTWERK_USER` env var
- **Resolution:** Added `stderr.contains("no developer
  specified")` assertion to `task_assign_no_dev_fails`
  test to verify the intended error path triggers.

### AQ-028 — Inconsistent `RUSTWERK_USER` fallback

- **Date:** 2026-04-04
- **Category:** API Design
- **Commit context:** v0.32.0 `RUSTWERK_USER` env var
- **Resolution:** Applied `RUSTWERK_USER` fallback to
  `effort log --dev` (made optional). Extracted shared
  `resolve_developer()` helper used by both `task assign`
  and `effort log` dispatch. Env-var resolution inlined
  in dispatch also resolved (AQ-013 equivalent).

### AQ-020 — scheduling.rs exceeds 500-line module-size rule

- **Date:** 2026-04-03
- **Category:** Module Size
- **Commit context:** v0.30.0 tree command
- **Resolution:** Split scheduling.rs (1,335 lines) into
  five focused modules: `queries.rs` (361), `critical_path.rs`
  (308), `bottleneck.rs` (257), `gantt_schedule.rs` (277),
  `scheduling.rs` (247, kept topo sort + summary). All
  modules now under 400 lines.

### AQ-026 — render_tree writes to stdout, not testable

- **Date:** 2026-04-03
- **Category:** API Design
- **Commit context:** v0.30.0 tree command
- **Resolution:** Changed `render_tree`/`render_node` to
  accept `&mut dyn Write`. Tests now capture output into
  `Vec<u8>` and assert content. Added `render_box_drawing`
  test verifying ├── └── │ characters.

### AQ-025 — build_tree duplicates reverse_dependents

- **Date:** 2026-04-03
- **Category:** Abstraction Boundaries
- **Commit context:** v0.30.0 tree command
- **Resolution:** `build_tree` now calls
  `self.reverse_dependents()` and filters/sorts the result
  instead of building its own map. Made
  `reverse_dependents` `pub(super)`.

### AQ-024 — scheduling.rs now 1,609 lines

- **Date:** 2026-04-03
- **Category:** Module Size
- **Commit context:** v0.30.0 tree command
- **Resolution:** Extracted `task_tree()`,
  `task_tree_remaining()`, `build_tree()`, and
  `build_subtree()` into new `domain/project/tree.rs`
  module with their tests. scheduling.rs: 1,609→1,335.

### AQ-023 — Bottleneck report mislabels ON_HOLD as ready/blocked

- **Date:** 2026-04-03
- **Category:** Type Safety
- **Commit context:** v0.29.0 ON_HOLD status
- **Description:** Bottleneck state label used if/else
  chain that would label ON_HOLD tasks as "ready" or
  "blocked" instead of "on hold".
- **Resolution:** Added explicit `Status::OnHold` branch
  returning "on hold" label.

### AQ-022 — Missing OnHold → InProgress transition

- **Date:** 2026-04-03
- **Category:** API Design
- **Commit context:** v0.29.0 ON_HOLD status
- **Description:** Duplicate of RT-042.
- **Resolution:** Fixed under RT-042.

### AQ-021 — O(V+E) full-graph sort in dependency_chain

- **Date:** 2026-04-03
- **Category:** API Design
- **Commit context:** v0.28.0 task list filters
- **Description:** `dependency_chain()` originally called
  `topological_sort()` on the entire graph to order a
  small subgraph result.
- **Resolution:** Replaced with iterative DFS post-order
  traversal that only visits the reachable subgraph,
  giving O(|subgraph|) instead of O(V+E).

### AQ-019 — Dead guard duplicating domain logic in binary

- **Date:** 2026-04-03
- **Category:** Abstraction Boundaries
- **Commit context:** v0.28.0 task list filters
- **Description:** `cmd_task_list` had an explicit
  `contains_key` check before calling `dependency_chain`,
  duplicating the domain's responsibility for validating
  task existence.
- **Resolution:** Changed `dependency_chain` to return
  `Result<Vec<&TaskId>, DomainError>` with a
  `TaskNotFound` error. Removed the duplicate guard.

### AQ-018 — --status not conflicting with --available/--active

- **Date:** 2026-04-03
- **Category:** API Design
- **Commit context:** v0.28.0 task list filters
- **Description:** Duplicate of RT-037.
- **Resolution:** Fixed under RT-037 (added
  `conflicts_with_all`).

### AQ-017 — Presentation layer reaches into domain internals

- **Date:** 2026-04-03
- **Category:** Abstraction Boundaries
- **Commit context:** v0.27.0 report bottlenecks command
- **Description:** `cmd_report_bottlenecks` accessed
  `project.tasks[&bn.id]` directly to enrich the `Bottleneck`
  with assignee and status, punching through the abstraction.
- **Resolution:** Enriched `Bottleneck` struct with `status`,
  `assignee`, and `ready` fields populated in
  `bottlenecks()`. CLI no longer touches `project.tasks`.

### AQ-016 — Redundant status match duplicates Display impl

- **Date:** 2026-04-03
- **Category:** Type Safety
- **Commit context:** v0.27.0 report bottlenecks command
- **Description:** `cmd_report_bottlenecks` hand-rolled a
  `match` on `Status` to produce display strings, duplicating
  the existing `Display` impl.
- **Resolution:** Now uses `bn.status` directly in the format
  string, which calls `Display` automatically.

### AQ-015 — Module size: scheduling.rs over 1000 lines

- **Date:** 2026-04-03
- **Category:** Module Size
- **Commit context:** v0.26.0 bottleneck detection
- **Description:** `scheduling.rs` exceeded 500 lines with
  `GanttRow` and `ProjectSummary` structs alongside scheduling
  algorithms.
- **Resolution:** Extracted `GanttRow` to `gantt_row.rs` and
  `ProjectSummary` to `summary.rs`. Re-exported from
  `mod.rs` to preserve public API.

### AQ-014 — Tuple return type `(TaskId, usize)` in bottlenecks

- **Date:** 2026-04-03
- **Category:** Type Safety
- **Commit context:** v0.26.0 bottleneck detection
- **Description:** `bottlenecks()` returned `Vec<(TaskId,
  usize)>` — callers would use `.1` for the count with no
  semantic clarity.
- **Resolution:** Introduced `Bottleneck` struct with `id` and
  `downstream_count` fields.

### AQ-013 — Repeated reverse-adjacency graph building

- **Date:** 2026-04-03
- **Category:** API Design
- **Commit context:** v0.26.0 bottleneck detection
- **Description:** The reverse adjacency map was built in three
  places (`topological_sort`, `remaining_critical_path`,
  `bottlenecks`) with slightly different filters, already
  diverging on status semantics.
- **Resolution:** Extracted `reverse_dependents()` private
  helper with a filter predicate. Used in `bottlenecks()`;
  the other two call sites retain their own logic for now
  since they also build `in_degree` maps.

### AQ-012 — Duplicated status-color match in `bar_style`

- **Date:** 2026-04-03
- **Category:** API Design
- **Commit context:** v0.18.0 critical path highlight
- **Description:** `bar_style()` matched on `status`
  twice — once for `base` and again for `critical`. The
  `base` result was discarded in the critical branch.
- **Resolution:** Simplified to `if critical { RED }
  else { match status }` — critical path overrides all
  status colors to RED, eliminating the second match.

### AQ-011 — Module size: `project.rs` and `rustwerk.rs`

- **Date:** 2026-04-03
- **Category:** Module Size
- **Commit context:** refactor split
- **Description:** `project.rs` (1892 lines) and
  `rustwerk.rs` (1529 lines) both exceeded the 500-line
  production code threshold.
- **Resolution:** Split `project.rs` into
  `project/mod.rs` (449 prod) + `project/scheduling.rs`
  (467 prod). Split `rustwerk.rs` into
  `rustwerk/main.rs` (295) + `commands.rs` (362) +
  `batch.rs` (326) + `gantt.rs` (213). All production
  files now under 500 lines.

### AQ-010 — `left_cap` and `right_cap` are constants disguised as methods

- **Date:** 2026-04-03
- **Category:** API Design
- **Commit context:** v0.17.0 VIZ-UNICODE
- **Description:** `left_cap()` and `right_cap()` took
  `&self` but returned the same character regardless of
  status, implying per-row variation that didn't exist.
- **Resolution:** Converted to associated constants
  `GanttRow::LEFT_CAP` and `GanttRow::RIGHT_CAP`.

### AQ-009 — Gantt rendering not testable; coupled to terminal

- **Date:** 2026-04-03
- **Category:** API Design
- **Commit context:** v0.16.0 VIZ-SCALE
- **Description:** `cmd_gantt` mixed I/O (terminal width
  detection, color detection) with rendering logic,
  making the scaling arithmetic untestable.
- **Fix:** Extracted `render_gantt(rows, width, color)`
  as a separate function. `cmd_gantt` is now a thin
  wrapper that loads data and calls `render_gantt`.
  Named constant `FALLBACK_WIDTH` replaces magic 80.
- **Resolved:** 2026-04-03

### AQ-007 — Task::assignee stringly-typed, no referential integrity

- **Date:** 2026-04-03
- **Category:** Type Safety
- **Commit context:** DEV-ASSIGN WBS task
- **Description:** `assign()` accepted raw `&str`, tasks
  could reference non-existent developers, case mismatches
  bypassed the `remove_developer` guard.
- **Fix:** `assign()` now takes `&DeveloperId`, validates
  against `self.developers`. Unregistered developers are
  rejected with `DeveloperNotFound`.
- **Resolved:** 2026-04-03

### AQ-008 — Developer errors reuse `ValidationError`

- **Date:** 2026-04-03
- **Category:** Error Handling
- **Commit context:** v0.15.0 Developer domain type
- **Description:** Developer-related errors used the
  generic `ValidationError(String)` while tasks had
  dedicated `TaskNotFound`/`DuplicateTaskId` variants.
  Callers couldn't match precisely without parsing
  strings.
- **Fix:** Added `DeveloperNotFound(String)` and
  `DeveloperAlreadyExists(String)` variants to
  `DomainError`.
- **Resolved:** 2026-04-03

### AQ-006 — `ansi` module uses `pub` in binary crate

- **Date:** 2026-04-03
- **Category:** Abstraction Boundaries
- **Commit context:** v0.14.0 Gantt colors
- **Description:** `ansi` module constants used `pub`
  visibility inside `src/bin/rustwerk.rs`, but nothing
  outside the file can reach them. Misleading visibility.
- **Fix:** Changed to `pub(super)` to restrict to parent
  module scope.
- **Resolved:** 2026-04-03

### AQ-005 — `--active` output missing critical-path marker

- **Date:** 2026-04-03
- **Category:** Consistency / Abstraction
- **Commit context:** v0.13.1 available/active fix
- **Description:** `--available` showed `*` for critical
  path tasks but `--active` used a hardcoded two-space
  indent. In-progress tasks on the critical path are the
  most schedule-sensitive — dropping the marker misleads
  prioritization.
- **Fix:** Applied same `crit.contains(*id)` marker logic
  to the active branch.
- **Resolved:** 2026-04-03

### AQ-004 — Missing end() accessor on GanttRow

- **Date:** 2026-04-03
- **Category:** Type Safety
- **Commit context:** v0.13.0 Gantt chart
- **Fix:** Added `pub fn end() -> u32` to `GanttRow`.
  CLI uses it instead of `start + width`.
- **Resolved:** 2026-04-03

### AQ-003 — Bar rendering logic in CLI instead of domain

- **Date:** 2026-04-03
- **Category:** Abstraction Boundaries
- **Commit context:** v0.13.0 Gantt chart
- **Fix:** Added `bar_fill()`, `fill_char()`, and
  `empty_char()` methods on `GanttRow`. CLI only
  concatenates characters.
- **Resolved:** 2026-04-03

### AQ-002 — GanttRow missing common trait derives

- **Date:** 2026-04-03
- **Category:** API Design
- **Commit context:** v0.13.0 Gantt chart
- **Fix:** Added `Clone`, `PartialEq`, `Eq` derives.
- **Resolved:** 2026-04-03

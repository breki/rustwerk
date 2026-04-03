# Artisan Findings — Resolved

Archive of fixed Artisan code quality findings, newest
first. See [artisan-log.md](artisan-log.md) for open
findings.

---

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

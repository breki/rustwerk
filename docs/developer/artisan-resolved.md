# Artisan Findings — Resolved

Archive of fixed Artisan code quality findings, newest
first. See [artisan-log.md](artisan-log.md) for open
findings.

---

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

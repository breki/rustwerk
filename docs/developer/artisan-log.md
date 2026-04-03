# Artisan Findings — Open

Code quality findings from the Artisan reviewer, newest
first. Fixed findings are moved to
[artisan-resolved.md](artisan-resolved.md).

**Next ID:** AQ-009

**Threshold:** when 10+ findings are open, a full-codebase
Artisan review is required before continuing feature work.

---

### AQ-007 — Task::assignee is stringly-typed, no referential integrity

- **Date:** 2026-04-03
- **Category:** Type Safety
- **Commit context:** v0.15.0 Developer domain type
- **Description:** `Task::assignee` is `Option<String>`
  while `DeveloperId` exists as a validated type. `assign()`
  accepts raw `&str` — tasks can reference non-existent
  developers, and case mismatches bypass the
  `remove_developer` guard.
- **Impact:** Phantom assignments, bypassed removal guards.
- **Suggested fix:** This is the DEV-ASSIGN WBS task —
  change `assignee` to `DeveloperId` and validate against
  `self.developers`.

### AQ-001 — Silent complexity fallback masks unscored tasks

- **Date:** 2026-04-03
- **Category:** Error Handling
- **Commit context:** v0.13.0 Gantt chart
- **Description:** `unwrap_or(1)` when complexity is
  `None` makes unscored tasks indistinguishable from
  complexity-1 tasks. Chart looks authoritative but may
  be meaningless if most tasks are unscored.
- **Impact:** Misleading planning chart.
- **Suggested fix:** Add `width_is_estimated: bool` to
  `GanttRow`, warn on stderr when defaults are used.

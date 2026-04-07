# Artisan Findings — Open

Code quality findings from the Artisan reviewer, newest
first. Fixed findings are moved to
[artisan-resolved.md](artisan-resolved.md).

**Next ID:** AQ-046

**Threshold:** when 10+ findings are open, a full-codebase
Artisan review is required before continuing feature work.

---

### AQ-031 — `batch.rs` exceeds 500-line threshold

- **Date:** 2026-04-04
- **Category:** Module Size
- **Commit context:** v0.33.0 batch dev commands
- **Description:** `batch.rs` is ~700 lines, with ~370 lines
  of tests. The `execute_one` dispatch function already has
  `#[allow(clippy::too_many_lines)]`.
- **Better approach:** Extract tests into a sibling file or
  split `execute_one` into sub-dispatchers by domain.

### AQ-027 — Cycle handling in build_subtree undocumented

- **Date:** 2026-04-03
- **Category:** Type Safety
- **Commit context:** v0.30.0 tree command
- **Description:** `build_subtree` uses `seen` set to
  produce `Reference` nodes for revisited tasks. This
  also acts as a cycle guard, but cycles are already
  rejected by `add_dependency`. Behavior is correct but
  the cycle-guard aspect is undocumented.

---

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

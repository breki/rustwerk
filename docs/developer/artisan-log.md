# Artisan Findings — Open

Code quality findings from the Artisan reviewer, newest
first. Fixed findings are moved to
[artisan-resolved.md](artisan-resolved.md).

**Next ID:** AQ-012

**Threshold:** when 10+ findings are open, a full-codebase
Artisan review is required before continuing feature work.

---

### AQ-011 — Module size: `project.rs` and `rustwerk.rs`

- **Date:** 2026-04-03
- **Category:** Module Size
- **Commit context:** v0.17.0 VIZ-UNICODE
- **Description:** `project.rs` is 1895 lines and
  `rustwerk.rs` is 1529 lines, both far exceeding the
  500-line threshold. `GanttRow` could be extracted to
  `gantt.rs`; batch/rendering logic could be split from
  the main binary.
- **Impact:** Large files are harder to navigate and
  review.
- **Suggested fix:** Extract `gantt.rs` module from
  `project.rs`; split rendering and batch submodules
  from `rustwerk.rs`.

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

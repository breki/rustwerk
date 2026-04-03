# Artisan Findings — Open

Code quality findings from the Artisan reviewer, newest
first. Fixed findings are moved to
[artisan-resolved.md](artisan-resolved.md).

**Next ID:** AQ-028

**Threshold:** when 10+ findings are open, a full-codebase
Artisan review is required before continuing feature work.

---

### AQ-027 — Cycle handling in build_subtree undocumented

- **Date:** 2026-04-03
- **Category:** Type Safety
- **Commit context:** v0.30.0 tree command
- **Description:** `build_subtree` uses `seen` set to
  produce `Reference` nodes for revisited tasks. This
  also acts as a cycle guard, but cycles are already
  rejected by `add_dependency`. Behavior is correct but
  the cycle-guard aspect is undocumented.

### AQ-026 — render_tree writes to stdout, not testable

- **Date:** 2026-04-03
- **Category:** API Design
- **Commit context:** v0.30.0 tree command
- **Description:** `render_tree`/`render_node` call
  `println!` directly. Tests only verify "no panic".
  Pre-existing pattern (same as gantt.rs).
- **Better approach:** Accept `&mut dyn Write` parameter.

### AQ-025 — build_tree duplicates reverse_dependents

- **Date:** 2026-04-03
- **Category:** Abstraction Boundaries
- **Commit context:** v0.30.0 tree command
- **Description:** `build_tree` constructs its own
  `children_of` map instead of reusing the existing
  `reverse_dependents()` method with post-sort.

### AQ-024 — scheduling.rs now 1,609 lines

- **Date:** 2026-04-03
- **Category:** Module Size
- **Commit context:** v0.30.0 tree command
- **Description:** Tree methods added ~150 lines to the
  already-oversized scheduling.rs. Subsumes AQ-020.
  Extract tree methods to `tree.rs` domain module.

### AQ-020 — scheduling.rs exceeds 500-line module-size rule

- **Date:** 2026-04-03
- **Category:** Module Size
- **Commit context:** v0.28.0 task list filters
- **Description:** `scheduling.rs` is ~1,300 lines and
  mixes five distinct responsibilities: topological sort,
  critical path, task-state queries, bottleneck detection,
  summary aggregation, and Gantt scheduling. Each has its
  own struct/algorithm family.
- **Impact:** Developers must scroll past unrelated code;
  no obvious place for new query methods. Pre-existing
  issue, not introduced by this diff.
- **Better approach:** Split into `topo.rs`,
  `critical_path.rs`, `queries.rs`, `bottleneck.rs`,
  `gantt_schedule.rs`.

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

# Artisan Findings — Open

Code quality findings from the Artisan reviewer, newest
first. Fixed findings are moved to
[artisan-resolved.md](artisan-resolved.md).

**Next ID:** AQ-056

**Threshold:** when 10+ findings are open, a full-codebase
Artisan review is required before continuing feature work.

---

### AQ-055 — `TaskDto.effort_estimate` remains stringly-typed

- **Date:** 2026-04-19
- **Category:** Type Safety (Low)
- **Commit context:** feat: add `rustwerk-plugin-api` crate
  (v0.40.0)
- **Description:** `TaskDto.status` was lifted to a
  `TaskStatusDto` enum in this commit, but
  `effort_estimate: Option<String>` still carries the
  host's serialized effort form (e.g. `"2d"`, `"4h"`).
  The host domain models effort as a structured
  `Effort { value: f64, unit: EffortUnit }`. Mirroring
  that in the DTO layer would remove the last
  stringly-typed field and let plugins parse effort
  without replicating the host's grammar.
- **Why deferred:** Mirroring `Effort` requires copying
  the host's `EffortUnit` enum and its grammar into the
  plugin-api crate, which couples the "plain strings for
  portability" DTO layer to host-internal value types.
  Kept deferred until a concrete plugin use case shows
  that parsing the string form is painful, at which
  point the right shape (enum? newtype? parser helper?)
  will be clearer.
- **Better approach when resolved:** Add an
  `EffortDto { value: f64, unit: EffortUnitDto }` pair
  to `rustwerk-plugin-api` with `#[serde(rename_all =
  "snake_case")]` on the unit enum; convert at the host
  boundary; provide a `Display` impl so the current
  string form is still obtainable for logging.

---

### AQ-048 — `domain/project/mod.rs` exceeds 500-line threshold

- **Date:** 2026-04-19
- **Category:** Module Size
- **Commit context:** feat: `task rename` command (v0.39.0)
- **Description:** `crates/rustwerk/src/domain/project/mod.rs`
  is over 1000 lines, holding add/remove/rename/update task
  logic, dependency graph operations (add_dependency,
  remove_dependency, has_cycle), effort logging, developer
  registry operations, and a large inline test block. The
  new `rename_task` added ~60 lines and pushed it further.
- **Better approach:** The `project` directory is already a
  module; split the impl block across sibling files
  following the existing pattern (bottleneck.rs,
  critical_path.rs, queries.rs, etc.):
  `project/tasks.rs` — add/remove/rename/update task
  operations; `project/dependencies.rs` — add_dependency,
  remove_dependency, has_cycle; `project/effort.rs` —
  log_effort, set_effort_estimate; `project/developers.rs` —
  add/remove developer. Each sub-module keeps its own
  `#[cfg(test)]` block. `project/mod.rs` stays focused on
  the struct definition, constructors, and serialization.
  Deferred from the v0.39.0 feat commit to keep the feature
  and refactor changesets separate.

---

### AQ-047 — `run_check` truncation logic is untested

- **Date:** 2026-04-19
- **Category:** Testability
- **Commit context:** chore: adopt rustbase template (add
  `xtask check`)
- **Description:** The `count > CHECK_MAX_ERROR_LINES` /
  `... and N more` truncation branch in `run_check`
  (`xtask/src/main.rs`) is pure presentation logic but is
  not covered by unit tests — only the pure
  `extract_check_errors` helper is tested. The arithmetic
  `count - CHECK_MAX_ERROR_LINES` is the kind of off-by-one
  that slips review.
- **Better approach:** Extract a
  `fn format_check_failure(errors: &[&str]) -> String`
  helper and have `run_check` print its result. Add a
  fifth unit test that locks truncation behavior for
  `errors.len() > CHECK_MAX_ERROR_LINES`.

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

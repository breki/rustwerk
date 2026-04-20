# Artisan Findings — Open

Code quality findings from the Artisan reviewer, newest
first. Fixed findings are moved to
[artisan-resolved.md](artisan-resolved.md).

**Next ID:** AQ-079

**Threshold:** when 10+ findings are open, a full-codebase
Artisan review is required before continuing feature work.

---

### AQ-074 — Plugin-response size cap does not bound `CStr::from_ptr` walk

- **Date:** 2026-04-19
- **Category:** Correctness / defensive coding (deferred — inherent)
- **Commit context:** feat: add dynamic plugin host
  (v0.43.0)
- **Description:** `parse_plugin_response` in
  `plugin_host.rs` calls `CStr::from_ptr(ptr)` to
  measure the plugin-returned string before applying
  `MAX_PLUGIN_RESPONSE_BYTES`. `CStr::from_ptr` does
  an unbounded `strlen` walk first; the cap limits
  parse memory but not the walk itself.
- **Impact:** A malicious plugin returning a
  multi-GB NUL-terminated buffer consumes host CPU
  and memory during the strlen walk, defeating the
  cap's DoS-prevention intent.
- **Better approach:** Use `libc::strnlen(ptr, MAX)`
  or a manual bounded scan. Both still require
  trusting the plugin's allocation to contain at
  least `MAX` bytes if scanning that far — reading
  past an allocation is UB regardless. True bounded
  reads require a protocol change (plugin returns
  `(ptr, len)` pair).
- **Deferred rationale:** The FFI contract already
  trusts the plugin to NUL-terminate its buffer;
  adding `strnlen` would bound CPU but not remove
  the underlying trust requirement. Revisit when
  the plugin API is revved.

---

### AQ-073 — `commands/task.rs` still over the 500-line threshold

- **Date:** 2026-04-19
- **Category:** Module Size (deferred)
- **Commit context:** feat: add `--json` global output
  flag (v0.42.0)
- **Description:** `crates/rustwerk/src/bin/rustwerk/commands/task.rs`
  is 573 lines after the CLI-JSON refactor — over the
  500-line threshold. Contains 11 `cmd_*` entry points
  plus filter helpers plus list rendering plus ~10 DTO
  structs.
- **Better approach:** Split into a `commands/task/`
  subdirectory, e.g. `list.rs` (filters + TaskListItem
  + TaskListOutput + filter_task_ids); `crud.rs` (add,
  update, remove, rename); `workflow.rs` (assign,
  unassign, status, depend, undepend); `describe.rs`.
- **Deferred rationale:** The CLI-JSON refactor was
  already large; splitting would obscure the diff.
  The DTOs are cohesive and the file is coherent to
  read. Worth revisiting if/when additional task
  commands are added.

---

### AQ-062 — `rustwerk-plugin-api` non-optional while `libloading` is gated

- **Date:** 2026-04-19
- **Category:** API Design (Low, deferred)
- **Commit context:** feat: wire plugin crates into
  workspace (v0.41.0)
- **Description:** `crates/rustwerk/Cargo.toml:22,28-29`
  — the `plugins` feature gates `libloading` (the
  dynamic-loading primitive) but `rustwerk-plugin-api`
  (the FFI types) is pulled in unconditionally.
  Asymmetric: disabling `plugins` still compiles the
  API crate.
- **Better approach:** Either mark `rustwerk-plugin-api`
  as `optional = true` and add it to `plugins =
  ["dep:libloading", "dep:rustwerk-plugin-api"]`, or
  document why the API crate must always be present
  (e.g. types used in non-plugin serialization paths).
- **Deferred rationale:** Same as RT-089 — scaffolding
  by design. Revisit when PLG-HOST lands and the
  actual consumption pattern is visible.

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

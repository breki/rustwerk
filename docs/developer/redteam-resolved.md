# Red Team Findings — Resolved

Archive of fixed red team findings, newest first.
See [redteam-log.md](redteam-log.md) for open findings.

---

### RT-044 — --status filter help text missing on-hold

- **Date:** 2026-04-03
- **Category:** Cosmetic
- **Commit context:** v0.29.0 ON_HOLD status
- **Resolution:** Updated help text in List command's
  `--status` arg to include `on-hold`.

### RT-043 — Tasks depending on ON_HOLD show as dep-blocked

- **Date:** 2026-04-03
- **Category:** Correctness (Low)
- **Commit context:** v0.29.0 ON_HOLD status
- **Description:** `dep_blocked_tasks` flags TODO tasks
  with non-Done deps, so ON_HOLD deps trigger blocking.
- **Resolution:** Kept as correct behavior — an on-hold
  dep IS incomplete. No code change needed.

### RT-042 — Missing OnHold → InProgress transition

- **Date:** 2026-04-03
- **Category:** Correctness
- **Commit context:** v0.29.0 ON_HOLD status
- **Description:** No direct `OnHold → InProgress` path
  forced unnecessary roundtrip through TODO.
- **Resolution:** Added `(OnHold, InProgress)` transition.

### RT-041 — ON_HOLD tasks pollute remaining critical path

- **Date:** 2026-04-03
- **Category:** Correctness
- **Commit context:** v0.29.0 ON_HOLD status
- **Description:** `remaining_critical_path` and
  `gantt_schedule_remaining` filtered `!= Done`, so
  ON_HOLD tasks appeared as active work on critical path
  and in `gantt --remaining`.
- **Resolution:** Added `&& status != OnHold` filter to
  both methods. ON_HOLD deps treated as satisfied in
  remaining schedule.

### RT-039 — --assignee case-sensitive with no normalization

- **Date:** 2026-04-03
- **Category:** Usability
- **Commit context:** v0.28.0 task list filters
- **Description:** `--assignee` did an exact string match
  without lowercasing. `DeveloperId::new()` lowercases IDs,
  so `--assignee Alice` would miss tasks assigned to
  `alice`.
- **Resolution:** Added `.to_lowercase()` on the assignee
  filter input before comparison.

### RT-037 — --status not declared conflicting with --available/--active

- **Date:** 2026-04-03
- **Category:** Correctness
- **Commit context:** v0.28.0 task list filters
- **Description:** `--status` had no `conflicts_with`
  against `--available` or `--active`, allowing nonsensical
  combinations that silently produced empty output.
- **Resolution:** Added `conflicts_with_all` to the
  `--status` clap arg definition.

### RT-036 — Hardcoded column width breaks for long IDs

- **Date:** 2026-04-03
- **Category:** Correctness
- **Commit context:** v0.27.0 report bottlenecks command
- **Description:** `cmd_report_bottlenecks` used `{:<12}` for
  the ID column. Task IDs longer than 12 chars would misalign
  all subsequent columns.
- **Resolution:** Compute `iw` dynamically from the actual
  bottleneck list, consistent with `cmd_task_list`.

### RT-035 — Panicking index on `project.tasks[&bn.id]`

- **Date:** 2026-04-03
- **Category:** Correctness
- **Commit context:** v0.27.0 report bottlenecks command
- **Description:** The CLI command used the panicking `[]`
  operator on `project.tasks` to look up bottleneck details.
  A domain bug could cause a panic instead of a clean error.
- **Resolution:** Enriched `Bottleneck` struct with `status`,
  `assignee`, and `ready` fields populated in the domain layer.
  The CLI no longer accesses `project.tasks` directly.

### RT-034 — Done tasks counted as downstream dependents

- **Date:** 2026-04-03
- **Category:** Correctness
- **Commit context:** v0.26.0 bottleneck detection
- **Description:** `bottlenecks()` built the reverse adjacency
  map from all tasks without filtering by status. Done tasks
  appeared as dependents, inflating bottleneck scores. A task
  blocking only finished tasks would show a high count despite
  blocking no remaining work.
- **Resolution:** Filter done tasks when building the reverse
  adjacency map. Extracted `reverse_dependents()` helper with
  a status predicate to make the intent explicit and prevent
  future divergence.

### RT-033 — BOLD+DIM conflict on critical Todo bars

- **Date:** 2026-04-03
- **Category:** Correctness
- **Commit context:** v0.18.0 critical path highlight
- **Description:** `\x1b[1m\x1b[2m` (BOLD then DIM)
  results in DIM only on most terminals, making critical
  Todo bars indistinguishable from non-critical ones.
- **Resolution:** Switched to rendering the entire
  critical path line in RED, bypassing per-status colors
  entirely. No ANSI attribute conflicts possible.

### RT-032 — `fill_char` for Todo is unreachable dead code

- **Date:** 2026-04-03
- **Category:** Correctness
- **Commit context:** v0.17.0 VIZ-UNICODE
- **Description:** For `Status::Todo`, `bar_fill()` always
  returns `(0, width)` — zero filled chars. So the
  `Status::Todo` arm in `fill_char()` is never used in
  practice.
- **Resolution:** Added defensive comment explaining the arm
  is only reached if `bar_fill` logic changes.

### RT-031 — Blocked and Done bars indistinguishable without color

- **Date:** 2026-04-03
- **Category:** Correctness
- **Commit context:** v0.17.0 VIZ-UNICODE
- **Description:** `fill_char()` returned `█` (U+2588)
  for both `Done` and `Blocked`. In non-color mode (piped
  output, `NO_COLOR`), blocked and done tasks were visually
  identical.
- **Resolution:** Changed `Blocked` fill to `▒` (U+2592,
  medium shade), restoring visual distinction without color.

### RT-030 — `scale(start=0)` returns 1, misaligning root tasks

- **Date:** 2026-04-03
- **Category:** Correctness
- **Commit context:** v0.16.0 VIZ-SCALE
- **Description:** `scale()` clamped all results to
  minimum 1, but `start = 0` should map to 0 (no
  padding). Every root task got a spurious 1-space
  indent, misaligning bars from tick marks.
- **Fix:** Split into `scale_min1()` (for bar widths)
  and `scale_pos()` (for positions, no clamp).
- **Resolved:** 2026-04-03

### RT-029 — Stale doc comment above `add_developer`

- **Date:** 2026-04-03
- **Category:** Correctness
- **Commit context:** v0.15.0 Developer domain type
- **Description:** Doc comment above `add_developer` read
  "Log effort on a task" — a stale paste from the adjacent
  method. Rustdoc showed the wrong description.
- **Fix:** Corrected the doc comment.
- **Resolved:** 2026-04-03

### RT-028 — `remove_developer` doesn't update `modified_at`

- **Date:** 2026-04-03
- **Category:** Correctness
- **Commit context:** v0.15.0 Developer domain type
- **Description:** Every other mutation method advances
  `modified_at`, but `remove_developer` skipped it.
  Timestamp-based change detection would miss this.
- **Fix:** Added `self.metadata.modified_at = Utc::now()`
  before returning the removed developer.
- **Resolved:** 2026-04-03

### RT-027 — ANSI state leaks across Gantt row fields

- **Date:** 2026-04-03
- **Category:** Correctness
- **Commit context:** v0.14.0 Gantt colors
- **Description:** The format string for Gantt rows applied
  `crit_style` (CYAN) before the marker and `id_style`
  after it, with only one reset at the end. For critical
  Done tasks, `crit_style = CYAN` carried through the ID
  when `id_style = ""`, making the ID appear cyan.
- **Fix:** Added `{rst}` reset between marker and ID style
  scopes so each color context is isolated.
- **Resolved:** 2026-04-03

### RT-026 — `--available --active` silently ignores `--active`

- **Date:** 2026-04-03
- **Category:** Correctness
- **Commit context:** v0.13.1 available/active fix
- **Description:** Both `--available` and `--active` could
  be passed simultaneously. The `if/else if` chain silently
  honoured `--available` and dropped `--active` with no
  error. An AI agent calling programmatically had no way
  to detect the misuse.
- **Fix:** Added `#[arg(conflicts_with = "available")]` on
  `active` so clap rejects the combination at parse time.
- **Resolved:** 2026-04-03

### RT-025 — Unbounded complexity causes OOM in Gantt

- **Date:** 2026-04-03
- **Category:** Security/DoS
- **Commit context:** v0.13.0 Gantt chart
- **Description:** No upper bound on complexity. Large
  values cause `" ".repeat()` to allocate gigabytes in
  the Gantt renderer.
- **Fix:** Added `Task::set_complexity` validating
  1..=1000. Applied in CLI, batch, and WBS import.
- **Resolved:** 2026-04-03

### RT-023 — Zero complexity accepted, corrupts schedule

- **Date:** 2026-04-03
- **Category:** Correctness
- **Commit context:** v0.13.0 Gantt chart
- **Description:** `complexity: 0` passed through
  `unwrap_or(1)` only for `None`, not `Some(0)`.
  Zero-width bars broke chart layout and critical path.
- **Fix:** `set_complexity` rejects 0. Validated in all
  input paths.
- **Resolved:** 2026-04-03

### RT-022 — Unbounded WBS import array (DoS)

- **Date:** 2026-04-03
- **Category:** Security
- **Commit context:** v0.12.0 WBS schema
- **Description:** `parse_wbs` had no size limit on the
  resulting array. Millions of entries would allocate
  until OOM.
- **Trigger:** A 100MB JSON file with 1M task entries.
- **Fix:** Added `MAX_WBS_ENTRIES = 10_000` limit in
  `import_into_project`.
- **Resolved:** 2026-04-03

### RT-021 — Unicode homoglyph spoofing in TaskId

- **Date:** 2026-04-03
- **Category:** Security
- **Commit context:** v0.12.0 WBS schema
- **Description:** `TaskId::new` used `c.is_alphanumeric()`
  which accepts Unicode alphanumerics (Cyrillic, Greek,
  etc.). Visually identical IDs using different codepoints
  could coexist as distinct keys.
- **Trigger:** Import two tasks with IDs "AUTH" (Latin)
  and "АUTH" (Cyrillic А) — both created.
- **Fix:** Changed to `c.is_ascii_alphanumeric()`.
- **Resolved:** 2026-04-03

### RT-020 — False idempotency on dependency re-add

- **Date:** 2026-04-03
- **Category:** Code Quality
- **Commit context:** v0.12.0 WBS schema
- **Description:** When a task ID already existed during
  import, pass one skipped it but pass two still processed
  its dependencies. Re-importing a WBS with edited deps
  could silently add new edges to existing tasks.
- **Trigger:** Import WBS, manually remove dep A→B,
  re-import same WBS — A→B silently re-added.
- **Fix:** Changed to fail with an error if an existing
  task's dependencies differ from those in the import.
- **Resolved:** 2026-04-03

### RT-019 — Partial state mutation on WBS import failure

- **Date:** 2026-04-03
- **Category:** Code Quality
- **Commit context:** v0.12.0 WBS schema
- **Description:** `import_into_project` created tasks in
  pass one, then added dependencies in pass two. If pass
  two failed (cycle, bad ID), the error was returned but
  all tasks from pass one remained in the project —
  leaving it in an inconsistent state with orphaned tasks.
- **Trigger:** Import a WBS with a circular dependency.
  Both tasks get created, then the cycle is detected and
  the error returned — but the tasks remain.
- **Fix:** Clone the project before mutation, restore on
  error (snapshot/rollback pattern).
- **Resolved:** 2026-04-03

### RT-018 — Unbounded batch command count (DoS)

- **Date:** 2026-04-03
- **Category:** Security
- **Commit context:** Coverage infrastructure
- **Description:** After the 10MB stdin cap, the
  deserialized command array had no size limit. A compact
  JSON payload with many small commands could expand to
  large memory usage during execution (each command
  mutates the in-memory project).
- **Trigger:** A 9MB JSON file with 500,000 minimal
  `task.add` commands.
- **Fix:** Added `MAX_BATCH_COMMANDS = 1000` limit after
  deserialization.
- **Resolved:** 2026-04-03

### RT-017 — Test binary lookup fragile for nextest

- **Date:** 2026-04-03
- **Category:** Code Quality
- **Commit context:** Coverage infrastructure
- **Description:** Integration tests found the rustwerk
  binary by navigating from `current_exe()` with two
  `pop()` calls, assuming a specific directory layout.
  This breaks with `cargo nextest` or non-standard
  `CARGO_TARGET_DIR`.
- **Trigger:** `cargo nextest run` — all integration
  tests fail with "failed to run rustwerk".
- **Fix:** Added `CARGO_BIN_EXE_rustwerk` env var lookup
  (set by cargo for workspace binaries) with the path
  computation as fallback.
- **Resolved:** 2026-04-03

### RT-016 — Batch exit(1) bypasses stdout flush

- **Date:** 2026-04-03
- **Category:** Code Quality
- **Commit context:** Coverage infrastructure
- **Description:** `cmd_batch` called
  `std::process::exit(1)` on batch failure, bypassing
  Rust's drop/flush guarantees. On Windows (where stdout
  is not line-buffered when piped), the error JSON output
  could be truncated or lost entirely.
- **Trigger:** Pipe batch output to another process on
  Windows — error JSON may be silently swallowed.
- **Fix:** Replaced `exit(1)` with `bail!()` to return an
  error through the normal `main() -> Result<()>` path,
  which ensures proper cleanup and flush.
- **Resolved:** 2026-04-03

### RT-015 — Coverage JSON silently defaults missing fields

- **Date:** 2026-04-03
- **Category:** Code Quality
- **Commit context:** Coverage infrastructure
- **Description:** `run_coverage` in xtask used
  `unwrap_or(0)` for the `covered` and `count` fields
  from `cargo llvm-cov` JSON output. If the JSON schema
  changed, these would silently default to 0 while the
  `percent` field still passed the threshold check,
  producing misleading output like `0/0 (91.0%)`.
- **Trigger:** A future version of cargo-llvm-cov renames
  `count` to `total`.
- **Fix:** Replaced `unwrap_or(0)` with `.ok_or()` that
  returns an explicit error on missing fields.
- **Resolved:** 2026-04-03

### RT-012 — Raw command name in batch error output

- **Date:** 2026-04-03
- **Category:** Security
- **Commit context:** v0.11.0 batch command
- **Description:** The `command` field from attacker-
  supplied JSON was interpolated into the error message
  without sanitization. While `serde_json` escapes the
  JSON encoding, the raw string could contain ANSI escape
  sequences or control characters that affect terminal
  rendering if the output is displayed raw.
- **Trigger:** `{"command":"task.add\u001b[31mRED",
  "args":{}}` — the error message contains an ANSI
  escape.
- **Fix:** Truncated command name to 64 chars and stripped
  control characters before embedding in error output.
- **Resolved:** 2026-04-03

### RT-011 — Unbounded stdin read in batch (DoS)

- **Date:** 2026-04-03
- **Category:** Security
- **Commit context:** v0.11.0 batch command
- **Description:** `read_to_string` on stdin had no size
  limit, allowing any process that feeds stdin to force
  arbitrarily large memory allocation until OOM.
- **Trigger:** `yes '[{}]' | head -c 10G | rustwerk batch`
- **Fix:** Added `stdin().take(10MB)` cap before reading.
- **Resolved:** 2026-04-03

### RT-010 — Empty batch skips project load

- **Date:** 2026-04-03
- **Category:** Code Quality
- **Commit context:** v0.11.0 batch command
- **Description:** An empty batch `[]` returned success
  without loading the project file. If the project was
  corrupt or missing, the caller got a false `[]` / exit 0
  instead of an error.
- **Trigger:** `echo '[]' | rustwerk batch` from a
  directory with no `.rustwerk/` project.
- **Fix:** Moved `load_project()` before the empty check.
- **Resolved:** 2026-04-03

### RT-009 — Batch task.update succeeds with no fields

- **Date:** 2026-04-03
- **Category:** Code Quality
- **Commit context:** v0.11.0 batch command
- **Description:** `task.update` in batch mode accepted a
  command with only an `id` and no `title` or `desc`,
  returning a success message even though nothing changed.
  Misleading for AI agents that expect confirmation to
  mean a mutation occurred.
- **Trigger:** `{"command":"task.update","args":{"id":"X"}}`
  returns `{"ok":true,"message":"Updated X"}`.
- **Fix:** Added validation requiring at least one of
  `title` or `desc` to be present.
- **Resolved:** 2026-04-03

### RT-008 — Batch complexity silently truncates large values

- **Date:** 2026-04-03
- **Category:** Code Quality
- **Commit context:** v0.11.0 batch command
- **Description:** In the batch `task.add` handler,
  `complexity` was extracted as `u64` from JSON and cast
  to `u32` with `as`, silently wrapping values above
  `u32::MAX`. A value of 5 billion would be stored as
  ~705 million.
- **Trigger:** `{"command":"task.add","args":{"title":"X",
  "complexity":5000000000}}`
- **Fix:** Replaced `as u32` with `u32::try_from` that
  returns an error on overflow.
- **Resolved:** 2026-04-03

### RT-007 — Batch applied count always reports 0

- **Date:** 2026-04-03
- **Category:** Code Quality
- **Commit context:** v0.11.0 batch command
- **Description:** The batch error JSON hardcoded
  `"applied": 0` regardless of how many commands had
  executed before the failure. While the all-or-nothing
  design means nothing is persisted, the field misleads
  callers (especially AI agents) about how far execution
  progressed.
- **Trigger:** Batch with 5 commands where the 4th fails.
  Error reports `applied: 0` instead of `applied: 3`.
- **Fix:** Replaced hardcoded `0` with loop index `i`.
- **Resolved:** 2026-04-03

### RT-006 — Show command hides effort when only actuals exist

- **Date:** 2026-04-03
- **Category:** Code Quality
- **Commit context:** v0.10.0 project summary
- **Description:** `cmd_show` only displayed the effort
  line when `total_estimated_hours > 0.0`. If tasks had
  logged actual effort but no estimates were set, the
  entire effort section was hidden — the user had no
  indication that any effort had been tracked.
- **Trigger:** Log effort on a task without setting an
  estimate, then run `rustwerk show`.
- **Fix:** Changed condition to show effort when either
  estimated or actual hours are greater than zero.
- **Resolved:** 2026-04-03

### RT-005 — Auto-IDs sort incorrectly beyond single digits

- **Date:** 2026-04-02
- **Category:** Code Quality
- **Commit context:** v0.3.0 task management
- **Description:** `BTreeMap<TaskId, _>` sorts
  lexicographically, so `T10` sorted before `T2`. Users
  with 10+ auto-generated tasks would see a confusing
  display order in `task list`.
- **Trigger:** Create 10+ tasks without explicit IDs.
  `task list` shows T1, T10, T11, ..., T2, T3, ...
- **Fix:** Zero-padded auto-IDs to 4 digits (T0001,
  T0002, etc.) for correct lexicographic ordering.
- **Resolved:** 2026-04-02

### RT-004 — Effort::parse accepts NaN and Infinity

- **Date:** 2026-04-02
- **Category:** Code Quality
- **Commit context:** v0.3.0 task management
- **Description:** Rust's `f64::parse` accepts "inf",
  "-inf", and "NaN" as valid inputs. The `> 0.0` check
  doesn't catch NaN (`NaN <= 0.0` is false) or positive
  infinity. These values would be stored in JSON and
  produce nonsensical output.
- **Trigger:** `Effort::parse("infH")` succeeds and stores
  infinity as the effort value.
- **Fix:** Added `!value.is_finite()` guard before the
  positivity check.
- **Resolved:** 2026-04-02

### RT-003 — add_task_auto silently overwrites on ID collision

- **Date:** 2026-04-02
- **Category:** Code Quality
- **Commit context:** v0.3.0 task management
- **Description:** `add_task_auto` called `BTreeMap::insert`
  without checking for a pre-existing key. If `next_auto_id`
  was manually set in the JSON to collide with an existing
  task, or if a user-supplied ID like "T3" existed, the
  auto-ID generator would silently overwrite that task.
- **Trigger:** Hand-edit `project.json` to set
  `next_auto_id: 1` while task `T0001` already exists,
  then run `task add "New task"`.
- **Fix:** Added a loop in `add_task_auto` that skips IDs
  already present in the task map.
- **Resolved:** 2026-04-02

### RT-002 — Effort display truncates large values via u64 cast

- **Date:** 2026-04-02
- **Category:** Code Quality
- **Commit context:** Phase 1 implementation
- **Description:** `Effort::Display` used `self.value as
  u64` for whole numbers, which silently truncates values
  above `u64::MAX` or large `f64` values like `1e20`.
  The serialized string would round-trip to a completely
  different value, corrupting stored data.
- **Trigger:** `Effort { value: 1e20, unit: H }` displays
  as a truncated integer.
- **Fix:** Replaced `as u64` with `{:.0}` format
  specifier which handles all f64 values correctly.
- **Resolved:** 2026-04-02

### RT-001 — Effort::parse panics on multibyte UTF-8 suffix

- **Date:** 2026-04-02
- **Category:** Code Quality
- **Commit context:** Phase 1 implementation
- **Description:** `Effort::parse` used byte-offset
  `split_at` which panics if the input string ends with
  a multibyte UTF-8 character (e.g. a Unicode lookalike
  for 'H'). The split would land in the middle of a
  character boundary, causing a runtime panic instead of
  a clean error.
- **Trigger:** `Effort::parse("2.5\u{FF28}")` where
  `\u{FF28}` is fullwidth 'H' (3 bytes).
- **Fix:** Replaced `split_at` with `chars().last()` and
  `len_utf8()` for safe character extraction.
- **Resolved:** 2026-04-02

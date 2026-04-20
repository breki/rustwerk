# Artisan Findings — Open

Code quality findings from the Artisan reviewer, newest
first. Fixed findings are moved to
[artisan-resolved.md](artisan-resolved.md).

**Next ID:** AQ-128

---

### AQ-107 — `MockHttp::new(Vec<Result<_, _>>)` is noisy at call sites

- **Date:** 2026-04-20
- **Category:** Test ergonomics
- **Commit context:** PLG-JIRA-UPDATE review sweep (v0.50.0).
- **Description:** `test_support::MockHttp::new`
  takes `Vec<Result<HttpResponse, HttpError>>`, so
  every test wraps with `ok(...)` / `transport_err(...)`
  helpers precisely because the raw `Result` is
  awkward. Across ~30 call sites the `vec![ok(...),
  ok(...), ...]` pattern is pure noise.
- **Impact:** Minor; only test readability.
- **Suggested fix:** Fluent builder
  `MockHttp::new().respond(status, body).transport_err("dns")`.
  Skip if the explicit queue reads better to you.

---

### AQ-106 — `ParseIssueError::InvalidSelfUrl` leaks an untruncated URL into error messages

- **Date:** 2026-04-20
- **Category:** Error messages + invariant violation
- **Commit context:** PLG-JIRA-UPDATE review sweep (v0.50.0).
- **Description:** `MAX_RESPONSE_BODY_BYTES`
  (`jira_client.rs:81`) caps response bodies at
  4 KiB so error messages stay bounded. But the
  parsed `self` URL bypasses that cap via the
  `InvalidSelfUrl(String)` variant — a 1 MB
  `javascript:…` URL lands whole in the task error
  message. *Partial* fix already in place: the
  `Display` for `InvalidSelfUrl` uses
  `{0:.256}` formatting, so printed messages are
  bounded — but the underlying `String` inside the
  variant is still unbounded. Same story for
  `InvalidIssueKey(String)` which uses `{0:.64}`.
- **Impact:** Memory pressure inside the plugin
  process on a hostile response; log/telemetry
  readers see bounded Display output, so the
  user-facing impact is already minimized.
- **Suggested fix:** Change the variants to store
  only the truncated prefix:
  `InvalidSelfUrl(String)` → `{ prefix: String }`
  built from `created.self_url.chars().take(256).collect()`.
  Or store `{ scheme: String }` which is the
  actually-interesting part.

---

### AQ-104 — `TaskPushResult::with_external_key` ownership inconsistency across call sites

- **Date:** 2026-04-20
- **Category:** API design consistency
- **Commit context:** PLG-JIRA-UPDATE review sweep (v0.50.0).
- **Description:** The create path passes
  `created.key.as_str()` (a borrow), the update
  path passes `key.as_str()` (also a borrow — now
  consistent after AQ-105 landed). But
  `TaskPushResult::with_external_key` lives in
  `rustwerk-plugin-api` and its signature should
  be audited for `impl Into<String>` so future
  callers with either `&str` or `String` at hand
  Just Work.
- **Impact:** Non-blocking. Today both call sites
  pass `&str`, so the signature is fine as long as
  it's `impl Into<String>`. Worth confirming.
- **Suggested fix:** Verify and (if needed) widen
  the signature to `impl Into<String>`. Rust API
  Guidelines C-GENERIC.

---

### AQ-103 — `existing_issue_key_validated` forces no allocation, but key cloning into `ExistingKey::Valid` still happens

- **Date:** 2026-04-20
- **Category:** API design / unnecessary allocation
- **Commit context:** PLG-JIRA-UPDATE review sweep (v0.50.0).
- **Description:** AQ-103 (original: return `&str`
  not `String`) was superseded by AQ-105's
  `IssueKey` newtype introduction, which
  necessarily owns a `String`. The
  `existing_issue_key_validated` function
  constructs an `IssueKey` per validated call —
  one allocation per task. Acceptable for the
  number of tasks we expect, but worth noting if
  we later run into an N-heavy batch.
- **Impact:** Trivial today; flag if batch sizes
  grow into the thousands.
- **Suggested fix:** Consider `IssueKey<'a>(&'a str)`
  borrowed variant for the read-only path, but
  almost certainly not worth the API-surface
  doubling.

---

### AQ-102 — `create_issue` / `get_issue` / `update_issue` share identical fallback shape but do not dedup

- **Date:** 2026-04-20
- **Category:** API design / DRY
- **Commit context:** PLG-JIRA-UPDATE review sweep (v0.50.0).
- **Description:** Three functions in
  `jira_client.rs` (`create_issue`, `get_issue`,
  `update_issue`) are structurally identical:
  direct call → if not 401/404 return wrapped; else
  `resolve_cloud_id` + retry against gateway URL.
  Only the HTTP verb closure and URL-builder pair
  differ. The duplication scanner reports 3.8%
  exact duplication (up from 2.8% pre-PLG-JIRA-UPDATE).
  **Deferred** because `get_issue` now returns
  `ProbeOutcome` (different shape from the other
  two's `JiraOpOutcome`), so the cleanest
  factoring is no longer a single generic helper —
  two helpers may be right (`with_gateway_fallback_op`
  for create/update, inline for probe).
- **Impact:** Every future change (429 retry,
  logging, new fallback class) has to be applied
  three times. High odds of drift over time.
- **Suggested fix:**
  ```rust
  fn with_gateway_fallback<C, F>(
      http: &C, config: &JiraConfig,
      direct_url: &str,
      gateway_url_for_cloud_id: impl FnOnce(&str) -> String,
      call: F,
  ) -> Result<JiraOpOutcome, HttpError>
  where F: Fn(&C, &str, &str) -> Result<HttpResponse, HttpError>
  ```
  `get_issue` stays distinct because its probe
  semantics differ.

---

### AQ-101 — `lib.rs` (1138 lines) and `jira_client.rs` (933 lines) cross the 500-line split threshold

- **Date:** 2026-04-20
- **Category:** Module size
- **Commit context:** PLG-JIRA-UPDATE review sweep (v0.50.0).
- **Description:**
  `crates/rustwerk-jira-plugin/src/lib.rs` now
  1138 lines, `jira_client.rs` 933 lines. Both
  contain multiple struct/enum-impl clusters.
  `lib.rs` mixes FFI exports, `Clock`/`SystemClock`,
  push orchestration (`push_one`,
  `push_one_create`, `push_one_update`), result
  translation, and state-building.
  `jira_client.rs` contains two error enums + the
  `IssueKey` newtype + 4 structs + `HttpClient`
  trait + 6 URL builders + 3 verb fns +
  `resolve_cloud_id` + `ureq` translator layer +
  `parse_created_issue` validator.
- **Impact:** Cognitive load growing; adding a 4th
  Jira verb (delete, list, transition for
  PLG-JIRA-FIELDS) will balloon further.
- **Suggested fix:** Dedicated refactor task,
  probably one per file:
  - Split `lib.rs` into `ffi.rs` (FFI entry
    points + `write_json`/`error_payload`),
    `push.rs` (`push_all`, `push_one`,
    `push_one_create`, `push_one_update`,
    `existing_issue_key_validated`), `state.rs`
    (`build_created_state`, `build_refreshed_state`,
    `format_last_pushed_at`, `Clock`,
    `SystemClock`), `result.rs` (the two
    `task_result_from_*_outcome` translators).
  - Split `jira_client.rs` into
    `jira_client/{mod, errors, urls, http, verbs, parse}.rs`.

---

### AQ-094 — `API_VERSION` version-history doc block will age poorly

- **Date:** 2026-04-20
- **Category:** Doc placement
- **Commit context:** feat: per-task plugin-state
  round-trip in the plugin API (v0.48.0)
- **Description:** `rustwerk-plugin-api/src/lib.rs`
  embeds a "Version history" block inside the doc
  comment for `API_VERSION`. At v2 the block is
  small and genuinely useful; by v5 it will be
  stale or, worse, silently drifted from reality.
  Rust API Guidelines (C-LINK / C-METADATA) point
  toward release notes or a module-level
  "Version history" section for historical context.
- **Why not fixed in-commit:** No CHANGELOG.md
  exists yet; the v2 note is load-bearing right
  now. Migrate when a CHANGELOG lands.
- **Suggested fix:** Add a `CHANGELOG.md` to the
  `rustwerk-plugin-api` crate (or at workspace
  root with sections per crate); migrate the
  version history there; keep the `API_VERSION`
  doc to one sentence + link.

### AQ-093 — `serde_json::Value` leaked directly on the plugin-API public surface

- **Date:** 2026-04-20
- **Category:** API hygiene
- **Commit context:** feat: per-task plugin-state
  round-trip in the plugin API (v0.48.0)
- **Description:** `TaskDto.plugin_state` and
  `TaskPushResult.plugin_state_update` expose
  `serde_json::Value` directly on a stable wire
  contract. Compile cost is zero (serde_json is
  already a dep), but it advertises "any JSON goes"
  without a schema hook and bakes the JSON-shaped
  type into the contract. A `#[serde(transparent)]
  struct PluginState(Value)` newtype would give a
  doc anchor for the opaque-blob semantics and
  leave room to swap representation
  (e.g. `Box<RawValue>`) without a v3 bump.
- **Why not fixed in-commit:** Optional polish per
  the reviewer's own framing. The current shape is
  defensible while the API surface stays small.
- **Suggested fix:** Introduce
  `pub struct PluginState(pub serde_json::Value);`
  with `#[serde(transparent)]` and migrate both
  fields. Bundle with any future type-level API
  refinement.

### AQ-088 — Visibility bumps on `plugin_host` widen the crate-internal surface

- **Date:** 2026-04-20
- **Category:** Abstraction boundaries
- **Commit context:** feat: add `rustwerk plugin
  install` subcommand (v0.47.0)
- **Description:** `DYLIB_EXT`, `home_dir`, and
  `load_plugin` in `plugin_host.rs` were promoted to
  `pub(crate)` to serve `commands::plugin::install`.
  `load_plugin` genuinely needs this — the install
  verifier needs the single-file load entry point.
  The other two are primitives that could be hidden
  behind higher-level operations (e.g. a
  `plugin_host::validate_dylib_extension(&Path)` that
  owns both `DYLIB_EXT` and the extension check, and
  a `plugin_host::user_plugin_dir() -> Option<PathBuf>`
  that wraps `home_dir`).
- **Why not fixed in-commit:** Coupled with the
  module-split in AQ-087, the cleanest shape emerges
  after both changes; doing this visibility cleanup
  standalone would just churn `plugin_host.rs`.
- **Suggested fix:** Move `validate_cdylib_extension`
  (currently in `commands/plugin.rs`) into
  `plugin_host`, similarly wrap the home lookup as a
  named operation. Revert `DYLIB_EXT` and `home_dir`
  to module-private.

### AQ-087 — `commands/plugin.rs` is 1200+ lines and hosts three independent subcommands

- **Date:** 2026-04-20
- **Category:** Module size
- **Commit context:** feat: add `rustwerk plugin
  install` subcommand (v0.47.0)
- **Description:** After PLG-INSTALL,
  `commands/plugin.rs` hosts `list`, `push`, and
  `install` — three subcommands with their own output
  structs, `RenderText` impls, helpers, and test
  sections. CLAUDE.md flags >500 lines + multiple
  structs-with-impls as a split trigger. `install`
  shares almost nothing with `push` (no task DTO, no
  config assembly, no exit-code gymnastics); they're
  colocated only because they share the `plugin` noun.
- **Why not fixed in-commit:** Clean split is a
  mechanical but non-trivial refactor (~1200 lines to
  re-distribute). Out of scope for a feature
  landing; belongs in its own `refactor:` commit.
- **Suggested fix:** Split into
  `commands/plugin/mod.rs` (shared `PluginListItem`,
  `to_list_item`, `RenderText` glue),
  `commands/plugin/list.rs`,
  `commands/plugin/push.rs`,
  `commands/plugin/install.rs`. Each lands in the
  200–400 line band. Tests travel with the code they
  cover.

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

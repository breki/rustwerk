# Artisan Findings — Resolved

Archive of fixed Artisan code quality findings, newest
first. See [artisan-log.md](artisan-log.md) for open
findings.

---

### PLG-JIRA-PARENT review sweep (2026-04-20)

Six findings raised and fixed in the same commit
(v0.53.0):

#### AQ-122 — `MappingWarning::InvalidParentKey` Display used `{raw:?}`

- **Date:** 2026-04-20
- **Category:** Error messages
- **Where:** `crates/rustwerk-jira-plugin/src/warnings.rs`
- **Description:** The newly-added variant used
  `{raw:?}` (Debug formatting), which escapes
  non-ASCII as `\u{...}`. Direct mirror of the
  AQ-120 regression fixed one commit earlier; every
  other `MappingWarning` arm already used plain `{}`.
- **Resolution:** Changed to `'{raw}'` with single
  quotes.

#### AQ-123 — `project/mod.rs` exceeded 500 lines with parent-forest logic

- **Date:** 2026-04-20
- **Category:** Module size
- **Where:** `crates/rustwerk/src/domain/project/mod.rs`
- **Description:** Five new parent-forest methods
  (`set_parent`, `unparent`, `is_ancestor_of`,
  `parent_push_levels`, `validate_parent_forest`)
  plus ~120 lines of tests pushed `mod.rs` past the
  500-line threshold while mixing two structurally
  distinct graph concepts (parent forest vs.
  dependency DAG).
- **Resolution:** Extracted `domain/project/parent.rs`
  submodule mirroring the existing
  `domain/project/scheduling.rs` pattern. All five
  methods + 14 focused tests moved; `mod.rs` shrinks
  by ~270 lines.

#### AQ-124 — `parent_push_levels` returned raw `Vec<Vec<TaskId>>`

- **Date:** 2026-04-20
- **Category:** Type safety / API design
- **Where:** `domain/project/parent.rs`
- **Description:** Outer-vs-inner Vec semantics
  (levels vs tasks-within-level) and the
  non-empty-levels invariant were prose-only.
- **Resolution:** Introduced a `pub struct
  PushLevels(Vec<Vec<TaskId>>)` newtype with `len`,
  `is_empty`, `iter`, and `IntoIterator` impls.
  Invariants (no empty levels, deterministic ordering
  within each level) documented on the type. Follows
  Rust API Guidelines C-NEWTYPE.

#### AQ-125 — `cmd_plugin_push` level loop mixed concerns

- **Date:** 2026-04-20
- **Category:** Abstraction boundaries
- **Where:** `crates/rustwerk/src/bin/rustwerk/commands/plugin.rs`
- **Description:** A ~140-line function held config
  assembly, plugin lookup, level loop with reload,
  state persistence, and result aggregation — hard
  to unit-test any piece in isolation.
- **Resolution:** Extracted `fn execute_levels`
  returning a typed `LevelExecution { combined_results,
  save_warnings, any_failure, levels_completed }` and
  `fn build_aggregate_result`. Both unit-testable
  without the full command. Folds in RT-137 and
  RT-139 fixes.

#### AQ-126 — `apply_parent` had a stringly-typed cross-plugin contract

- **Date:** 2026-04-20
- **Category:** Type safety
- **Where:** `crates/rustwerk-jira-plugin/src/mapping.rs` + `push.rs`
- **Description:** The `"key"` literal appeared in
  four places — `build_created_state` (write),
  `existing_issue_key_validated` (read),
  `build_refreshed_state` (read), and
  `apply_parent` (cross-task read). A typo on any
  one would fail silently.
- **Resolution:** Introduced `const STATE_KEY_FIELD:
  &str = "key"` in `push.rs` and reference it from
  every write / read site.

#### AQ-127 — `--parent ""` caught at runtime instead of clap parse time

- **Date:** 2026-04-20
- **Category:** API design
- **Where:** `crates/rustwerk/src/bin/rustwerk/main.rs` + `commands/task.rs`
- **Description:** The empty-string check ran after
  `load_project()`, so a user with a broken project
  couldn't learn they'd typo'd. Also inconsistent
  between `task add` (fell through to `TaskId::new`
  error) and `task update` (custom `bail!` pointing
  at `task unparent`).
- **Resolution:** Added
  `.value_parser(clap::builder::NonEmptyStringValueParser::new())`
  on both `--parent` arg definitions. Clap rejects
  the empty string at parse time. Runtime check
  removed from `cmd_task_update`. Help text already
  says "Use `task unparent` to clear."

---

### PLG-JIRA-FIELDS review sweep (2026-04-20)

Nine findings raised and fixed in the same commit
(v0.52.0):

#### AQ-113 — `lib.rs` exceeded 500 lines with mixed responsibilities

- **Date:** 2026-04-20
- **Category:** Module size
- **Where:** `crates/rustwerk-jira-plugin/src/lib.rs`
- **Description:** Post-PLG-JIRA-FIELDS, `lib.rs` held
  FFI exports, push orchestration, transition logic,
  and warnings in a single 1679-line file.
- **Resolution:** Split into focused submodules —
  `push` (orchestration), `transition` (workflow +
  state splicing), `warnings` (typed `MappingWarning`).
  Production code in `lib.rs` is now FFI-only
  (~200 lines); end-to-end FFI tests stay colocated
  with the exports they exercise.

#### AQ-114 — `status_wire_name` duplicated the serde wire format

- **Date:** 2026-04-20
- **Category:** Type safety
- **Where:** moved from `crates/rustwerk-jira-plugin/src/lib.rs`
  to `crates/rustwerk-plugin-api/src/lib.rs::TaskStatusDto::as_wire`
- **Description:** Hand-rolled `match` from
  `TaskStatusDto` to `"snake_case"` strings duplicated
  the `rename_all = "snake_case"` contract on the DTO.
  A future variant rename would silently desynchronize
  the two.
- **Resolution:** Moved the wire-name helper to the
  DTO itself as a `const fn`, colocated with the serde
  attrs. New test
  `task_status_dto_as_wire_matches_serde_wire_format`
  guards the two against drifting.

#### AQ-115 — `with_last_status` silently swallowed non-object input

- **Date:** 2026-04-20
- **Category:** Correctness / API clarity
- **Where:** `crates/rustwerk-jira-plugin/src/transition.rs::with_last_status`
- **Resolution:** Added `debug_assert!` on the invariant
  and a release-mode fallback that wraps the value in a
  fresh object. (Same fix as RT-135.)

#### AQ-116 — In-place `TaskPushResult` mutation broke builder consistency

- **Date:** 2026-04-20
- **Category:** API design
- **Where:** `crates/rustwerk-plugin-api/src/lib.rs::TaskPushResult`
- **Description:** The crate otherwise used
  `TaskPushResult::ok(...).with_external_key(...)` chained
  builders, but `append_warnings` and
  `maybe_transition_after_write` mutated
  `r.message` / `r.plugin_state_update` directly.
- **Resolution:** Added `with_appended_message(&str)` on
  `TaskPushResult`. Transition / warnings paths now
  decorate via the builder style exclusively; state
  updates flow through the existing
  `with_plugin_state_update`.

#### AQ-117 — Warnings were stringly-typed `Vec<String>`

- **Date:** 2026-04-20
- **Category:** Type safety / API design
- **Where:** `crates/rustwerk-jira-plugin/src/warnings.rs` (new)
- **Description:** Mapping and transition warnings
  were built with `format!` at each site and joined
  with `"; "`. Tests asserted on substrings; warning
  content could contain the separator, corrupting
  structure; no machine-readable surface for future
  TUI consumption.
- **Resolution:** Introduced
  `enum MappingWarning { UnmappedAssignee, UnmappedPriority, RejectedLabel, TransitionHttp, TransitionTransport }`
  with a `Display` impl that owns the wire format.
  Call sites push typed variants; the render step
  happens once in
  `transition::append_warnings`. Tests now match on
  enum variants.

#### AQ-118 — `; ` separator collided with warning content

- **Date:** 2026-04-20
- **Category:** Correctness (minor)
- **Where:** `crates/rustwerk-jira-plugin/src/transition.rs::append_warnings`
- **Description:** Joined warnings with `"; "`, but
  warning bodies embed Jira HTTP responses that contain
  `;` and `)`.
- **Resolution:** Switched separator to `" | "` (not
  produced by any `MappingWarning::Display` impl) so a
  downstream parser can reliably re-split.

#### AQ-119 — `status_map: HashMap<String, Option<String>>` was ambiguous

- **Date:** 2026-04-20
- **Category:** Type safety
- **Where:** `crates/rustwerk-jira-plugin/src/config.rs::JiraConfig::status_map`
- **Description:** Both `{"done": null}` and `{}`
  produced identical observable behavior — two
  representations collapsing to one semantic.
- **Resolution:** Collapsed to
  `HashMap<String, String>`. Statuses absent from the
  map fire no transition; explicit `null` is no longer
  accepted. Manual + llms.txt updated to match.

#### AQ-120 — `InvalidAssigneeEmail` error used `Debug` formatting

- **Date:** 2026-04-20
- **Category:** Error messages
- **Where:** `crates/rustwerk-jira-plugin/src/config.rs::ConfigError`
- **Description:** `#[error("... key {0:?} ...")]`
  rendered with `Debug` (surrounding `"`, escape
  sequences), inconsistent with sibling variants that
  used `{0}`.
- **Resolution:** Changed to `'{0}'` with explicit
  single-quote delimiters and `Display` formatting.

#### AQ-121 — `BuiltPayload` named after construction verb, not identity

- **Date:** 2026-04-20
- **Category:** API design (minor)
- **Where:** `crates/rustwerk-jira-plugin/src/mapping.rs`
- **Description:** "Built" described how the struct
  was made, not what it is.
- **Resolution:** Renamed to `IssuePayload` (the
  outbound Jira issue payload). `body` + `warnings`
  fields unchanged.

---

### PLG-JIRA-ISSUETYPE review sweep (2026-04-20)

Five findings raised and fixed in the same commit
(v0.51.0):

#### AQ-108 — `issue_type` update bypassed the `Project` domain API

- **Date:** 2026-04-20
- **Category:** Abstraction boundaries
- **Where:** `commands/task.rs`, `batch.rs`
- **Description:** Title/desc flowed through
  `Project::update_task` and tags through
  `Project::set_task_tags` (both bump
  `modified_at`), but the new `issue_type` path
  reached into `project.tasks.get_mut(&id)` from
  the binary, skipping the `modified_at` refresh and
  putting business logic in the wrong layer.
- **Impact:** Project mtime stayed stale after a
  type-only update (regression vs. every other field);
  `DomainError::TaskNotFound` construction duplicated
  between CLI and batch.
- **Resolution:** Added
  `Project::set_task_issue_type(&mut self, id,
  Option<IssueType>)` modeled on `set_task_tags`.
  Both the CLI (`cmd_task_update`) and batch
  (`task.update` dispatch) now go through it. Unit
  tests cover set / clear / `modified_at` bump /
  nonexistent-id error.

#### AQ-109 — `TaskDto.issue_type: Option<String>` vs closed `TaskStatusDto`

- **Date:** 2026-04-20
- **Category:** Type safety
- **Where:** `crates/rustwerk-plugin-api/src/lib.rs`
- **Description:** Sibling classification fields used
  opposite strategies on the same DTO — `status` was
  a closed enum, `issue_type` was stringly-typed —
  with no documented rationale for the asymmetry.
  Future readers would be tempted to "fix" the
  inconsistency by closing the enum, losing
  forward-compat.
- **Impact:** Design-intent leak; drift risk.
- **Resolution:** Expanded the field's doc comment to
  spell out the asymmetry: `Status` is a finite,
  host-controlled workflow; `issue_type` is an open
  classification that the `default_issue_type`
  fallback deliberately accommodates. Plugins are
  documented as responsible for validating and
  falling back on unknown kebab names.

#### AQ-110 — `cmd_task_update` five positional `Option<&str>` arguments

- **Date:** 2026-04-20
- **Category:** API design
- **Where:** `crates/rustwerk/src/bin/rustwerk/commands/task.rs`
- **Description:** The growing `cmd_task_update`
  signature
  (`id, title, desc, tags, issue_type` —
  four same-typed `Option<&str>`) let callers swap
  two arguments without a compiler error. Every new
  optional field (PLG-JIRA-PARENT is next) would
  force another positional shuffle.
- **Impact:** Latent bug class; mechanical
  call-site churn.
- **Resolution:** Introduced `TaskUpdateFields<'a>`
  struct with named fields + an `is_empty()` method
  that drives the "at least one of" check. The
  dispatcher in `main.rs` now builds one struct
  literal instead of juggling positional args.
  Adding a `parent` field later becomes a
  one-field struct extension.

#### AQ-111 — Batch key `"type"` diverged from serialized field `"issue_type"`

- **Date:** 2026-04-20
- **Category:** API design
- **Where:** `crates/rustwerk/src/bin/rustwerk/batch.rs`
- **Description:** Every other batch arg key matched
  the domain field name (`tags`, `desc`, `title`,
  `id`). Using `"type"` for issue-type made the
  batch API the only surface where the key
  disagreed with `project.json`'s `"issue_type"`.
- **Impact:** Foot-gun for anyone scripting batch
  input from templates that also write
  `project.json`.
- **Resolution:** Batch handlers now accept
  `"issue_type"` as canonical and fall back to
  `"type"` as a documented alias via
  `args.get("issue_type").or_else(|| args.get("type"))`.

#### AQ-112 — `IssueType::list_marker` + parallel hand-rolled renderer match

- **Date:** 2026-04-20
- **Category:** Duplication / dead code
- **Where:** `domain/task.rs`, `commands/task.rs`
- **Description:** `list_marker` was defined and
  covered by a uniqueness test, but the `task list`
  renderer ignored it and hand-rolled a parallel
  `match`. Two sources of truth for the same
  mapping; future variants would drift silently
  (uniqueness test would pass, UI would be wrong).
- **Impact:** `list_marker` was dead weight; real
  mapping had no test coverage.
- **Resolution:** Deleted `list_marker` and its
  uniqueness test. The single mapping lives at the
  one call site in `commands/task.rs`.

---

### PLG-JIRA-UPDATE review sweep (2026-04-20)

One finding raised and fixed in the same commit
(`feat: idempotent plugin push jira (probe → update
or recreate)`, v0.50.0). Six more (AQ-101 module
size, AQ-102 verb dedup, AQ-103 borrow-not-own,
AQ-104 ownership consistency, AQ-106 URL truncation,
AQ-107 `MockHttp` builder) were logged open as
deferred polish.

- **AQ-105 — Stringly-typed Jira issue key across
  the whole call chain.** Raw `&str` flowed from
  `plugin_state.key` → `existing_issue_key` →
  `push_one_update` → `get_issue` → `update_issue` →
  URL builders with no validation at any boundary.
  **Fix:** introduced
  `pub(crate) struct IssueKey(String)` with a
  private constructor that enforces
  `[A-Z][A-Z0-9_]*-[0-9]+` and a 64-char length cap.
  `CreatedIssue.key` is now `IssueKey` (was
  `String`); `direct_issue_url` / `gateway_issue_url`
  / `get_issue` / `update_issue` all take
  `&IssueKey`. This is the type-safe encoding of
  the RT-121 defense — a malformed stored value
  cannot reach `format!`-based URL construction
  because it cannot even be constructed. Rust API
  Guidelines C-NEWTYPE. Regression tests
  `issue_key_parse_accepts_valid_keys`,
  `issue_key_parse_rejects_path_traversal_and_other_garbage`.

---

### PLG-JIRA-STATE review sweep (2026-04-20)

Six findings raised and fixed in the same commit
(`feat: jira plugin records created-issue state on
first push`, v0.49.0). AQ-095 / AQ-096 were flagged as
preexisting but folded into this PR's scope since the
error-type change ripples through the new call sites.

- **AQ-095 — `HttpClient` returned
  `Result<HttpResponse, String>`.** Stringly-typed
  errors could not be matched, and callers kept
  re-prefixing (`"HTTP error: HTTP transport error:
  …"`). **Fix:** introduced `HttpError` (via
  `thiserror::Error`) with `Transport`, `TenantInfo`,
  and `TenantInfoDecode` variants; `HttpClient` and
  `create_issue` now return `Result<_, HttpError>`.
  `push_one` formats the error once via `Display`
  with no classification prefix. Regression asserted
  in `push_all_handles_http_errors` (checks the
  double-prefix pattern is gone).

- **AQ-096 — `tenant_info` failures rendered as
  "HTTP error: tenant_info …" (double classification
  prefix).** Subsumed by AQ-095: `HttpError::TenantInfo(500)`
  and `HttpError::TenantInfoDecode(_)` have their own
  `Display` impls, and the caller no longer wraps.
  Assertions in `create_issue_errors_when_tenant_info_fails` /
  `…_missing_cloud_id` updated to `matches!` on the
  typed variants.

- **AQ-097 — `parse_created_issue` returned
  `Option`, collapsing "empty body" and "malformed
  body" into one branch.** Subsumed by RT-118's fix:
  the function now returns `Result<CreatedIssue,
  ParseIssueError>` with distinct `EmptyBody`,
  `Malformed`, `EmptyField`, and `InvalidSelfUrl`
  variants so the caller can decide (silent skip vs.
  visible warning) per category.

- **AQ-098 — `Clock::now_iso8601` returned `String`,
  forcing allocation at the trait boundary.** **Fix:**
  `Clock::now(&self) -> chrono::DateTime<chrono::Utc>`;
  `build_jira_state` owns the `to_rfc3339_opts(Secs,
  true)` formatting call, co-located with the JSON
  blob shape. `FixedClock` now stores a `DateTime<Utc>`
  built via `TimeZone::with_ymd_and_hms`, and
  `build_jira_state_formats_timestamp_as_utc_iso8601_seconds`
  pins the wire format independently of `SystemClock`.

- **AQ-099 — `CreatedIssue.id` was unused
  ("retained for future use").** **Fix:** deleted the
  field. Serde ignores unknown keys by default, so
  `id` can return trivially when a future iteration
  needs it. `parse_created_issue_reads_id_key_and_self`
  renamed to `…_reads_key_and_self`.

- **AQ-100 — `FakeHttp` (lib.rs) duplicated `MockHttp`
  (jira_client.rs).** **Fix:** hoisted to
  `src/test_support.rs` (`#[cfg(test)] mod
  test_support`), with shared `MockHttp`, `Call`,
  `ok`, and `transport_err` helpers. When
  PLG-JIRA-UPDATE adds `put_json` to `HttpClient`,
  only one fake has to change.

---

### PLG-API-STATE review sweep (2026-04-20)

Four findings raised and fixed in the same commit
(`feat: per-task plugin-state round-trip in the
plugin API`, v0.48.0). Two more (AQ-093 Value
newtype, AQ-094 version-history doc placement)
were logged open as deferred polish.

- **AQ-089 — `TaskPushResult` struct-literal
  boilerplate at every construction site.** Adding
  `plugin_state_update` required touching every
  literal — a pattern that would repeat at every
  future v3+ field. **Fix:** added
  `TaskPushResult::ok(task_id, message)` and
  `::fail(task_id, error)` constructors defaulting
  the two optional fields to `None`, plus fluent
  `.with_external_key(k)` and
  `.with_plugin_state_update(v)` setters. Migrated
  all three production sites in the jira plugin
  and the `result_with_task_updates` test helper.
  The pre-existing explicit-field literals in the
  plugin-api tests remain (they deliberately test
  field shapes). New ID fields land going forward
  without mass-editing callers.

- **AQ-090 — `task_to_dto` conflated pure mapping
  with plugin-namespace slicing.** The added
  `plugin_name` parameter forced every test that
  cared only about field mapping to supply a
  spurious `"jira"`. **Fix:** split into two
  functions — `task_to_dto(id, task)` for pure
  domain→DTO mapping (returns `plugin_state: None`)
  and `task_to_dto_for_plugin(id, task, plugin_name)`
  which calls the pure version and slices the
  per-plugin namespace via struct-update syntax.
  Existing tests for base mapping lost their
  spurious plugin name; slicing behavior has its
  own tight tests.

- **AQ-091 — `cmd_plugin_push` save block inlined
  multiple responsibilities.** The apply-state +
  persist + error-handling logic sat inside the
  main dispatch function. **Fix:** extracted
  `persist_plugin_state(root, project, plugin_name,
  pushed_ids, result) -> Option<String>` which
  owns the dirty check, the save call, and the
  save-failure-message formatting (RT-113).
  `cmd_plugin_push` now reads linearly: invoke
  plugin → persist state → emit output.

- **AQ-092 — No integration test for
  save-on-partial-failure.** Unit tests proved the
  dirty flag fires but nothing proved
  `file_store::save` actually ran on an aggregate
  failure. **Fix:** added
  `persist_plugin_state_saves_on_aggregate_failure_when_any_task_succeeded`,
  which drives `persist_plugin_state` with a
  tempdir-backed project, a `PluginResult
  { success: false, ... }`, and a single
  `Some(update)` entry, then reloads from disk to
  confirm the write landed. Two companion tests
  (`persist_plugin_state_writes_on_dirty_and_reloads`,
  `persist_plugin_state_no_op_when_nothing_to_save`)
  cover the happy paths.

### PLG-INSTALL review sweep (2026-04-20)

Five findings raised and fixed in the same commit
(`feat: add rustwerk plugin install subcommand`,
v0.47.0). Two more from the same sweep (AQ-087
module split, AQ-088 visibility) were logged open
as deferred refactors.

- **AQ-082 — `--scope project` silently falls back
  to the current directory.** `cmd_plugin_install`
  did `load_project().or_else(|_| env::current_dir())`,
  so running `rustwerk plugin install foo.dll` from
  outside a rustwerk project would create a stray
  `.rustwerk/plugins/` tree in whatever directory the
  user happened to be in. **Fix:** restructured to
  require `load_project()` only when `scope ==
  Project`; `User` scope runs independent of cwd.
  `resolve_scope_dir` signature updated from
  `project_root: &Path` to `Option<&Path>` and bails
  with a named error when the scope needs a project
  but none was found. Regression test
  `resolve_scope_dir_project_errors_without_project_root`
  pins the behavior.

- **AQ-083 — `PluginInstallOutput::scope` was
  stringly-typed.** The field stored `"project"` /
  `"user"` as a `String`, populated via a hand-rolled
  `InstallScope::as_str().to_string()`. **Fix:**
  `scope: InstallScope` with `#[derive(Serialize)]
  #[serde(rename_all = "snake_case")]` on the enum;
  added `fmt::Display` for text rendering; dropped
  `as_str`. JSON consumers now see the same
  `"project"` / `"user"` values typed through serde.

- **AQ-084 — `verify: &dyn Fn(…)` forced needless
  dynamic dispatch.** The verifier closure is called
  once per `plugin install` invocation in a non-hot
  path; the trait-object indirection existed purely
  for test injection. **Fix:** signature changed to
  `verify: impl Fn(&Path) -> Result<PluginListItem>`.
  All four test call sites work unchanged (closures
  coerce identically).

- **AQ-085 — Tuple return
  `(PluginListItem, PathBuf, bool)` obscured
  meaning.** `install_from_path` returned a 3-tuple
  ending in a bare `bool`, the canonical "mixed up at
  the call site" anti-pattern. **Fix:** introduced a
  private `struct InstallOutcome { info, replaced }`
  and now returns that directly. The `PathBuf` was
  dropped from the outcome because `info.path`
  already carries it (see AQ-086).

- **AQ-086 — `PluginInstallOutput.destination`
  duplicated `installed.path`.** `to_list_item` fills
  `PluginListItem.path` from `loaded.source_path()`,
  which is the final destination after the copy;
  `PluginInstallOutput` was storing the same string
  again in a parallel `destination` field. JSON
  consumers saw two fields with identical values.
  **Fix:** dropped the duplicated field; the
  renderer and JSON output now both read
  `installed.path` as the single source of truth.

### PLG-MAP review sweep (2026-04-20)

Three findings raised and fixed in the same commit
(`feat: render Jira description as ADF`, v0.46.0).

- **AQ-079 — Redundant empty-string branch in
  `adf_doc`.** The function branched on
  `text.is_empty()` to produce a one-element vector,
  but `"".split('\n')` already yields `[""]`, so both
  arms produced the same result. Dead branching
  obscures intent. **Fix:** the `if/else` was
  collapsed into a single
  `normalized.split('\n').map(adf_paragraph).collect()`
  call; the "empty input is still a valid doc"
  guarantee is now expressed in the module doc comment
  and guarded by the
  `adf_doc_is_valid_even_when_input_empty` test.

- **AQ-080 — Description fallback logic lived inside
  `build_issue_payload`.** The "empty description
  falls back to title" rule was inlined in the payload
  builder, mixing policy with mapping and meaning
  `adf_doc` could never be exercised with the raw
  (possibly empty) description. **Fix:** extracted
  `fn description_text(task: &TaskDto) -> &str`
  alongside `summary_for`, so `build_issue_payload`
  now reads as a flat mapping and the fallback policy
  has one named home.

- **AQ-081 — Trailing commas inside `json!`
  literals.** New `json!` sites ended `content`/`text`
  entries with a trailing comma before the closing
  brace, while existing sites in the same file did
  not. **Fix:** dropped the trailing commas for
  consistency with the surrounding code.

### PLG-CLI review sweep (2026-04-19)

Four findings raised and fixed in the same commit
(`feat: add plugin CLI subcommands`, v0.45.0).

- **AQ-075 — `filter_tasks` re-borrow scan.** Same
  codepath as RT-100. Resolution shared: return
  `Vec<(TaskId, &'a Task)>` via
  `HashMap::get_key_value`.

- **AQ-076 — Error-message style diverged from
  neighbouring commands.** `"plugin 'X' not found
  (available: [a, b])"` used a shape not found
  elsewhere.
  **Fix:** rephrased to `"unknown plugin: X (available:
  a, b)"` to mirror the existing
  `"unknown status: X (expected: ...)"` form used by
  `parse_status`.

- **AQ-077 — Unicode `✓`/`✗` glyphs in rendering.** No
  other `RenderText` impl uses these; Windows consoles
  without UTF-8 mojibake them.
  **Fix:** swapped to `[ok]` / `[fail]` text prefixes
  in `render_push_text` and `render_task_result`. All
  tests updated.

- **AQ-078 — `git` shell-out ad-hoc.** `git_user_email`
  was inline in `commands/plugin.rs` with no shared
  home for future `git` callers.
  **Fix:** moved to a new `bin/rustwerk/git.rs` module
  (`git::user_email`) so the next caller has a
  discoverable home.

### PLG-HOST craftsmanship bundle

- **Date:** 2026-04-19
- **Category:** API design / correctness / documentation
- **Commit context:** feat: add dynamic plugin host
  (v0.43.0)
- **Resolution:** Four findings from the Artisan
  review were addressed in the same commit:
  - **Fragile `CStr` borrow + free ordering** in
    `push_tasks` and `call_info`. Extracted
    `parse_plugin_response<T>` helper that takes
    ownership of the byte buffer (via
    `CStr::from_ptr().to_bytes().to_vec()`) before
    returning, so the `CStr` borrow is statically
    dropped before the caller frees the plugin
    pointer. Eliminates the class of
    use-after-free regressions when adding future
    error messages that reference buffer contents.
  - **Missing `LoadedPlugin` invariant doc.** Added
    an explicit `# Invariant` section to the struct
    docs stating `push_tasks` and `free_string`
    must originate from `_library`, and that only
    `load_plugin` should construct the type.
  - **Silent duplicate-name skip.** Shadowed
    plugins now print `"plugin '<name>' at <path>
    shadowed by <higher-path>"` to stderr instead
    of being dropped quietly.
  - **Contradictory field-order comment.** The
    struct had two comments about drop order: one
    asserting fields drop in declaration order so
    `_library` must go last, the other saying
    order is moot. Replaced with a single accurate
    note that fn pointers are `Copy`/no-`Drop`,
    the `Library` outlives them for the struct's
    lifetime, and field order is readability
    only.

- **Also resolved**: `MODULE_COVERAGE_EXEMPT` in
  `xtask/src/main.rs` originally listed both
  forward- and backslash variants per entry (6
  entries for 3 files). Collapsed to forward-slash
  only; the match site normalises the JSON-reported
  path once before comparing.

### AQ-063..072 — CLI-JSON craftsmanship bundle

- **Date:** 2026-04-19
- **Category:** Architecture + API design + error handling
- **Commit context:** feat: add `--json` global output
  flag (v0.42.0)
- **Resolution:** Ten findings from the Artisan review
  of the initial CLI-JSON implementation were
  addressed in the same commit as part of a wholesale
  refactor:
  - **AQ-063** — `json: bool` threaded through every
    `cmd_*` mixed business and presentation logic.
    Refactored: each `cmd_*` now returns an owned DTO
    implementing `Serialize + RenderText`. A new
    `render::emit<T>(&T, OutputFormat)` helper in
    `src/bin/rustwerk/render.rs` picks the renderer;
    the ~20 if/else branches collapsed to one call
    site each in `main.rs`.
  - **AQ-064** — `json_output::print` propagated
    `BrokenPipe` as an error, making
    `rustwerk ... --json | head` exit non-zero with
    a scary message. `render::emit` now treats
    `BrokenPipe` as a clean `Ok(())`.
  - **AQ-065** — `cmd_init` emitted the raw user
    argument instead of the persisted project name.
    Now reads `project.metadata.name` after
    `Project::new` normalisation.
  - **AQ-066** — `cmd_task_describe --json` could
    not distinguish missing from empty (`content:
    null` in both cases). Added explicit `exists:
    bool` field.
  - **AQ-067** — `CompleteReportOutput` duplicated
    all `SummaryJson` fields inline. Now embeds
    `summary: SummaryJson`; the shared shape is
    authoritative.
  - **AQ-068** — `TaskAssignJson` was reused for
    `task unassign` and `DevAddJson` for
    `dev remove`. Introduced a neutral `TaskRef { id,
    title }` / `DevRef { id, name }` pair; the
    renamed `TaskAssignOutput` DTO now models both
    assign and unassign explicitly via
    `Option<DeveloperId>`.
  - **AQ-069** — `created_at` was hand-formatted as
    `String`. Now serialized via serde's default
    `chrono::DateTime<Utc>` encoder (RFC 3339).
  - **AQ-070** — `EffortByDevJson.hours` was
    `f64`; lifted to `Option<f64>` (RT-089) and kept
    the name since the pair `{developer, hours}` is
    unambiguous in its container.
  - **AQ-071** — `print_json` helper went unused
    after the refactor. Deleted.
  - **AQ-072** — `gantt::render_gantt` /
    `tree::render_tree` wrote to stdout directly via
    `print!` / `println!`, which prevented them from
    being invoked from `RenderText::render_text`.
    Both now take `&mut dyn Write` and propagate
    `io::Result`. Tests updated to pass a `Vec<u8>`.

### AQ-056..061 — Installer script craftsmanship (bundle)

- **Date:** 2026-04-19
- **Category:** Error handling + API design + UX
- **Commit context:** chore: add cross-platform install
  scripts
- **Resolution:** Six craftsmanship findings from the
  Artisan review of the new installer scripts were
  fixed in the same commit:
  - **AQ-056** — `install.sh` resolved the latest
    version through an unpiped `curl | sed`, which
    swallowed curl errors and surfaced only an opaque
    "could not resolve latest version." The resolution
    logic now downloads to a tempfile with `dl_to`,
    so network/HTTP errors propagate directly, and
    only falls back to the redirect path when the API
    call actually fails.
  - **AQ-057** — `install.ps1` silently mutated the
    user's persistent PATH while `install.sh` only
    printed a hint, an undocumented contract mismatch.
    The PowerShell script now prints a hint by default
    and only mutates PATH when the caller opts in via
    `RUSTWERK_MODIFY_PATH=1`, matching `install.sh`.
  - **AQ-058** — `RUSTWERK_INSTALL_DIR` was undocumented
    in the README despite being honored by both
    scripts. The README install section now lists all
    three environment overrides (`RUSTWERK_VERSION`,
    `RUSTWERK_INSTALL_DIR`, `RUSTWERK_MODIFY_PATH`).
  - **AQ-059** — The archive layout (`<staging>/rustwerk`)
    was hardcoded; any future packaging change would
    produce "binary not found in archive" with no
    recovery. Both scripts now fall back to a
    recursive search for the binary when the expected
    path is absent.
  - **AQ-060** — `install.sh` used
    `grep " $archive\$"` for the checksum lookup,
    which relied on coincidental whitespace layout
    and would substring-match future entries. Both
    scripts now parse `SHA256SUMS` by splitting on
    whitespace and matching the filename field
    exactly (stripping the leading `*` marker used by
    binary-mode `sha256sum`).
  - **AQ-061** — `install.ps1` created its temp
    directory before the `try/finally`, leaking it if
    interrupted in between. Creation now happens
    inside `try` with the `finally` guarding cleanup.

---

### AQ-049..054 — `rustwerk-plugin-api` code quality fixes (bundle)

- **Date:** 2026-04-19
- **Category:** Error Handling + API Design + Type Safety
- **Commit context:** feat: add `rustwerk-plugin-api` crate
  (v0.40.0)
- **Resolution:** Six Artisan findings from the initial
  review of the new plugin API crate were addressed in the
  same commit:
  - **AQ-049** — `HelperError` exposed concrete upstream
    error types (`serde_json::Error`, `NulError`) in
    public variants, making any upstream major bump a
    breaking change to the plugin API. Inner types are
    now held as `#[source]` only; variant fields are not
    named (tuple variants with `#[source]`), so the
    public surface is stable.
  - **AQ-050** — Hand-rolled `Display` / `Error` /
    `From` impls drifted from the `thiserror` workspace
    convention. `HelperError` now derives via
    `#[derive(thiserror::Error)]`; `thiserror = "2"` was
    added as a direct dependency.
  - **AQ-051** — Error messages had redundant category
    prefixes (`"json error: ..."`) that duplicated
    through `anyhow` chains. Messages now describe the
    failing operation (`"failed to (de)serialize plugin
    payload as JSON"`, `"plugin payload contained an
    interior null byte"`, `"plugin payload exceeds the
    {limit}-byte size cap"`) instead of naming the
    category.
  - **AQ-052** — `TaskDto.status` was stringly-typed.
    Replaced with `TaskStatusDto` enum with
    snake_case wire format covering all five host
    `Status` variants. (Also listed as RT-076; the
    Artisan angle was the API Guidelines C-CUSTOM-TYPE
    violation.) `effort_estimate` remains a string —
    see open AQ-055.
  - **AQ-053** — `PluginApiVersionFn` was typed as
    `unsafe extern "C" fn() -> u32` despite having no
    pointer arguments and a scalar return, forcing
    callers into no-op `unsafe { }` blocks. The alias
    is now `extern "C" fn() -> u32`; the three FFI
    functions that genuinely cross safety boundaries
    (`PluginInfoFn`, `PluginPushTasksFn`,
    `PluginFreeStringFn`) retain `unsafe`.
  - **AQ-054** — `PluginResult.task_results: Vec<_>`
    could not distinguish "operation produced zero task
    results" from "operation doesn't produce per-task
    output". Changed to `Option<Vec<TaskPushResult>>`
    with `#[serde(default, skip_serializing_if =
    "Option::is_none")]`. Tests cover both forms and
    confirm the JSON representations are distinct.

---

### AQ-rename-bundle — `task rename` code quality fixes

- **Date:** 2026-04-19
- **Category:** Abstraction Boundaries / API Design / Error Handling
- **Commit context:** feat: `task rename` command (v0.39.0)
- **Resolution:** Extracted duplicated `.md` rename logic
  from `cmd_task_rename` and the batch post-save loop into
  a reusable `file_store::rename_task_description` helper
  (refuses overwrite, returns a typed
  `DescriptionFileError`) plus `remove_task_description`.
  The batch driver no longer re-parses commands in a
  post-save loop; side effects are collected into a typed
  `FileSideEffect` enum during `execute_one` and replayed
  after `save_project`. `cmd_task_rename` parameter names
  aligned with the clap variant (`old_id, new_id`) for
  end-to-end vocabulary consistency.  `cmd_task_remove`
  and batch `task.remove` now clean up description files,
  matching the lifecycle behavior of `task rename`.
  `unwrap()` on the just-checked `tasks.remove(old_id)`
  replaced with `.expect("existence checked above")` to
  document the invariant at the call site.

### AQ-046 — `run_check` missing `→ cargo ...` trace line

- **Date:** 2026-04-19
- **Category:** UX consistency
- **Commit context:** chore: adopt rustbase template (add
  `xtask check`)
- **Resolution:** Added
  `println!("→ {} check --workspace --message-format=short", cargo_bin());`
  at the top of `run_check` so it matches the trace-line
  convention used by `run_cmd` for every other xtask
  subcommand.

### AQ-044 — Redundant directory creation in version test

- **Date:** 2026-04-07
- **Category:** API Design
- **Commit context:** v0.38.0 `--version` flag
- **Resolution:** Removed redundant `fs::create_dir_all`
  call since `temp_dir()` already creates the directory.

### AQ-045 — Weak version format assertion

- **Date:** 2026-04-07
- **Category:** Type Safety
- **Commit context:** v0.38.0 `--version` flag
- **Resolution:** Replaced `contains('.')` with structured
  assertion that splits on space and verifies 3
  dot-separated version components.

### AQ-041 — Hand-rolled "task not found" in `cmd_task_describe`

- **Date:** 2026-04-04
- **Category:** Abstraction Boundaries
- **Commit context:** v0.37.0 `task describe` command
- **Resolution:** Changed from `anyhow::bail!` to
  `DomainError::TaskNotFound`, consistent with other
  domain-validated commands.

### AQ-042 — Unnecessary `format!` in `task_description_path`

- **Date:** 2026-04-04
- **Category:** API Design
- **Commit context:** v0.37.0 `task describe` command
- **Resolution:** Replaced `format!("{task_id}.md")` with
  `.join(task_id.as_str()).with_extension("md")`.

### AQ-043 — Trailing period inconsistency in messages

- **Date:** 2026-04-04
- **Category:** Error Handling & Messages
- **Commit context:** v0.37.0 `task describe` command
- **Resolution:** Removed trailing period from "No description
  file" message to match majority of CLI output.

### AQ-040 — `--tag` filter silently ignores invalid tags

- **Date:** 2026-04-04
- **Category:** API Design / Consistency
- **Commit context:** v0.36.0 `--tag` filter
- **Resolution:** Added early `Tag::new` validation
  alongside `--chain`/`--status` under the "fail fast"
  comment. Invalid tags now produce a clear error. Uses
  validated `Tag` in the retain closure via
  `t.tags.contains(&tag)`.

### AQ-039 — Encapsulation violation: direct project.tasks access for tags

- **Date:** 2026-04-04
- **Category:** Abstraction Boundaries
- **Commit context:** v0.35.0 `--tags` flag
- **Resolution:** Added `Project::set_task_tags` method
  that handles `modified_at` internally. CLI and batch
  now use this method instead of direct field access.

### AQ-032 — Repetitive `.map_err` boilerplate across codebase

- **Date:** 2026-04-04
- **Category:** Error Handling
- **Commit context:** v0.34.0 map_err removal
- **Resolution:** Removed all 51 occurrences of
  `.map_err(|e| anyhow::anyhow!("{e}"))` across the CLI,
  replaced with plain `?`. `DomainError` already implements
  `std::error::Error` via `thiserror`, so anyhow converts
  automatically. One custom `.map_err` in `batch.rs`
  (for `u32::try_from`) preserved — it has a meaningful
  custom message.

### AQ-038 — File size: task.rs at 614 lines

- **Date:** 2026-04-04
- **Category:** Module Size
- **Commit context:** v0.34.0 tags field
- **Resolution:** Noted but acceptable — file contains
  closely related types. Will extract `Effort` types if
  it grows further.

### AQ-037 — Linear search on a sorted collection

- **Date:** 2026-04-04
- **Category:** Efficiency
- **Commit context:** v0.34.0 tags field
- **Resolution:** Replaced `contains()` with
  `binary_search()` in `add_tag`, `remove_tag`, and
  `has_tag`. Insert uses `binary_search` insertion
  point instead of push+sort.

### AQ-036 — Inconsistent return types: add_tag vs remove_tag

- **Date:** 2026-04-04
- **Category:** API Design
- **Commit context:** v0.34.0 tags field
- **Resolution:** Both `add_tag` and `remove_tag` now
  return `Result<bool, DomainError>` where `bool`
  indicates whether the collection was modified.

### AQ-035 — `Vec<String>` where a `Tag` newtype would be safer

- **Date:** 2026-04-04
- **Category:** Type Safety
- **Commit context:** v0.34.0 tags field
- **Resolution:** Introduced `Tag` newtype with
  `new(s: &str) -> Result<Self, DomainError>`,
  custom `Serialize`/`Deserialize`, `Display`. Field
  changed from `Vec<String>` to `Vec<Tag>`.

### AQ-034 — Missing test for `dev.add` without `id`

- **Date:** 2026-04-04
- **Category:** Error Handling
- **Commit context:** v0.33.0 batch dev commands
- **Resolution:** Added `batch_dev_add_missing_id` test.

### AQ-033 — Inline `use` for developer types in batch

- **Date:** 2026-04-04
- **Category:** API Design
- **Commit context:** v0.33.0 batch dev commands
- **Resolution:** Moved `Developer` and `DeveloperId` imports
  to module-level, removed 3 inline `use` statements from
  match arms.

### AQ-030 — `commands.rs` exceeds 500-line threshold

- **Date:** 2026-04-04
- **Category:** Module Size
- **Commit context:** refactor after v0.32.0
- **Resolution:** Split `commands.rs` (652 lines) into
  five focused modules: `task.rs` (290), `project.rs`
  (145), `report.rs` (177), `dev.rs` (61), `effort.rs`
  (51), with `mod.rs` re-exports. Added error-path
  integration tests and per-module coverage floor (85%).

### AQ-029 — Test does not assert on error message

- **Date:** 2026-04-04
- **Category:** Error Handling
- **Commit context:** v0.32.0 `RUSTWERK_USER` env var
- **Resolution:** Added `stderr.contains("no developer
  specified")` assertion to `task_assign_no_dev_fails`
  test to verify the intended error path triggers.

### AQ-028 — Inconsistent `RUSTWERK_USER` fallback

- **Date:** 2026-04-04
- **Category:** API Design
- **Commit context:** v0.32.0 `RUSTWERK_USER` env var
- **Resolution:** Applied `RUSTWERK_USER` fallback to
  `effort log --dev` (made optional). Extracted shared
  `resolve_developer()` helper used by both `task assign`
  and `effort log` dispatch. Env-var resolution inlined
  in dispatch also resolved (AQ-013 equivalent).

### AQ-020 — scheduling.rs exceeds 500-line module-size rule

- **Date:** 2026-04-03
- **Category:** Module Size
- **Commit context:** v0.30.0 tree command
- **Resolution:** Split scheduling.rs (1,335 lines) into
  five focused modules: `queries.rs` (361), `critical_path.rs`
  (308), `bottleneck.rs` (257), `gantt_schedule.rs` (277),
  `scheduling.rs` (247, kept topo sort + summary). All
  modules now under 400 lines.

### AQ-026 — render_tree writes to stdout, not testable

- **Date:** 2026-04-03
- **Category:** API Design
- **Commit context:** v0.30.0 tree command
- **Resolution:** Changed `render_tree`/`render_node` to
  accept `&mut dyn Write`. Tests now capture output into
  `Vec<u8>` and assert content. Added `render_box_drawing`
  test verifying ├── └── │ characters.

### AQ-025 — build_tree duplicates reverse_dependents

- **Date:** 2026-04-03
- **Category:** Abstraction Boundaries
- **Commit context:** v0.30.0 tree command
- **Resolution:** `build_tree` now calls
  `self.reverse_dependents()` and filters/sorts the result
  instead of building its own map. Made
  `reverse_dependents` `pub(super)`.

### AQ-024 — scheduling.rs now 1,609 lines

- **Date:** 2026-04-03
- **Category:** Module Size
- **Commit context:** v0.30.0 tree command
- **Resolution:** Extracted `task_tree()`,
  `task_tree_remaining()`, `build_tree()`, and
  `build_subtree()` into new `domain/project/tree.rs`
  module with their tests. scheduling.rs: 1,609→1,335.

### AQ-023 — Bottleneck report mislabels ON_HOLD as ready/blocked

- **Date:** 2026-04-03
- **Category:** Type Safety
- **Commit context:** v0.29.0 ON_HOLD status
- **Description:** Bottleneck state label used if/else
  chain that would label ON_HOLD tasks as "ready" or
  "blocked" instead of "on hold".
- **Resolution:** Added explicit `Status::OnHold` branch
  returning "on hold" label.

### AQ-022 — Missing OnHold → InProgress transition

- **Date:** 2026-04-03
- **Category:** API Design
- **Commit context:** v0.29.0 ON_HOLD status
- **Description:** Duplicate of RT-042.
- **Resolution:** Fixed under RT-042.

### AQ-021 — O(V+E) full-graph sort in dependency_chain

- **Date:** 2026-04-03
- **Category:** API Design
- **Commit context:** v0.28.0 task list filters
- **Description:** `dependency_chain()` originally called
  `topological_sort()` on the entire graph to order a
  small subgraph result.
- **Resolution:** Replaced with iterative DFS post-order
  traversal that only visits the reachable subgraph,
  giving O(|subgraph|) instead of O(V+E).

### AQ-019 — Dead guard duplicating domain logic in binary

- **Date:** 2026-04-03
- **Category:** Abstraction Boundaries
- **Commit context:** v0.28.0 task list filters
- **Description:** `cmd_task_list` had an explicit
  `contains_key` check before calling `dependency_chain`,
  duplicating the domain's responsibility for validating
  task existence.
- **Resolution:** Changed `dependency_chain` to return
  `Result<Vec<&TaskId>, DomainError>` with a
  `TaskNotFound` error. Removed the duplicate guard.

### AQ-018 — --status not conflicting with --available/--active

- **Date:** 2026-04-03
- **Category:** API Design
- **Commit context:** v0.28.0 task list filters
- **Description:** Duplicate of RT-037.
- **Resolution:** Fixed under RT-037 (added
  `conflicts_with_all`).

### AQ-017 — Presentation layer reaches into domain internals

- **Date:** 2026-04-03
- **Category:** Abstraction Boundaries
- **Commit context:** v0.27.0 report bottlenecks command
- **Description:** `cmd_report_bottlenecks` accessed
  `project.tasks[&bn.id]` directly to enrich the `Bottleneck`
  with assignee and status, punching through the abstraction.
- **Resolution:** Enriched `Bottleneck` struct with `status`,
  `assignee`, and `ready` fields populated in
  `bottlenecks()`. CLI no longer touches `project.tasks`.

### AQ-016 — Redundant status match duplicates Display impl

- **Date:** 2026-04-03
- **Category:** Type Safety
- **Commit context:** v0.27.0 report bottlenecks command
- **Description:** `cmd_report_bottlenecks` hand-rolled a
  `match` on `Status` to produce display strings, duplicating
  the existing `Display` impl.
- **Resolution:** Now uses `bn.status` directly in the format
  string, which calls `Display` automatically.

### AQ-015 — Module size: scheduling.rs over 1000 lines

- **Date:** 2026-04-03
- **Category:** Module Size
- **Commit context:** v0.26.0 bottleneck detection
- **Description:** `scheduling.rs` exceeded 500 lines with
  `GanttRow` and `ProjectSummary` structs alongside scheduling
  algorithms.
- **Resolution:** Extracted `GanttRow` to `gantt_row.rs` and
  `ProjectSummary` to `summary.rs`. Re-exported from
  `mod.rs` to preserve public API.

### AQ-014 — Tuple return type `(TaskId, usize)` in bottlenecks

- **Date:** 2026-04-03
- **Category:** Type Safety
- **Commit context:** v0.26.0 bottleneck detection
- **Description:** `bottlenecks()` returned `Vec<(TaskId,
  usize)>` — callers would use `.1` for the count with no
  semantic clarity.
- **Resolution:** Introduced `Bottleneck` struct with `id` and
  `downstream_count` fields.

### AQ-013 — Repeated reverse-adjacency graph building

- **Date:** 2026-04-03
- **Category:** API Design
- **Commit context:** v0.26.0 bottleneck detection
- **Description:** The reverse adjacency map was built in three
  places (`topological_sort`, `remaining_critical_path`,
  `bottlenecks`) with slightly different filters, already
  diverging on status semantics.
- **Resolution:** Extracted `reverse_dependents()` private
  helper with a filter predicate. Used in `bottlenecks()`;
  the other two call sites retain their own logic for now
  since they also build `in_degree` maps.

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

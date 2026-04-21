# Red Team Findings — Resolved

Archive of fixed red team findings, newest first.
See [redteam-log.md](redteam-log.md) for open findings.

---

### PLG-JIRA-E2E review sweep (2026-04-20)

Six findings raised and fixed in the same commit
(test-only; no version bump):

#### RT-141 — `ureq::Error` Display in teardown stderr was a credential-leak footgun

- **Date:** 2026-04-20
- **Category:** Security (fragile)
- **Where:** `crates/rustwerk-jira-plugin/tests/jira_live.rs` — `TeardownGuard::drop` error arm, `issue_exists` error paths
- **Description:** `eprintln!("... {e}")` printed the
  full `ureq::Error` Display, which can include the
  request URL. Today auth sits in the `Authorization`
  header (safe), but a future ureq variant or a
  refactor that embeds auth in the URL would leak the
  real API token into stderr / CI logs.
- **Resolution:** Added `redact_ureq_error(&e)` that
  renders `HTTP {status}` or `transport ({kind})`
  only, never the full Display. Used at every
  error-rendering site in the test.

#### RT-142 — `non_empty_env` accepted whitespace-only values

- **Date:** 2026-04-20
- **Category:** Correctness (UX)
- **Where:** `jira_live.rs::non_empty_env`
- **Description:** `.filter(|s| !s.is_empty())` let
  whitespace-only env vars through. A trailing-space
  token from a `.env` file or shell heredoc would
  reach Jira and produce a baffling 401 instead of a
  clean skip.
- **Resolution:** Changed predicate to
  `!s.trim().is_empty()`.

#### RT-143 — `panic = "abort"` would silently invalidate the panic-path test

- **Date:** 2026-04-20
- **Category:** Correctness (future-fragility)
- **Where:** `jira_live.rs::jira_live_teardown_runs_on_panic`
- **Description:** The test uses `catch_unwind` to
  prove the Drop guard runs on panic. Under
  `panic = "abort"`, Drop never runs and the test
  becomes meaningless (process aborts mid-assertion).
- **Resolution:** Gated the test on
  `#[cfg(panic = "unwind")]`. If a future profile
  flips to abort panics, the function drops out of
  the test binary rather than silently lying about
  what it proves.

#### RT-144 — 2.5s DELETE-propagation budget was flaky

- **Date:** 2026-04-20
- **Category:** Correctness (flake vector)
- **Where:** `jira_live_teardown_runs_on_panic`
- **Description:** 5 × 500ms polling was borderline
  for a slow tenant; under load a DELETE-followed-by-
  GET race could produce false-positive "still
  exists" readings.
- **Resolution:** Bumped to a 10s deadline with
  exponential backoff (250ms → 500ms → 1s → 2s).

#### RT-145 — Teardown guard attached after shape assertions leaked issues on partial failure

- **Date:** 2026-04-20
- **Category:** Correctness
- **Where:** `push_one_issue` (original version)
- **Description:** The helper asserted `code == 0`
  and `parsed["success"] == true` *before* returning,
  so the caller couldn't construct the guard until
  the function had already panicked on any malformed
  response. If Jira accepted the POST (issue exists)
  but the plugin later panicked on a shape check, the
  issue leaked.
- **Resolution:** Refactored `push_one_issue` to
  return `(TeardownGuard, PushedIssue)`. The key is
  extracted from the raw payload *before* any
  assertion fires; the guard wraps it immediately so
  every subsequent panic still triggers teardown.

#### RT-146 — Same partial-failure leak window when plugin reports `success: false` with a valid key

- **Date:** 2026-04-20
- **Category:** Correctness
- **Description:** Same root cause as RT-145: a
  future plugin path that reports `success: false`
  while still having populated `external_key` (e.g.
  post-create transition failure) would leak the
  created issue because the `success == true` assert
  fired before key extraction.
- **Resolution:** Folded into the RT-145 fix — key
  extraction now happens *before* any success
  assertion.

---

### PLG-JIRA-PARENT review sweep (2026-04-20)

Five findings raised and fixed in the same commit
(v0.53.0):

#### RT-136 — `parent_push_levels` silently truncated runtime cycles

- **Date:** 2026-04-20
- **Category:** Correctness
- **Where:** `crates/rustwerk/src/domain/project/parent.rs::parent_push_levels`
- **Description:** A defensive `if d > self.tasks.len()
  { break; }` capped depth when an in-memory cycle
  somehow survived load validation (e.g. a future
  code path that bypasses `file_store::load`). The
  task then landed in a truncated level and got
  pushed out of parent/child order.
- **Resolution:** Changed return type to
  `Result<PushLevels, DomainError>`. A revisit in the
  ancestor walk now returns `CycleDetected` loudly.
  Regression test
  `parent_push_levels_rejects_runtime_cycle`.

#### RT-137 — Partial-run aggregate message claimed full level count

- **Date:** 2026-04-20
- **Category:** Correctness
- **Where:** `crates/rustwerk/src/bin/rustwerk/commands/plugin.rs`
- **Description:** When the between-levels project
  reload failed at level N > 0, the loop broke but
  the aggregate message still reported
  `"pushed across {levels.len()} level(s)"` — the
  planned count, not the executed count.
- **Resolution:** Extracted a `LevelExecution` outcome
  struct with `levels_completed`; new
  `build_aggregate_result` helper reports
  `"X of Y level(s) completed"` when partial and
  flags success = false. Tests
  `aggregate_result_flags_partial_run_when_reload_failed`
  and `aggregate_result_reports_full_count_on_clean_run`.

#### RT-138 — Empty task selection silently succeeded

- **Date:** 2026-04-20
- **Category:** Correctness (UX)
- **Where:** `cmd_plugin_push`
- **Description:** `rustwerk plugin push --tasks
  "status=typo'd"` produced `0 task(s) pushed across
  0 level(s)` with `success: true` and exit code 0 —
  indistinguishable from "everything already done."
- **Resolution:** Empty selection now returns a
  `PluginResult` with `success: false` and message
  `"no matching tasks to push (filter produced empty
  selection)"`.

#### RT-139 — `save_warning.get_or_insert` swallowed later level warnings

- **Date:** 2026-04-20
- **Category:** Correctness (minor)
- **Where:** `cmd_plugin_push`
- **Description:** Only the first save warning across
  the level loop was retained; later levels' failures
  were lost. Operator would see one problem, fix it,
  run again, discover another unrelated one.
- **Resolution:** Collected into `Vec<String>`;
  `LevelExecution::save_warning` joins with `" | "`
  (a separator none of the individual messages
  produce). Test
  `level_execution_save_warning_joins_with_pipe`.

#### RT-140 — `is_ancestor_of` could infinite-loop on runtime-corrupted forest

- **Date:** 2026-04-20
- **Category:** Hardening (minor)
- **Where:** `crates/rustwerk/src/domain/project/parent.rs::is_ancestor_of`
- **Description:** If load-time cycle validation was
  ever bypassed (future refactor), the ancestor-walk
  would hang the CLI.
- **Resolution:** Added a `HashSet<TaskId> seen` guard
  that returns `false` on revisit. Regression test
  `is_ancestor_of_handles_runtime_cycle_without_hanging`.

---

### PLG-JIRA-FIELDS review sweep (2026-04-20)

Three findings raised and fixed in the same commit
(v0.52.0):

#### RT-133 — Update path silently skips transition when stored state is malformed

- **Date:** 2026-04-20
- **Category:** Correctness
- **Where:** `crates/rustwerk-jira-plugin/src/transition.rs::maybe_transition_after_write`
- **Description:** The transition helper early-returned
  on `result.plugin_state_update.is_none()`, but the
  update path leaves that field `None` whenever
  `build_refreshed_state` rejects a malformed stored
  blob (missing `self`). The PUT still succeeded and
  the issue key was known good, yet the transition
  silently never fired.
- **Impact:** A user whose `plugin_state.jira` lost its
  `self` field via a manual edit would silently drift
  out of workflow sync — no warning, no retry, Jira
  status never updates.
- **Resolution:** Gate on `external_key.is_some()`
  instead. When the state is absent, `record_last_status`
  synthesizes a minimal `{ "last_status": … }` blob so
  the idempotency anchor still lands. New regression
  test `update_still_fires_transition_when_stored_state_malformed`.

#### RT-134 — `apply_labels` converted a single bad tag into a duplicate-issue factory

- **Date:** 2026-04-20
- **Category:** Correctness
- **Where:** `crates/rustwerk-jira-plugin/src/mapping.rs::apply_labels`
- **Description:** With `labels_from_tags: true`, tags
  were forwarded verbatim. Jira labels reject
  whitespace / control chars, so a tag like
  `"tech debt"` caused the create POST to return 400 —
  the whole task failed, no state was recorded, and
  the next push created a duplicate Jira issue.
- **Impact:** One malformed tag per task turned the
  feature into a duplicate-issue generator, inconsistent
  with the "skip + warn" policy used for assignee /
  priority mapping.
- **Resolution:** Added `is_valid_jira_label` predicate
  (rejects empty, whitespace, control chars). Invalid
  tags are dropped with a typed
  `MappingWarning::RejectedLabel`; valid tags still
  flow through. When every tag is rejected, the field
  is omitted entirely. Unit tests cover the predicate
  and the field-omission case.

#### RT-135 — `with_last_status` silently no-op on non-object state

- **Date:** 2026-04-20
- **Category:** Correctness (low)
- **Where:** `crates/rustwerk-jira-plugin/src/transition.rs::with_last_status`
- **Description:** If the input `serde_json::Value`
  wasn't an object, the function returned it
  unchanged. The caller then persisted it, believing
  `last_status` had been inserted. Future pushes would
  keep re-firing transitions because the idempotency
  field was never actually written.
- **Impact:** Low today — all current state producers
  emit objects. But a silent no-op on a future state
  shape drift would cause chatty transitions forever.
- **Resolution:** `debug_assert!(state.is_object(), …)`
  catches the bug in dev; release-mode falls back to
  wrapping the value in a fresh
  `{ "last_status": … }` object so the anchor lands
  either way. Test
  `with_last_status_panics_on_non_object_in_debug`
  guards the invariant.

---

### PLG-JIRA-ISSUETYPE review sweep (2026-04-20)

Four findings raised and fixed in the same commit
(v0.51.0):

#### RT-129 — Unknown kebab bypasses `default_issue_type` fallback

- **Date:** 2026-04-20
- **Category:** Correctness
- **Where:** `crates/rustwerk-jira-plugin/src/config.rs`
- **Description:** `resolve_issue_type_name` used to
  pass an unrecognized kebab name through verbatim
  (e.g. `"bug"` → `{"name":"bug"}`), bypassing the
  user-configured `default_issue_type` safety net.
- **Impact:** Future domain variant or corrupted
  `project.json` would produce a JSON payload Jira
  rejects (HTTP 400) instead of falling back to the
  configured default.
- **Resolution:** Unknown / implausible kebab wire
  values now fall through to
  `default_issue_type` → `"Task"`. The fallback chain
  doc-comment was rewritten to match.

#### RT-130 — `"subtask"` alias asymmetry

- **Date:** 2026-04-20
- **Category:** Correctness
- **Where:** `crates/rustwerk-jira-plugin/src/config.rs`
- **Description:** CLI accepted `"subtask"` and
  `"sub-task"` as aliases, but the plugin looked up
  `issue_type_map` with an exact match. A user who
  wrote `issue_type_map: { "subtask": "Subtask" }`
  silently got the built-in `"Sub-task"` instead.
- **Impact:** Silent map-override drop.
- **Resolution:** Added
  `canonicalize_issue_type_kebab` which folds
  `"subtask"` → `"sub-task"`; applied at config
  load-time (normalizes user-supplied keys) and at
  resolve-time (normalizes incoming wire values).

#### RT-131 — `task update --type story` alone could regress

- **Date:** 2026-04-20
- **Category:** Correctness
- **Where:** `crates/rustwerk/src/bin/rustwerk/commands/task.rs`
- **Description:** The CLI called `project.update_task`
  unconditionally even when only `--type` was set,
  then did a second `tasks.get_mut(...)` for the
  `issue_type` mutation. If `update_task(None, None)`
  ever began returning `Err`, type-only updates would
  fail before the type landed.
- **Impact:** Fragile coupling; unreachable
  `TaskNotFound` branch in the binary.
- **Resolution:** Subsumed by AQ-108 — `cmd_task_update`
  now calls `project.set_task_issue_type` through the
  domain API, which owns the existence check.

#### RT-132 — No length/charset validation on wire `issue_type`

- **Date:** 2026-04-20
- **Category:** Security (low)
- **Where:** `crates/rustwerk-jira-plugin/src/config.rs`
- **Description:** `TaskDto.issue_type: Option<String>`
  is intentionally stringly-typed on the wire, but
  the plugin forwarded whatever arrived straight into
  the Jira payload with no length or charset cap.
- **Impact:** Low — `serde_json` blocks JSON breakout,
  but a malicious/buggy upstream could push arbitrarily
  long or control-char-laden strings that land in
  Jira's validator as opaque 400s.
- **Resolution:** Added
  `is_plausible_issue_type_wire` (bounded 64 bytes,
  no control chars, non-empty); implausible values
  fall through to the default fallback rather than
  being forwarded.

---

### PLG-JIRA-UPDATE review sweep (2026-04-20)

Three findings raised and fixed in the same commit
(`feat: idempotent plugin push jira (probe → update
or recreate)`, v0.50.0). Four more (RT-124 probe
body not validated, RT-125 gateway flag in message,
RT-126 TOCTOU test, RT-127 `jira_url` scheme check)
were logged open as deferred hardening. RT-125 was
folded into the RT-122 fix and is therefore resolved
rather than logged open.

- **RT-121 — Path traversal via stored Jira key.**
  `existing_issue_key` read `plugin_state.jira.key`
  with no validation and spliced it straight into
  URL builders. A poisoned `project.json` with
  `"key": "../../admin/application-properties"`
  would cause the plugin to issue authenticated
  `GET`/`PUT` requests against arbitrary Jira paths.
  **Fix:** new `IssueKey(String)` newtype with a
  private constructor validating
  `[A-Z][A-Z0-9_]*-[0-9]+` and length ≤ 64 chars.
  URL builders and verb signatures take
  `&IssueKey`; a malformed stored value is surfaced
  as `ExistingKey::Invalid(raw)` and fails the task
  loudly (`"stored Jira key … is not a valid issue
  key — refusing to splice it into a URL"`).
  Regression tests
  `existing_issue_key_malformed_value_returns_invalid_variant`,
  `push_fails_loudly_when_stored_key_is_invalid`,
  `issue_key_parse_rejects_path_traversal_and_other_garbage`,
  `parse_created_issue_rejects_invalid_issue_key`.

- **RT-122 — Ambiguous probe 404 (direct 401 +
  gateway 404) silently created duplicate Jira
  issues.** The old `push_one_update` only checked
  `probe.status == 404`, ignoring whether the direct
  read was authoritative. Direct 401 + gateway 404 →
  recreate → duplicate issue while the original
  stayed alive and orphaned. **Fix:** `get_issue`
  now returns a `ProbeOutcome` enum (`Exists`,
  `MissingConfirmed`, `MissingAmbiguous`,
  `OtherStatus`). `MissingConfirmed` (direct 404 +
  gateway 404) is the only state that triggers a
  recreate; `MissingAmbiguous` fails the task with a
  clear message telling the operator to check token
  scope. Regression test
  `ambiguous_probe_404_fails_without_recreating_duplicate`
  (lib.rs) + `get_issue_direct_401_and_gateway_404_returns_missing_ambiguous`
  (jira_client.rs).

- **RT-123 — Refresh path silently dropped additive
  state fields.** `build_refreshed_state`
  reconstructed the state blob with only
  `{key, self, last_pushed_at}`, violating the
  `build_created_state` docstring's "additive
  fields" contract. A future plugin version writing
  `last_hash` would lose it on the first successful
  `PUT`. **Fix:** refresh now clones the existing
  `Object`, validates `key`/`self` are string-typed,
  and mutates only `last_pushed_at` in place,
  preserving every other field verbatim. Regression
  test `refresh_preserves_additive_state_fields`.

- **RT-125 — Update-path message omitted `via
  gateway` when only the probe used it.** Folded
  into the RT-122 fix: `push_one_update` now threads
  the probe's `used_gateway` into
  `task_result_from_update_outcome`, which ORs it
  with the PUT's flag. Regression test
  `update_message_reports_gateway_when_probe_alone_used_it`.

---

### PLG-JIRA-STATE review sweep (2026-04-20)

Three findings raised and fixed in the same commit
(`feat: jira plugin records created-issue state on
first push`, v0.49.0). All centered on
`parse_created_issue` being silently permissive.

- **RT-118 — Silent parse failure on 2xx body lets a
  Jira/proxy schema drift cause unbounded duplicate
  issues.** `parse_created_issue` was `Option`-based
  and swallowed every failure as `None`; the caller
  still reported `success: true` with no state update,
  so the next push would create a second issue, then
  a third, with no log or warning. **Fix:** changed
  the return type to `Result<CreatedIssue,
  ParseIssueError>`. In `task_result_from_outcome`,
  `EmptyBody` (e.g. 204) stays a silent skip, but any
  other variant appends a visible `(WARNING: …; plugin
  state not recorded — next push may create a
  duplicate Jira issue)` to the `TaskPushResult`
  message. Regression test
  `success_with_malformed_body_warns_in_message`.

- **RT-119 — Empty-string `key` / `self` accepted and
  persisted as idempotency anchor.** A response of
  `{"key":"","self":""}` deserialized cleanly and set
  `external_key: Some("")`, breaking PLG-JIRA-UPDATE's
  create-vs-update dispatch. **Fix:**
  `parse_created_issue` now returns
  `ParseIssueError::EmptyField { field: "key" | "self" }`
  on empty values. Regression tests
  `parse_created_issue_empty_key_is_rejected`,
  `parse_created_issue_empty_self_is_rejected`,
  `success_with_empty_key_warns_in_message`.

- **RT-120 — `self` URL written to persisted project
  state without scheme validation.** A compromised
  Jira or MitM could inject `javascript:`, `file:///`,
  or phishing URLs into plugin state, which downstream
  viewers (UI, reports, AI-agent prompts) would treat
  as trusted. **Fix:** `parse_created_issue` now
  parses `self_url` via `url::Url` and rejects any
  scheme other than `http`/`https` as
  `ParseIssueError::InvalidSelfUrl`. Regression tests
  `parse_created_issue_rejects_non_http_scheme`,
  `parse_created_issue_rejects_file_scheme`,
  `success_with_non_http_self_url_warns_in_message`.

---

### PLG-API-STATE review sweep (2026-04-20)

Five findings raised and fixed in the same commit
(`feat: per-task plugin-state round-trip in the
plugin API`, v0.48.0). Three more from the same
sweep (RT-115 plugin-name case sensitivity, RT-116
project.json write race, RT-117 v1-plugin compat
shim) were logged open as deferred hardening.

- **RT-110 — No size cap on `plugin_state_update`.**
  `apply_state_updates` was storing plugin-returned
  blobs verbatim with no bound. A buggy or hostile
  plugin could grow project.json unboundedly across
  pushes, each under the per-call 10 MiB cap.
  **Fix:** per-entry 64 KiB cap (constant
  `MAX_PLUGIN_STATE_UPDATE_BYTES`); oversized
  updates are logged to stderr and skipped.
  Regression test
  `apply_state_updates_rejects_oversized_blobs`.

- **RT-111 — Cross-task state injection.** Plugin
  response could include `plugin_state_update`
  entries for tasks the host didn't select; the
  previous `apply_state_updates` wrote them anyway,
  stamping state onto excluded tasks. **Fix:**
  `cmd_plugin_push` now collects a
  `HashSet<TaskId>` of pushed IDs and passes it to
  `apply_state_updates`, which rejects any entry
  not in the set. Regression test
  `apply_state_updates_rejects_entries_for_tasks_not_pushed`.

- **RT-112 — Silent drop when `TaskId::new` fails or
  the task is missing/excluded.** The
  `let ... else { continue }` arms were silently
  discarding updates with no user-visible signal.
  Next push would create a duplicate external
  resource. **Fix:** each skip path now emits a
  diagnostic `eprintln!("rustwerk: plugin '…' …")`
  matching the existing `discover_plugins` pattern.
  The four skip reasons (invalid ID, not in pushed
  set, unknown task, oversized blob) each have a
  distinct message.

- **RT-113 — Save failure after successful push
  silently orphaned external side effects.**
  `cmd_plugin_push` used `?` on `file_store::save`,
  so a save failure hid the successful `PluginResult`
  from the user — losing the external keys they
  would need to recover manually, and guaranteeing
  the next push creates duplicates. **Fix:**
  extracted `persist_plugin_state` which returns
  `Option<String>` (a human-readable save warning);
  `PluginPushOutput::Executed` grew a
  `save_warning: Option<String>` field; `is_success`
  returns false when a warning is present so the
  process exits non-zero; the renderer prints the
  plugin result first, then the `WARNING: …` line.
  Regression test
  `is_success_false_when_save_warning_set_even_if_plugin_succeeded`.

- **RT-114 — `Value::Null` stored verbatim,
  contradicting the "no clear variant" docstring.**
  `Some(Null)` was silently persisted as a stored
  null entry, breaking the next push's distinction
  between absent and null. **Fix:**
  `apply_state_updates` now treats
  `update.is_null()` as a no-op. Regression test
  `apply_state_updates_rejects_null_updates`.

### PLG-INSTALL review sweep (2026-04-20)

Two findings raised and fixed in the same commit
(`feat: add rustwerk plugin install subcommand`,
v0.47.0). Two more (RT-108, RT-109) from the same
sweep were logged open as deferred hardening.

- **RT-106 — Symlink destination allows writing
  plugin bytes outside `plugins/`.** `install_from_path`
  checked `dest.exists()` then called `fs::copy`,
  which on every platform follows symlinks at the
  destination. A pre-existing symlink like
  `.rustwerk/plugins/evil.dll -> ~/.bashrc` on the
  `--force` path would cause `rustwerk plugin
  install` to overwrite `~/.bashrc` with plugin bytes.
  **Fix:** added `reject_symlink_dest` which calls
  `fs::symlink_metadata` and bails when the
  destination is a symlink. `NotFound` passes through
  (fresh install is legal); other stat errors are
  surfaced instead of silently swallowed. Regression
  test `install_from_path_rejects_symlink_destination`
  (Unix-only; Windows symlink creation requires
  elevation) proves the link target is preserved.

- **RT-107 — `source == dest` silently deletes an
  existing plugin.** Running
  `rustwerk plugin install .rustwerk/plugins/foo.dll
  --force` took a shortcut through `fs::copy(src, dst)`
  where `src == dst`: on Linux this truncates the
  file to zero bytes before copying, then the
  follow-up `verify(&dest)` fails on the empty file
  and the verify-cleanup branch deletes it — silently
  wiping the user's installed plugin. **Fix:** added
  `reject_self_copy` which canonicalises both paths
  (only when `dest` already exists) and bails when
  they resolve to the same inode/file-id. Regression
  test `install_from_path_rejects_self_copy` proves
  the existing file survives intact when the guard
  triggers.

### PLG-MAP review sweep (2026-04-20)

Two findings raised and fixed in the same commit
(`feat: render Jira description as ADF`, v0.46.0).

- **RT-104 — CRLF line endings produce trailing `\r` in
  ADF text nodes.** `adf_doc` at `mapping.rs:53` used
  `text.split('\n')`, which splits only on LF. A
  Windows-style description with `\r\n` line endings
  left a trailing `\r` on every line, embedded verbatim
  in the ADF `text` node. Jira's ADF validator rejects
  text nodes containing control chars other than
  `\t`/`\n`, so a Windows user whose description came
  from a CRLF file would see `POST /rest/api/3/issue`
  fail with HTTP 400 (or render garbled paragraphs).
  **Fix:** `adf_doc` now normalizes both `\r\n` and
  bare `\r` to `\n` before splitting. Regression tests
  `payload_description_normalizes_crlf` and
  `payload_description_normalizes_bare_cr` cover both
  cases.

- **RT-105 — ASCII control chars pass through
  unfiltered.** `adf_paragraph` embedded input verbatim
  in a `text` node. Control chars (U+0000–U+001F
  except `\t`/`\n`, plus U+007F) are rejected by ADF's
  schema, so a single stray `\x0c` or ANSI escape
  sequence from pasted terminal output caused the POST
  to fail with an opaque HTTP 400 instead of a local
  validation error. **Fix:** `adf_paragraph` now
  filters out control chars other than `\t` before
  building the text node. Regression test
  `payload_description_strips_control_chars` proves
  form-feed and escape bytes are dropped;
  `payload_description_preserves_tabs` proves `\t`
  still survives.

### PLG-CLI review sweep (2026-04-19)

Four findings raised and fixed in the same commit
(`feat: add plugin CLI subcommands`, v0.45.0).

- **RT-100 — `filter_tasks` quadratic scan.** After a
  successful `HashMap::get`, the code walked every key
  in `project.tasks` via `.keys().find(...)` just to
  satisfy the borrow checker — O(N·M) for N requested
  IDs over M project tasks.
  **Fix:** replaced with
  `project.tasks.get_key_value(&task_id)` (single
  lookup); `filter_tasks` now returns
  `Vec<(TaskId, &'a Task)>` with the cheap `TaskId`
  cloned.

- **RT-101 — Per-task failure detail dropped on plugin
  failure.** `cmd_plugin_push` returned `Err(anyhow!)`
  on `result.success == false`, which short-circuited
  `render::emit`; users saw only the top-level message
  with no indication of which task failed.
  **Fix:** `cmd_plugin_push` always returns
  `Ok(Executed{..})`; new `PluginPushOutput::is_success`
  drives the exit code from `dispatch_plugin` in
  `main.rs` after the output is rendered.

- **RT-102 — `plugin list` required a rustwerk
  project.** `cmd_plugin_list` called `load_project()`,
  so running the diagnostic command outside a project
  failed with "not a rustwerk project".
  **Fix:** falls back to `env::current_dir()` when no
  project is found — user-scoped
  `~/.rustwerk/plugins/` discovery still works.

- **RT-103 — Plugin-reported names rendered verbatim
  into host output.** A malicious plugin could embed
  ANSI escapes or newlines in `PluginInfo.name` and
  see them reflected into `plugin list` and error
  messages.
  **Fix:** new `validate_plugin_name` in
  `plugin_host.rs` runs immediately after `call_info`
  and rejects anything outside `[A-Za-z0-9_-]+` or
  longer than 64 chars; five tests cover the allowed
  shapes and rejection paths.

---

### PLG-JIRA hardening sweep (2026-04-19)

Seven findings raised and fixed in the same commit
(`feat: add jira plugin`, v0.44.0). All were discovered
during red-team review of the new Jira plugin cdylib
before it was exposed to any end-user flow.

- **RT-093 — Gateway fallback leaked token to
  attacker-controlled `jira_url`.** If a project
  supplied `jira_url` pointing at a host the attacker
  controlled, the plugin would POST the Basic-auth
  credentials to that host, follow a crafted
  `_edge/tenant_info` response containing any
  `cloudId`, then replay the same credentials +
  payload to
  `https://api.atlassian.com/ex/jira/{attacker-cloud-id}/…`
  — handing the operator's real Jira token to a
  tenant the attacker owned.
  **Fix:** `JiraConfig::validate` now rejects any
  `jira_url` whose host does not end in
  `.atlassian.net` (case-insensitively, with a
  non-empty subdomain label). New `DisallowedHost`
  variant on `ConfigError`, seven unit tests covering
  the positive and negative cases.

- **RT-094 — `jira_url` scheme not validated; allowed
  `http://`.** Basic-auth would have been sent in
  clear text on any plaintext URL.
  **Fix:** `InsecureScheme` variant; `validate_jira_url`
  rejects any scheme other than `https`.

- **RT-095 — Transport-error messages could leak URL
  userinfo.** `format!("HTTP transport error: {t}")`
  stringified `ureq::Transport`, which includes the
  target URL; a `jira_url` of the form
  `https://user:token@site.atlassian.net` would
  surface the credentials in
  `TaskPushResult.message`.
  **Fix:** new `transport_error_message` renders only
  `ErrorKind` (plus short message when present),
  never the URL.

- **RT-096 — Unbounded Jira response body embedded in
  per-task error messages.** A large response body
  (or a malicious proxy returning multi-MB payloads)
  would be placed verbatim into every failing
  `TaskPushResult.message`; with N tasks the
  aggregated `PluginResult` easily exceeded the
  host's 10 MiB response cap, silently aborting the
  whole batch with a host-side parse failure.
  **Fix:** new `truncate_body` caps response bodies
  at 4 KiB on a UTF-8 boundary and appends
  `"…[truncated]"` so the signal survives. Three
  unit tests including a multi-byte codepoint
  straddling the cap.

- **RT-097 — No HTTP timeouts configured.** Bare
  `ureq::get`/`post` helpers have no read timeout;
  a slow Jira would hang the plugin (and the host)
  indefinitely.
  **Fix:** `UreqClient` now holds a preconfigured
  `ureq::Agent` with 30-second connect/read/write
  timeouts. Constructed via `UreqClient::default()`.

- **RT-098 — `unsafe_code = "allow"` was set
  crate-wide.** Only the FFI exports in `lib.rs`
  actually need unsafe — `config.rs`, `jira_client.rs`,
  `mapping.rs` have no need.
  **Fix:** crate-level lint changed to `deny`;
  `lib.rs` opts in with `#![allow(unsafe_code)]`
  mirroring the `plugin_host.rs` precedent. Any new
  module accidentally reaching for unsafe will fail
  to compile.

- **RT-099 — `ureq` TLS backend not pinned.** The
  default ureq 2.x feature set pulled `native-tls`
  (and hence system OpenSSL on Linux), making
  portable-binary builds dependent on whatever TLS
  stack happened to be around.
  **Fix:** `Cargo.toml` now declares
  `ureq = { default-features = false, features = ["json", "tls"] }`
  so rustls + webpki-roots is the only option.

---

### Red-team backlog sweep (2026-04-19)

Resolved four findings in a single `fix:` commit
("fix: close red-team backlog items") and retired five
findings as stale / won't-fix. Brought the open log
from 16 down to 7 findings (below the 10+ threshold).

- **RT-068 — `cargo xtask check` reported "0
  compilation error(s)" for non-rustc failures.**
  Resolved. `xtask/src/main.rs::run_check` now falls
  back to printing the last 20 non-empty stderr lines
  when cargo exits non-zero but emits no lines
  matching `error[`/`error:`, turning the diagnostic
  black-hole into a useful stderr tail.
- **RT-069 — `Bash(git checkout:*)` overly broad in
  `/template-sync` allowed-tools.** Resolved. The
  permission is removed from
  `.claude/commands/template-sync.md`; the workflow
  never invoked `git checkout` anyway, so this closes
  a prompt-injection escape surface.
- **RT-072 — Windows-reserved TaskIds (`CON`,
  `NUL`, etc.).** Resolved. `TaskId::new` now
  rejects the 22 Windows device-name aliases
  (`CON`, `PRN`, `AUX`, `NUL`, `COM1`..`COM9`,
  `LPT1`..`LPT9`) case-insensitively via a
  `WINDOWS_RESERVED_IDS` allowlist. Unit test
  covers the reserved set plus prefix-collision
  negatives (`CONTACT`, `COM10`, `NULL` still
  pass).
- **RT-024 — Cyclic graph in hand-edited
  `project.json` panicked `critical_path`.**
  Resolved. `file_store::load` now calls
  `project.topological_sort()` and rejects the load
  with a new `StoreError::InvalidProject` variant if
  the returned order is shorter than the task count
  (Kahn's algorithm drops cycle participants
  silently; unequal lengths mean ≥ 1 cycle). Unit
  test hand-crafts a two-task cycle and asserts the
  rejection message.

Retired as stale / won't-fix:

- **RT-082 — `rustwerk` 0.40.0 bumped without plugin
  loader consumer.** PLG-HOST (v0.43.0) now supplies
  the loader. The chain PLG-API 0.40 → PLG-WORKSPACE
  0.41 → CLI-JSON 0.42 → PLG-HOST 0.43 is
  self-documenting via DIARY entries.
- **RT-040 — Cyclic deps vanish from `--chain`
  output.** Defense-in-depth for a state already
  prevented by runtime `add_dependency` validation
  AND (as of this sweep) by `load` validation. Not
  reachable.
- **RT-038 — Dangling dependency refs truncate
  `--chain`.** Mitigated by DEP-GUARD (prevents
  task removal while dependents exist). Not
  reachable through normal operation.
- **RT-014 — Batch `--file` path traversal.**
  Accepted as inherent to the local-CLI trust model
  per the original finding's own rationale.
  Reopen if rustwerk is ever wired into a
  non-interactive orchestration path.
- **RT-013 — Batch rollback is implicit.**
  Forward-looking design concern, not a current
  bug. Revisit only if the batch path grows
  checkpointing or partial-save behaviour.

### Plugin host trust-model hardening (bundle)

- **Date:** 2026-04-19
- **Category:** Trust model / code execution
- **Commit context:** feat: add dynamic plugin host
  (v0.43.0)
- **Resolution:** Three findings from the PLG-HOST
  red-team review were addressed in the same commit:
  - **Default `target/debug` + `target/release`
    discovery dropped.** Original design auto-loaded
    any cdylib cargo dropped into `target/*` —
    build-script artifacts, dep cdylibs, proc-macros
    on some platforms — *before* the API-version
    check runs (dynamic loading executes
    initializers first). Now gated behind
    `RUSTWERK_PLUGIN_DEV=1` env var. End-user
    installs only scan
    `<project>/.rustwerk/plugins/` and
    `~/.rustwerk/plugins/`.
  - **Empty `HOME`/`USERPROFILE` no longer causes
    CWD-relative scan.** `env::var_os` only filters
    *unset* values; an empty string yielded
    `PathBuf::from("")` which joined to the relative
    `.rustwerk/plugins` path, scanned against the
    process CWD. An attacker in a shared tmp dir
    could drop plugins and get code exec on `HOME=
    rustwerk …`. `home_dir()` now treats empty as
    absent.
  - **Shadowed plugins logged instead of silently
    skipped.** When a less-trusted directory has a
    plugin of the same name as an already-loaded
    one, `discover_plugins` now writes a stderr
    warning naming both paths so users can spot
    shadow attacks and name collisions.

### RT-091 — `task describe --json` leaked absolute filesystem path

- **Date:** 2026-04-19
- **Category:** Information leak
- **Commit context:** feat: add `--json` global output
  flag (v0.42.0)
- **Description:** Initial CLI-JSON implementation
  emitted `path: path.display().to_string()` using the
  absolute path. Text mode only showed the path on the
  not-found branch; JSON always showed it, leaking the
  developer's home directory in every successful call.
- **Resolution:** In the same commit, `cmd_task_describe`
  now calls `abs_path.strip_prefix(&root)` and emits a
  project-relative path. Integration test
  `json_task_describe_reports_missing_content` now
  asserts the path is not absolute.

### RT-090 — `task describe` had no size cap on file read

- **Date:** 2026-04-19
- **Category:** Denial of service
- **Commit context:** feat: add `--json` global output
  flag (v0.42.0)
- **Description:** `std::fs::read_to_string` on the
  description file had no limit — a symlink to
  `/dev/zero` or a multi-GB file would exhaust memory.
  Pretty-printed JSON escape expansion amplified the
  worst case (e.g. NULs become `\u0000`, 6× expansion).
- **Resolution:** `cmd_task_describe` now stats the
  file, refuses anything over `MAX_DESCRIBE_BYTES`
  (1 MiB), and wraps the read in `File::take` so the
  cap is enforced even if the metadata races. UTF-8
  errors are surfaced with a clear "description file
  is not valid UTF-8: <path>" message.

### RT-089 — Float NaN/Infinity could abort JSON output post-save

- **Date:** 2026-04-19
- **Category:** Correctness
- **Commit context:** feat: add `--json` global output
  flag (v0.42.0)
- **Description:** `serde_json::to_writer_pretty`
  refuses non-finite `f64`. Commands like `effort log
  --json` mutate state, save to disk, then serialize —
  a `NaN`/`Inf` would exit non-zero after the save had
  already landed.
- **Resolution:** `render::finite(f64) -> Option<f64>`
  helper, applied to every float field in the DTOs
  (`pct_complete`, hour totals, percentages). Non-finite
  values serialize as `null` instead of aborting.

### RT-084..088 — Installer script hardening (bundle)

- **Date:** 2026-04-19
- **Category:** Correctness + Security
- **Commit context:** chore: add cross-platform install
  scripts
- **Resolution:** Five findings from the red-team review
  of the new installer scripts were fixed in the same
  commit:
  - **RT-084** — Windows PowerShell 5.1 does not default
    to TLS 1.2, which GitHub requires. `install.ps1`
    now sets
    `[Net.ServicePointManager]::SecurityProtocol` to
    `Tls12` before any web request. Harmless on pwsh 7.
  - **RT-085** — A 32-bit PowerShell host on 64-bit
    Windows reports `PROCESSOR_ARCHITECTURE=x86` and
    would have rejected a capable AMD64 machine.
    `install.ps1` now consults `PROCESSOR_ARCHITEW6432`
    first.
  - **RT-086** — Unauthenticated GitHub API is
    rate-limited to 60 req/hr/IP; shared NAT/CI users
    would see a cryptic "could not resolve latest
    version." Both scripts now fall back to following
    the `releases/latest` HTML redirect and extracting
    the tag from the `Location:` header when the API
    call fails. `RUSTWERK_VERSION` remains the explicit
    escape hatch.
  - **RT-087** — The `wget` fallback lacked HTTPS
    enforcement and retry/backoff. `install.sh` now
    invokes `wget --https-only --tries=3` and
    `curl --proto '=https' --tlsv1.2 --retry 3`
    uniformly via `dl_to` helper functions.
  - **RT-088** — `RUSTWERK_INSTALL_DIR` was written
    verbatim into the user's persistent PATH on
    Windows; a value containing `;` would corrupt it
    permanently. `install.ps1` now rejects such values.
    Additionally both scripts `rm` the destination
    binary before the final copy/move so that a
    symlink in the install dir is replaced, not
    followed.

---

### RT-073..081 — `rustwerk-plugin-api` FFI contract hardening (bundle)

- **Date:** 2026-04-19
- **Category:** Correctness + Security + Project Config
- **Commit context:** feat: add `rustwerk-plugin-api` crate
  (v0.40.0)
- **Resolution:** Nine findings surfaced in the initial
  red-team review of the new plugin API crate were
  addressed in the same commit:
  - **RT-073** — FFI out-pointer ownership on error was
    under-specified. Crate-level docs now mandate that
    the host initializes `*out = null` before each call,
    the plugin must leave `*out` null or pointing at a
    plugin-allocated string regardless of return code,
    and the host must always call
    `rustwerk_plugin_free_string(*out)` (null-safe) even
    on error. This removes the leak / UB ambiguity on
    error paths.
  - **RT-074** — "non-zero on error" had no enumerated
    meaning. Added `ERR_OK`, `ERR_GENERIC`,
    `ERR_INVALID_INPUT`, `ERR_VERSION_MISMATCH` constants;
    doc specifies hosts must treat unknown non-zero codes
    as `ERR_GENERIC`.
  - **RT-075** — `rustwerk_plugin_api_version` call order
    was not mandated. Docs now require it to be the first
    export invoked, and require the host to unload without
    calling any other export on mismatch.
  - **RT-076** — `TaskDto.status` was a free-form string.
    Replaced with `TaskStatusDto` enum (`#[serde(rename_all
    = "snake_case")]`) mirroring all five host `Status`
    variants (`todo`, `in_progress`, `blocked`, `done`,
    `on_hold`) with a round-trip test covering every
    variant.
  - **RT-077** — Capability matching case-sensitivity
    ambiguity. Docs now specify lowercase-ASCII matching
    and that plugins should emit only lowercase identifiers
    to avoid silent mismatches.
  - **RT-078** — `deserialize_from_cstr` was size-unbounded.
    Added `deserialize_from_cstr_bounded(s, max_bytes)`
    that rejects inputs exceeding the caller-supplied cap
    before parsing; original helper retained with a doc
    note pointing to the bounded variant for less-trusted
    boundaries.
  - **RT-079** — Plugin-controlled strings could contain
    terminal-escape sequences. Docs now require the host
    to sanitize plugin-returned strings before writing to
    a terminal.
  - **RT-080** — Crate was publishable by default despite
    API_VERSION=1 being brand-new and subject to churn.
    Added `publish = false` to `Cargo.toml`.
  - **RT-081** — `serde_json = "1"` pinned no minimum
    patch. Pinned to `serde_json = "1.0.140"` so plugins
    built against this crate have a known-good floor.

---

### RT-071(rename) — `task rename` JSON-vs-filesystem divergence

- **Date:** 2026-04-19
- **Category:** Correctness (Medium)
- **Commit context:** feat: `task rename` command (v0.39.0)
- **Resolution:** The CLI `cmd_task_rename` now preflights
  the destination description-file path and bails with a
  non-zero exit code if it already exists, preventing
  overwrite. The batch driver collects filesystem side
  effects as a typed `FileSideEffect` enum during
  `execute_one` (instead of re-parsing the command JSON in
  a separate post-save loop with silent `let...else`
  fallbacks), then replays them in command order after
  `save_project` and reports any fs failures in a JSON
  error envelope on stderr while exiting non-zero. A new
  `file_store::rename_task_description` helper refuses to
  overwrite an existing destination and returns a typed
  `DescriptionFileError`. `file_store::remove_task_description`
  is used by `task remove` (CLI and batch) so the `.md`
  cleanup path is consistent across lifecycle operations.
  Defensive dedup + self-ref stripping added to
  `Project::rename_task` to preserve the no-duplicate /
  no-self-cycle invariants even in the face of unexpected
  state.

### RT-067 — `extract_check_errors` drops user errors that mention "aborting"

- **Date:** 2026-04-19
- **Category:** Correctness (Low)
- **Commit context:** chore: adopt rustbase template (add
  `xtask check`)
- **Resolution:** Changed the filter from
  `.contains("aborting")` to
  `.starts_with("error: aborting due to")`, which matches
  only the exact rustc summary terminator. Added a
  regression test (`keeps_user_errors_that_mention_aborting`)
  that asserts a user-authored
  `error: aborting build: feature flag missing` survives
  filtering while the rustc summary line is still dropped.
  Also logged as upstream feedback.

### RT-065 — TOCTOU race in `cmd_task_describe`

- **Date:** 2026-04-04
- **Category:** Correctness (Medium)
- **Commit context:** v0.37.0 `task describe` command
- **Resolution:** Replaced `path.exists()` + `read_to_string`
  with a single `read_to_string` call, matching on
  `ErrorKind::NotFound` for the "no file" branch.

### RT-066 — `task_description_path` accepts raw `&str`

- **Date:** 2026-04-04
- **Category:** Security / latent path traversal (Medium)
- **Commit context:** v0.37.0 `task describe` command
- **Resolution:** Changed signature from `&str` to `&TaskId`,
  ensuring only validated task IDs can reach path construction.

### RT-064 — CLI `task update` with no fields is a silent no-op

- **Date:** 2026-04-04
- **Category:** Correctness (Low)
- **Commit context:** v0.35.0 `--tags` flag
- **Resolution:** Added early guard in `cmd_task_update`
  that bails if none of `--title`, `--desc`, or `--tags`
  are provided, matching the batch path's validation.

### RT-063 — Batch `tags` silently drops non-string values

- **Date:** 2026-04-04
- **Category:** Correctness
- **Commit context:** v0.35.0 `--tags` flag
- **Resolution:** Added `parse_batch_tags` helper that
  uses `.map(|v| v.as_str().context(...))` instead of
  `filter_map`, so non-string values produce an error.

### RT-062 — Unbounded tag count (DoS via memory/CPU)

- **Date:** 2026-04-04
- **Category:** Security (Low)
- **Commit context:** v0.34.0 tags field
- **Resolution:** Added `Task::MAX_TAGS = 20` limit.
  `add_tag` returns error when limit is reached.

### RT-061 — Deserialized tags bypass all validation

- **Date:** 2026-04-04
- **Category:** Correctness
- **Commit context:** v0.34.0 tags field
- **Resolution:** Introduced `Tag` newtype with custom
  `Deserialize` impl that validates on load. Invalid
  tags in JSON are rejected at parse time.

### RT-060 — No validation of tag content

- **Date:** 2026-04-04
- **Category:** Correctness
- **Commit context:** v0.34.0 tags field
- **Resolution:** `Tag::new` validates slug format:
  lowercase alphanumeric + hyphens, max 50 chars.

### RT-059 — Batch `task.assign` missing `RUSTWERK_USER` fallback

- **Date:** 2026-04-04
- **Category:** Correctness
- **Commit context:** v0.32.0 `RUSTWERK_USER` env var
- **Resolution:** By design. Batch commands are
  deterministic — all arguments must be explicit in the
  JSON input. Added code comment and manual documentation
  stating this is intentional.

### RT-058 — No checksums for release artifacts

- **Date:** 2026-04-04
- **Category:** Security
- **Commit context:** project config hardening
- **Resolution:** Added `sha256sum` step to release workflow
  that generates `SHA256SUMS` file included in release.

### RT-057 — `.gitignore` missing `*.pdb`

- **Date:** 2026-04-04
- **Category:** Security
- **Commit context:** project config hardening
- **Resolution:** Added `*.pdb` to `.gitignore`.

### RT-056 — `chrono` default features not disabled

- **Date:** 2026-04-04
- **Category:** Security
- **Commit context:** project config hardening
- **Resolution:** Set `default-features = false` with
  explicit `clock`, `serde`, `std` features.

### RT-055 — Missing `unsafe_code = "forbid"`

- **Date:** 2026-04-04
- **Category:** Security
- **Commit context:** project config hardening
- **Resolution:** Added `unsafe_code = "forbid"` to
  workspace lints.

### RT-054 — Workspace lints not shared; crates diverge

- **Date:** 2026-04-04
- **Category:** Correctness
- **Commit context:** project config hardening
- **Resolution:** Moved lints to `[workspace.lints]` in
  root `Cargo.toml`, both crates use `workspace = true`.

### RT-053 — No clippy pedantic lint group enabled

- **Date:** 2026-04-04
- **Category:** Correctness
- **Commit context:** project config hardening
- **Resolution:** Added `pedantic = { level = "warn" }` to
  workspace clippy lints. Fixed all pedantic warnings across
  the codebase.

### RT-052 — Actions not pinned to SHA

- **Date:** 2026-04-04
- **Category:** Security
- **Commit context:** CI/release workflow setup
- **Resolution:** Pinned all `actions/checkout`,
  `actions/cache`, `actions/upload-artifact`, and
  `actions/download-artifact` to full commit SHAs with
  version comments.

### RT-051 — Non-semver tags trigger release workflow

- **Date:** 2026-04-04
- **Category:** Correctness
- **Commit context:** CI/release workflow setup
- **Resolution:** Added semver validation step early in
  the build job that rejects malformed tags.

### RT-050 — Cache includes target/ allowing poisoning

- **Date:** 2026-04-04
- **Category:** Security
- **Commit context:** CI/release workflow setup
- **Resolution:** Removed `target` from cache paths in
  both `ci.yml` and `release.yml`. Only `~/.cargo/registry`
  and `~/.cargo/git` are cached now.

### RT-049 — Awk regex injection via crafted tag name

- **Date:** 2026-04-04
- **Category:** Security
- **Commit context:** CI/release workflow setup
- **Resolution:** Changed awk from regex match (`~`) to
  string match (`index()`) for version comparison.

### RT-048 — Missing permissions block in CI workflow

- **Date:** 2026-04-04
- **Category:** Correctness
- **Commit context:** CI/release workflow setup
- **Resolution:** Added `permissions: contents: read` at
  the top of `ci.yml`.

### RT-046 — project.tasks[*id] panics on absent key

- **Date:** 2026-04-03
- **Category:** Correctness
- **Commit context:** v0.31.0 status command
- **Resolution:** Changed to `.get(*id).and_then()` with
  fallback for missing keys.

### RT-045 — usize underflow if filled > bar_width

- **Date:** 2026-04-03
- **Category:** Correctness
- **Commit context:** v0.31.0 status command
- **Description:** Floating-point rounding could make
  `filled` exceed `bar_width`, causing usize underflow
  in `bar_width - filled`.
- **Resolution:** Added `.min(bar_width)` clamp.

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

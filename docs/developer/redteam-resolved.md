# Red Team Findings — Resolved

Archive of fixed red team findings, newest first.
See [redteam-log.md](redteam-log.md) for open findings.

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

# Development Diary

This diary tracks functional changes to the RustWerk codebase in
reverse chronological order.

---

### 2026-04-20

- Richer Jira field mapping — status transitions, assignee, priority, labels (v0.52.0)

    PLG-JIRA-FIELDS. `JiraConfig` gains four optional
    mapping fields: `status_map` (rustwerk status wire
    name → Jira workflow **transition ID**, discovered
    via `GET /issue/{key}/transitions`), `assignee_map`
    (email → `accountId`; keys validated to contain
    `@` at load time), `priority_map` (stringified
    complexity score → Jira priority name), and
    `labels_from_tags` (boolean, default `false`).

    Status changes go through the separate Jira
    transitions endpoint (`POST /issue/{key}/transitions`)
    because Jira rejects `status` in the create/update
    body. A new `jira_client::transition` verb carries
    the same direct/gateway fallback as the other verbs.
    The plugin records `plugin_state.jira.last_status`
    after a successful transition so repeated pushes
    with unchanged status skip the workflow-event
    round-trip. A failed transition leaves `last_status`
    absent so the next push retries automatically.

    Labels reject whitespace/control chars — tags that
    would otherwise 400 the whole push are dropped with
    a typed `MappingWarning::RejectedLabel` rather than
    aborting the task and losing the idempotency anchor.

    Introduced a typed `MappingWarning` enum for
    mapping and transition warnings, replacing
    stringly-typed `Vec<String>`. `TaskStatusDto` gains
    `as_wire()` on the DTO crate so the snake_case wire
    names live alongside the serde attrs (single source
    of truth). `TaskPushResult` gains a
    `with_appended_message` builder to keep decoration
    consistent with the existing builder style.

    Plugin crate split into focused modules: `push`
    (orchestration), `transition` (workflow + state),
    `warnings` (typed `MappingWarning`), with
    `mapping`/`config`/`jira_client` untouched in
    scope. `lib.rs` is now FFI-only.

- Per-task Jira issue type — `--type epic|story|task|sub-task` (v0.51.0)

    PLG-JIRA-ISSUETYPE. The jira plugin's hardcoded
    `"issuetype": {"name": "Task"}` is gone. The domain
    gains an `IssueType` enum (`Epic` / `Story` /
    `Task` / `SubTask`, serialized kebab-case), stored
    as `Task.issue_type: Option<IssueType>` and
    surfaced on the CLI via `--type` on `task add` /
    `task update` (and the batch API's `type` arg). An
    empty `""` on update clears the type. `task list`
    renders a single-letter prefix (`E:` / `S:` / `T:`
    / `s:`) so the WBS view shows type at a glance.

    The plugin API bumps to **v3** — `TaskDto` gains
    a stringly-typed `issue_type: Option<String>`
    carrying the kebab-case wire name. Host rejects
    v2 plugins with a rebuild-required message at
    load. The jira plugin's `JiraConfig` grows two
    optional fields: `default_issue_type` (Jira-side
    name used when the task has no explicit type;
    defaults to `"Task"`) and `issue_type_map` (wire
    name → exact Jira string, for sites that renamed
    `"Sub-task"` to `"Subtask"` or localized
    names). `build_issue_payload` now takes
    `&JiraConfig` and resolves per task via
    `JiraConfig::resolve_issue_type_name` — task
    override → config default → `"Task"`, with map
    overrides applied on the first leg.

    Parent/epic linking (the hierarchy that the issue
    type often implies) stays out of scope — it lands
    in PLG-JIRA-PARENT.

    Same-commit review sweep (nine findings, all fixed):

    - **RT-129:** unknown kebab values (future domain
      variants, corrupted `project.json`) used to be
      forwarded to Jira verbatim, bypassing the
      `default_issue_type` safety net and producing
      HTTP 400. The plugin's `resolve_issue_type_name`
      now falls through to the configured default
      whenever the incoming kebab is not recognized by
      the map or the built-in table.
    - **RT-130:** `IssueType::parse` accepted `subtask`
      as an alias for `sub-task`, but the plugin did
      exact-match map lookups. Added
      `canonicalize_issue_type_kebab`, applied at
      config load-time (so user-entered map keys
      normalize) and at resolve-time.
    - **RT-131 / AQ-108:** the CLI and batch paths
      mutated `task.issue_type` through
      `project.tasks.get_mut(...)` directly, skipping
      the `modified_at` bump every other update went
      through. Added
      `Project::set_task_issue_type(&mut self, id,
      Option<IssueType>)` to the domain layer; both
      `cmd_task_update` and the batch dispatcher now
      go through it.
    - **RT-132:** plugin-side validation of the wire
      `issue_type` — added
      `is_plausible_issue_type_wire` (≤64 bytes, no
      control chars, non-empty); anything else falls
      back to the default.
    - **AQ-109:** the `Option<String>` wire type vs
      closed `TaskStatusDto` enum was deliberate
      (forward-compat for new domain variants) but
      undocumented. Expanded the doc comment on
      `TaskDto.issue_type` to spell out the intent.
    - **AQ-110:** `cmd_task_update` had grown to five
      positional args, four `Option<&str>`; swapping
      two would not be caught by the compiler.
      Introduced `TaskUpdateFields<'a>` struct;
      future fields become a one-field extension.
    - **AQ-111:** the batch API key `"type"` was the
      only surface disagreeing with `project.json`'s
      serialized `"issue_type"`. Handlers now accept
      `"issue_type"` as canonical and `"type"` as a
      documented alias.
    - **AQ-112:** `IssueType::list_marker` was tested
      for variant-uniqueness but never used — the
      renderer hand-rolled a parallel match.
      Deleted the method and its test.

- Idempotent `plugin push jira` — probe → update or recreate (v0.50.0)

    PLG-JIRA-UPDATE. The second (and Nth) push of a
    task no longer creates duplicate Jira issues.
    `HttpClient` gained `put_json`; `jira_client`
    gained `get_issue` (probe) and `update_issue`
    (PUT), all three verbs sharing a
    `resolve_cloud_id` helper so the Platform API
    Gateway fallback behavior is identical across
    create / probe / update. `push_one` now
    dispatches on the task's existing
    `plugin_state.jira.key`: absent → create (the
    existing path); present → probe `GET
    /issue/{key}`, then update `PUT` (refreshing
    `last_pushed_at`, preserving `key`/`self`) or
    recreate via POST if the issue was deleted
    Jira-side.

    Same-commit review sweep hardened the update
    path against a handful of duplicate-creation and
    data-loss failure modes found by the red team:

    - **RT-121:** stored Jira keys could previously
      contain path-traversal segments
      (`../../admin`) and be spliced straight into
      URL builders. The fix introduced a
      `pub(crate) struct IssueKey(String)` newtype
      validated against `[A-Z][A-Z0-9_]*-[0-9]+`
      with a 64-char length cap; every URL builder
      and verb signature now takes `&IssueKey`. A
      malformed stored value is surfaced as
      `ExistingKey::Invalid(raw)` and fails the task
      loudly rather than silently recreating.
    - **RT-122:** direct-URL 401 plus gateway 404 is
      not proof of absence — the direct read was
      blocked, so the gateway's "missing" answer
      could just mean scoped-token restriction.
      Before the fix this triggered a silent
      recreate and produced a second Jira issue
      while the original stayed alive. `get_issue`
      now returns a `ProbeOutcome` enum with
      explicit `Exists` / `MissingConfirmed`
      (direct 404 + gateway 404) /
      `MissingAmbiguous` (direct 401 + gateway 404)
      / `OtherStatus` variants; only
      `MissingConfirmed` triggers recreate,
      `MissingAmbiguous` fails the task with a
      message telling the operator to verify token
      scope.
    - **RT-123:** the refresh path silently dropped
      any additive state fields
      (`last_hash`, `last_response_etag`, …) that a
      future plugin version might add, because it
      rebuilt the blob from scratch. Now refresh
      clones the existing `Object` and mutates only
      `last_pushed_at` in place, preserving every
      other field verbatim — matching the
      "additive fields" contract in
      `build_created_state`'s docstring.
    - **RT-125 (fold-in):** the update-path task
      message used to omit `via gateway` when only
      the probe (not the PUT) fell back. Now
      `push_one_update` threads the probe's
      `used_gateway` into
      `task_result_from_update_outcome`, which ORs
      it with the PUT's flag.
    - **AQ-105:** the `IssueKey` newtype is the
      type-safe encoding of RT-121's defense —
      C-NEWTYPE from the Rust API Guidelines. A
      malformed value cannot reach URL construction
      because it cannot even be constructed.

    Four red-team findings (RT-124 probe-body
    validation, RT-126 TOCTOU regression test,
    RT-127 `jira_url` scheme check, RT-128 flaky
    env-race in `plugin_host` tests) and six
    Artisan findings (AQ-101..104, AQ-106, AQ-107)
    are logged open as deferred polish — module
    size at the top of the list.

    `docs/manual.md` and `llms.txt` document the
    idempotency contract so the end-user view
    matches the new behavior.

- Jira plugin records created-issue state on first push (v0.49.0)

    PLG-JIRA-STATE. The jira plugin now parses the
    Jira create-issue response into a typed
    `CreatedIssue { key, self_url }` and, on success,
    attaches a `plugin_state_update` blob
    (`{ "key", "self", "last_pushed_at" }`) to the
    per-task result. The host persists this under
    `plugin_state.jira` via PLG-API-STATE's round-trip,
    giving PLG-JIRA-UPDATE the idempotency anchor it
    needs. `last_pushed_at` is ISO-8601 UTC with seconds
    precision (e.g. `2026-04-22T09:14:07Z`).

    Same-commit review sweep (RT-118/119/120,
    AQ-095..100) tightened the parse path so a
    misbehaving Jira cannot cause silent duplication:
    `parse_created_issue` now returns
    `Result<CreatedIssue, ParseIssueError>` with
    explicit `EmptyBody` / `Malformed` / `EmptyField` /
    `InvalidSelfUrl` variants. Only `EmptyBody` (204)
    stays a silent skip; every other failure appends a
    visible `WARNING: …; plugin state not recorded`
    to the task message so operators catch schema
    drift before duplicate issues pile up. `self` URLs
    are validated through `url::Url` and rejected
    unless the scheme is `http`/`https`, closing a
    `javascript:` / `file:` injection vector into
    persisted state.

    Alongside the feature: `HttpClient` now returns
    `Result<_, HttpError>` (a `thiserror` enum with
    `Transport`, `TenantInfo`, `TenantInfoDecode`
    variants) instead of `String`, killing the
    double-prefix "HTTP error: HTTP transport error:
    …" the old call site used to emit. `Clock`
    returns `DateTime<Utc>` rather than a preformatted
    `String`, moving the format choice into
    `build_jira_state` where the wire shape lives.
    And the recording `MockHttp`/`FakeHttp` fakes from
    the two test modules were merged into a shared
    `src/test_support.rs` so adding `put_json` for
    PLG-JIRA-UPDATE will only touch one fake.

    `chrono` (no-default-features, `clock` + `std`)
    joins `rustwerk-jira-plugin` dependencies.

- Per-task plugin-state round-trip in the plugin API (v0.48.0)

    PLG-API-STATE. Bumps `rustwerk_plugin_api`'s
    `API_VERSION` from 1 to 2 and introduces an
    opaque, plugin-namespaced per-task state bag that
    plugins can read and write across pushes — the
    foundation for PLG-JIRA-STATE and PLG-JIRA-UPDATE
    to make `rustwerk plugin push` idempotent.

    On the API side: `TaskDto.plugin_state: Option<Value>`
    carries the prior state the plugin recorded for
    this task, and `TaskPushResult.plugin_state_update:
    Option<Value>` carries the state the plugin wants
    persisted back. Added
    `TaskPushResult::ok(id, msg)` / `::fail(id, err)`
    constructors plus `with_external_key` /
    `with_plugin_state_update` setters so future
    optional fields don't force mass edits at every
    call site. The "no clear-variant" contract is
    deliberate — `None` = unchanged, `Some(v)` =
    replace; `Some(Null)` is treated as a no-op by
    the host so the docstring matches behavior.

    On the domain side: `Task` gains
    `plugin_state: BTreeMap<String, Value>` keyed by
    plugin name. Rename / delete carry the state
    along automatically via the existing struct
    move/consume semantics. The field is omitted
    from `project.json` when empty.

    On the host side: `task_to_dto` is now pure
    mapping (returns `plugin_state: None`); a new
    `task_to_dto_for_plugin` slices the per-plugin
    namespace out of `task.plugin_state` so a plugin
    only sees its own entry. New helpers
    `apply_state_updates` (merge) and
    `persist_plugin_state` (merge + atomic save)
    enforce namespacing on the write side. The
    plugin-host version-mismatch error now tells
    authors to rebuild against the current
    `rustwerk-plugin-api` crate.

    Red team / Artisan sweep (RT-110..RT-114 +
    AQ-089..AQ-092, all fixed in-commit): cap
    `plugin_state_update` at 64 KiB per task to
    prevent project.json bloat; filter plugin
    responses by the set of task IDs actually pushed
    so a plugin can't stamp state onto excluded
    tasks; log every skipped update to stderr so
    drops are diagnosable instead of silent; treat
    `Some(Value::Null)` as a no-op per the "no
    clear-variant" contract; surface save failures
    as a `save_warning` on `PluginPushOutput` that
    prints after the successful plugin result (so
    the user sees any external keys) and flips the
    process exit non-zero; add builder methods for
    `TaskPushResult`; split `task_to_dto` into a
    base function and a per-plugin slicing variant;
    extract `persist_plugin_state` with a
    tempdir-backed integration test for
    save-on-partial-failure.

    Five findings from the sweep logged open
    (RT-115 plugin-name case sensitivity, RT-116
    concurrent push races, RT-117 v1-plugin compat
    shim, AQ-093 `Value` newtype, AQ-094
    CHANGELOG.md migration).

- Add `rustwerk plugin install` subcommand (v0.47.0)

    PLG-INSTALL. The new command
    `rustwerk plugin install <SOURCE> [--scope
    project|user] [--force]` copies a pre-built cdylib
    into the plugin discovery directory and
    load-verifies it via the same host entry point
    `plugin list` uses. Failed verification rolls back
    the copy so a botched install never leaves a
    half-installed plugin behind. Wrong-extension
    sources are rejected before any filesystem
    mutation.

    Delivered source kind: path-to-already-built
    cdylib only. The cargo-build variants
    (`--from <package-name>`, cargo-project-dir
    source) are deferred to a follow-up task
    `PLG-INSTALL-BUILD` — the c=2 rating for
    PLG-INSTALL couldn't absorb a cargo-subprocess
    wrapper.

    Red team / Artisan sweep (RT-106..RT-107,
    AQ-082..AQ-086, all fixed in-commit): reject
    symlink destinations so `fs::copy` can't write
    plugin bytes outside `plugins/`; reject
    source == dest canonicalisation so
    re-installing from the discovery dir can't
    truncate and delete the already-installed plugin;
    require `load_project` to succeed for
    `--scope project` instead of silently falling
    back to cwd; type `PluginInstallOutput.scope` as
    `InstallScope` with `serde(rename_all = "snake_case")`
    instead of `String`; switch `verify` from
    `&dyn Fn` to `impl Fn`; replace the
    `(PluginListItem, PathBuf, bool)` tuple return
    with a named `InstallOutcome` struct; drop
    `PluginInstallOutput.destination` as redundant
    with `installed.path`.

    Four findings from the same sweep logged open
    for separate refactors: AQ-087 (split
    `commands/plugin.rs` at 1200+ lines into
    per-subcommand modules), AQ-088 (narrow
    `plugin_host` visibility leaks), RT-108 (reject
    Windows-reserved cdylib filenames), RT-109
    (migrate install to temp-file + atomic rename).

- Render Jira description as ADF (v0.46.0)

    PLG-MAP. The jira plugin's `description` field now
    ships as an Atlassian Document Format (ADF) `doc`
    node — one `paragraph` per line, empty paragraphs
    for blank lines, fallback to the task title when
    the description is empty. Replaces the
    plain-string placeholder that PLG-JIRA shipped and
    unblocks rich-text rendering in Jira Cloud (the
    REST API v3 rejects plain strings in
    description).

    Red team/Artisan sweep (RT-104..RT-105,
    AQ-079..AQ-081, all fixed in-commit): normalize
    `\r\n` and bare `\r` to `\n` so Windows-authored
    descriptions don't smuggle trailing carriage
    returns into ADF text nodes; strip ASCII control
    characters other than `\t` so stray form-feeds or
    ANSI escapes from pasted terminal output don't
    trigger opaque HTTP 400 responses from the ADF
    validator; extract `description_text` so the
    empty-description-falls-back-to-title policy has
    one named home; drop the redundant empty-string
    branch in `adf_doc`; drop inconsistent trailing
    commas inside `json!` literals.

### 2026-04-19

- Add plugin CLI subcommands (v0.45.0)

    PLG-CLI. `rustwerk plugin list` and
    `rustwerk plugin push <NAME>
    [--project-key --tasks --dry-run]` wire the
    PLG-HOST loader into the user-facing CLI. The
    plugin architecture shipped three commits ago is
    now actually reachable from the command line —
    drop a cdylib into `.rustwerk/plugins/` and the
    two commands discover, introspect, and invoke it.

    Config assembly is host-generic: `JIRA_URL` /
    `JIRA_TOKEN` env, `git config user.email` (via a
    new tiny `git.rs` module), and `--project-key`
    get rolled into a JSON object. Absent keys are
    omitted entirely so plugins can distinguish
    "not configured" from empty. `--dry-run` prints
    *only* resolved key names — never values — so
    piping into logs is safe.

    `plugin_host.rs` dropped its
    `#![allow(dead_code)]` guard and gained
    `validate_plugin_name`: plugin-reported names are
    now confined to `[A-Za-z0-9_-]+` and 64 chars,
    preventing a hostile cdylib from smuggling ANSI
    escapes or newlines into host output.

    Four red-team findings (RT-100..RT-103) and four
    Artisan findings (AQ-075..AQ-078) all fixed
    in-commit — see the respective resolved logs.
    Notable: `filter_tasks` was quadratic (walked
    every project key via `.keys().find` after a
    successful lookup); swapped to
    `HashMap::get_key_value`. Exit-code handling
    moved out of the command module so per-task
    failure detail now renders even when the plugin
    reports an aggregate failure.

- Add Jira plugin (v0.44.0)

    PLG-JIRA. The `rustwerk-jira-plugin` cdylib now
    exports the four FFI entry points required by
    `rustwerk-plugin-api` (`api_version`, `info`,
    `push_tasks`, `free_string`) and can actually push
    rustwerk tasks as Jira Cloud issues over HTTPS via
    Basic auth. Mapping to issue JSON is deliberately
    minimal here (`project` + `summary` +
    plain-text `description` + `issuetype: Task`);
    PLG-MAP upgrades the description to Atlassian
    Document Format and adds richer fields.

    The crate layout is split for testability:
    `config.rs` owns `JiraConfig` parsing + validation,
    `jira_client.rs` owns the HTTP client behind an
    `HttpClient` trait, `mapping.rs` builds the issue
    payload, and `lib.rs` is the FFI façade. All unit
    tests exercise the trait with a recording
    `MockHttp` / `FakeHttp` fake — no network traffic
    in `cargo xtask validate`. 57 new tests.

    Gateway fallback mirrors the marketplace-v2 Python
    behaviour: if the direct
    `{jira_url}/rest/api/3/issue` call returns 401 or
    404, the plugin fetches `{jira_url}/_edge/tenant_info`
    to discover `cloudId`, then retries via
    `https://api.atlassian.com/ex/jira/{cloudId}/rest/api/3/issue`.
    Any other status short-circuits without fallback.

    Red-team review on the hardening surface raised
    seven findings, all fixed in-commit — see
    `redteam-resolved.md` for RT-093..RT-099 covering
    host allowlist, https-only scheme, transport-error
    sanitisation, response-body truncation, HTTP
    timeouts, narrowing `unsafe_code` to `lib.rs`
    only, and pinning rustls as the sole TLS backend.

    Not yet user-facing: the plugin builds, the host's
    `plugin_host.rs` can discover and load it, but the
    `rustwerk plugin list` / `plugin push` commands
    are still PLG-CLI. Artisan: no findings.

- Close red-team backlog items (v0.43.1)

    Backlog sweep. The open red-team log had grown to
    16 entries — past the 10+ threshold that requests a
    full-codebase review before continuing feature
    work. Triaged the backlog, fixed four items
    in-commit, retired five as stale/won't-fix, and
    slimmed one. Open count: 16 → 7.

    Fixes: **RT-068** (`xtask check` now prints the
    last 20 stderr lines when cargo exits non-zero
    without matching "error[" / "error:" markers, so
    non-compile failures — manifest parse, lockfile
    corruption, registry network — are no longer a
    diagnostic black hole); **RT-069** (removed the
    unused `Bash(git checkout:*)` permission from
    `/template-sync` allowed-tools, closing a
    prompt-injection escape surface); **RT-072**
    (`TaskId::new` now rejects the 22 Windows
    device-name aliases — `CON`, `NUL`,
    `COM1`..`COM9`, `LPT1`..`LPT9`, etc. —
    case-insensitively, so a project created on Linux
    can't silently break on a Windows checkout);
    **RT-024** (`file_store::load` now runs
    `topological_sort` and rejects any
    `project.json` whose loaded dependency graph
    contains a cycle via a new
    `StoreError::InvalidProject` variant, turning a
    downstream panic in `critical_path` into a clean
    load error).

    Retired: RT-082 (PLG-HOST consumed the bumped
    crate, making the "bump without consumer" moot);
    RT-040 and RT-038 (defense-in-depth for states
    already prevented by runtime validation and, as
    of this sweep, `load` validation); RT-014
    (accepted as inherent to local-CLI trust model
    per the original finding's own rationale);
    RT-013 (forward-looking design debt, not a
    current bug).

- Add dynamic plugin host (v0.43.0)

    PLG-HOST. The main binary can now discover and
    load plugin cdylibs at runtime via `libloading`.
    New module
    `crates/rustwerk/src/bin/rustwerk/plugin_host.rs`
    is the only place in the binary with `unsafe`
    code, gated behind `#[cfg(feature = "plugins")]`
    and `#![allow(unsafe_code)]` at module level.
    The workspace enforces `unsafe_code = "forbid"`
    which Cargo cannot relax per-module, so
    `crates/rustwerk/Cargo.toml` redeclares the
    lints block with `unsafe_code = "deny"` and the
    full clippy config duplicated from the
    workspace — the only way to grant one module an
    `#[allow]` while keeping the rest of the binary
    unsafe-free.

    `discover_plugins(project_root)` scans
    `<project>/.rustwerk/plugins/` then
    `~/.rustwerk/plugins/`. The original design also
    scanned `target/debug` and `target/release` for
    dev convenience, but the red-team review
    correctly flagged this as a code-execution
    vector — every cdylib cargo drops into
    `target/*` (build-script artifacts, transitive
    dep cdylibs, proc-macros on some platforms) is
    executed via `Library::new`'s DLL-constructor
    call *before* the API-version check runs. The
    path is now gated behind `RUSTWERK_PLUGIN_DEV=1`
    and end users never see it. `home_dir()` also
    treats empty `HOME`/`USERPROFILE` as absent so a
    `HOME=` invocation can't turn the plugin scan
    into a CWD-relative scan.

    Each load: `libloading::Library::new` → call
    `rustwerk_plugin_api_version` (reject mismatch)
    → cache the remaining three FFI entry points as
    plain fn pointers alongside the live `Library`.
    `LoadedPlugin::push_tasks(config_json,
    tasks_json)` passes two `CString` inputs plus an
    out-pointer, enforces a 10 MiB size cap on the
    response, and frees the plugin-owned pointer on
    both success and error paths. Parse and free are
    sequenced through a `parse_plugin_response<T>`
    helper that owns the byte buffer before
    returning, so the `CStr` borrow is statically
    dropped before the plugin's allocator reclaims
    the pointer — eliminating a future
    use-after-free regression surface. See RT-091,
    RT-092 in `redteam-log.md` for the remaining
    deferred items (library constructors run before
    version check; structured incident logging
    needs PLG-CLI).

    `xtask/src/main.rs` gains a
    `MODULE_COVERAGE_EXEMPT` list. FFI host paths
    need a real cdylib to exercise (covered by
    integration tests once PLG-JIRA lands), and
    three `lib.rs` files are re-exports/stubs only.
    Exempt files show with a `~` marker; path
    separators normalised once so one
    forward-slash entry works on both Windows and
    Unix.

- Add global `--json` output flag (v0.42.0)

    CLI-JSON. Every command now accepts a global
    `--json` flag (`#[arg(long, global = true)]` in
    the `Cli` struct) and emits a pretty-printed JSON
    DTO to stdout instead of the default human text.
    Integration tests cover every shape; `llms.txt`
    documents the contract so AI agents can rely on
    the wire format instead of scraping text. Error
    output is unchanged (structured JSON errors are
    AI-ERRORS' job). `batch` always emitted JSON, so
    `--json` is now rejected there as redundant
    instead of silently swallowed.

    During the Artisan review the initial `json: bool`
    threading through every `cmd_*` was flagged as
    mixing business logic with presentation — the
    follow-up refactor in the same commit reshapes
    every command to return an owned DTO implementing
    `Serialize + RenderText`. A new
    `src/bin/rustwerk/render.rs` module holds the
    trait plus a generic `emit<T>(&T, OutputFormat)`
    helper; the ~20 if/else branches collapse to a
    single call site per command in `main.rs`.
    Adding a new output format (yaml, ndjson) is now
    a single-file change. See AQ-063..AQ-072 in
    `artisan-resolved.md`.

    Red team caught three real issues, all resolved in
    the same commit: `task describe --json` leaked
    absolute filesystem paths (RT-091 — now project-
    relative via `strip_prefix(&root)`); no size cap on
    description file read (RT-090 — `MAX_DESCRIBE_BYTES
    = 1 MiB` with `File::take`, symlink to `/dev/zero`
    now refused); and `serde_json` aborting on
    non-finite floats post-save (RT-089 — all `f64`
    fields wrapped in `Option<f64>` via a `finite()`
    helper that serializes `NaN`/`Inf` as `null`).
    `BrokenPipe` on stdout is now a clean exit. One
    finding deferred: `commands/task.rs` is 573 lines,
    over the 500-line module threshold; logged as
    AQ-073 pending a natural split.

- Wire plugin crates into workspace (v0.41.0)

    PLG-WORKSPACE scaffolding for the plugin architecture.
    `crates/rustwerk-jira-plugin` is added as a new
    workspace member with `crate-type = ["cdylib"]` and
    `unsafe_code = "allow"` (needed once FFI exports land;
    Cargo cannot override a single workspace lint, so the
    full lint block is inlined). The main `rustwerk` crate
    picks up two new deps: `rustwerk-plugin-api` (path dep,
    non-optional) and `libloading = "0.8"` (optional,
    gated behind a new default `plugins` feature). Current
    jira-plugin contents are a doc-comment stub — actual
    FFI entry points arrive with PLG-JIRA, and dynamic
    loading arrives with PLG-HOST. Reviewer flagged the
    half-wired feature and lint duplication — both logged
    as deferred in `redteam-log.md` (RT-089, RT-090) and
    `artisan-log.md` (AQ-062) until PLG-HOST lands and the
    consumption pattern is visible.

- Add cross-platform install scripts (v0.40.0)

    `scripts/install.sh` (POSIX) and `scripts/install.ps1`
    (PowerShell) let end users install rustwerk with a
    single `curl | sh` or `irm | iex` command — no Rust
    toolchain required. Both scripts detect OS/arch, resolve
    the latest tag (via the GitHub API with a
    `releases/latest` redirect fallback for rate-limited
    callers, overridable with `RUSTWERK_VERSION`), download
    the matching archive produced by `.github/workflows/release.yml`,
    verify its SHA256 against the published `SHA256SUMS`,
    and drop the binary in `$HOME/.local/bin` or
    `%LOCALAPPDATA%\Programs\rustwerk\bin` (overridable via
    `RUSTWERK_INSTALL_DIR`). The Windows script forces
    TLS 1.2 for Windows PowerShell 5.1 and consults
    `PROCESSOR_ARCHITEW6432` to detect 64-bit Windows from
    a 32-bit host. PATH handling is symmetric: both scripts
    print a hint by default; `install.ps1` only mutates the
    persistent user PATH when `RUSTWERK_MODIFY_PATH=1`.
    README gains an `Install` section documenting the
    one-liners and environment overrides.

- Add `rustwerk-plugin-api` crate (v0.40.0)

    New workspace member `crates/rustwerk-plugin-api`
    defines the stable contract between the rustwerk host
    and plugin dynamic libraries. Provides serde DTOs
    (`PluginInfo`, `PluginResult`, `TaskPushResult`,
    `TaskDto`), the `API_VERSION = 1` constant, and
    `unsafe extern "C" fn` type aliases for the four FFI
    entry points a plugin must export
    (`rustwerk_plugin_api_version`,
    `rustwerk_plugin_info`,
    `rustwerk_plugin_push_tasks`,
    `rustwerk_plugin_free_string`). Safe helper
    functions `serialize_to_cstring` and
    `deserialize_from_cstr` reduce plugin-author
    boilerplate without requiring unsafe code inside the
    API crate itself — pointer conversions remain the
    caller's responsibility in their `extern "C"`
    wrappers. Depends only on `serde` and `serde_json`
    so plugins do not pull in the full rustwerk tree.

- Add `task rename` command (v0.39.0)

    New `rustwerk task rename <OLD> <NEW>` changes a
    task's ID while preserving status, effort log, tags,
    and assignee. `Project::rename_task` moves the
    `BTreeMap` key and rewrites dependency references
    across all other tasks (with defensive dedup and
    self-reference stripping to preserve the invariants
    that `add_dependency` enforces). The CLI wrapper
    preflight-checks the destination description-file
    path and refuses to overwrite. Batch gets a matching
    `task.rename` command; filesystem side effects
    (description rename/remove) are collected as typed
    `FileSideEffect` values during `execute_one` and
    replayed after `save_project`, so chained renames
    (A→B, B→C) apply files in the correct order and any
    failure is reported in a JSON envelope on stderr
    with a non-zero exit. `task remove` (CLI + batch) now
    also cleans up the `.md` description file for
    consistency. New `file_store::rename_task_description`
    and `remove_task_description` helpers centralize the
    filesystem lifecycle; the rename helper refuses to
    overwrite existing destinations.

- Adopt rustbase template and add `xtask check`

    Retroactively linked rustwerk to the
    [rustbase](https://github.com/breki/rustbase) template
    via `.template-sync.toml` (pinned to rustbase `076cf44`
    / v0.4.0). Added `cargo xtask check` for fast
    compile-only verification with concise output
    (`Check OK` or `FAILED: N compilation error(s)` with up
    to 10 error lines). New slash commands: `/check`,
    `/validate`, `/test`, `/template-improve`,
    `/template-sync`. Future template updates can be
    pulled via `/template-sync`; rustwerk-side feedback
    accumulates in `docs/developer/template-feedback.md`.

### 2026-04-07

- Add `--version` flag to CLI (v0.38.0)

    `rustwerk --version` prints `rustwerk X.Y.Z`.
    Uses clap's built-in `version` attribute which pulls
    from `Cargo.toml` automatically.

### 2026-04-04

- Add `task describe` command for task description files (v0.37.0)

    `task describe <ID>` reads and displays
    `.rustwerk/tasks/<ID>.md`. Shows the file path when
    no description exists. Added `task_description_path`
    to `file_store` (accepts `&TaskId` for type safety).
    Updated `/next-task` skill to use `task describe`.

- Add `--tag` filter to `task list` command (v0.36.0)

    `task list --tag <TAG>` filters tasks by tag. Combines
    with existing filters (`--status`, `--assignee`,
    `--available`, `--chain`). Uses `Task::has_tag` for
    case-insensitive matching.

- Add `--tags` flag to `task add` and `task update` (v0.35.0)

    CLI commands `task add` and `task update` now accept
    `--tags` for comma-separated tag input. Batch commands
    `task.add` and `task.update` accept a `tags` JSON array.
    Added `Task::set_tags` domain method for replacing all
    tags at once. Updated manual.

- Add `tags` field to task model (v0.34.0)

    Tasks now support an optional `tags` field using a
    validated `Tag` newtype — slug-like strings (lowercase
    alphanumeric + hyphens, max 50 chars), sorted and
    deduplicated, max 20 per task. Domain methods:
    `add_tag`, `remove_tag`, `has_tag`. Tags are validated
    on deserialization, omitted from JSON when empty.
    Updated project file spec with Tags section.

- Add `dev.add` and `dev.remove` batch commands (v0.33.0)

    Batch operations can now register and remove developers
    via `dev.add` (with optional `email` and `role` fields)
    and `dev.remove`. This completes the batch command set
    needed for fully automated project setup from a WBS.

- Add `RUSTWERK_USER` env var for developer identity (v0.32.0)

    `task assign` and `effort log` now fall back to the
    `RUSTWERK_USER` environment variable when no developer
    is specified on the command line. Batch commands remain
    fully explicit — no env var fallback by design. Added
    `resolve_developer()` helper, project file format spec,
    typical workflow section in the manual.

- Harden project configuration (v0.31.0)

    Consolidated workspace lints (`[workspace.lints]`),
    added `unsafe_code = "forbid"`, enabled clippy pedantic
    with curated allows, disabled unused `chrono` default
    features, added `*.pdb` to `.gitignore`, added SHA256
    checksums to release workflow. Extended `/commit` red
    team to review GitHub Actions and project config files.

- Fix GitHub Actions CI and add release workflow (v0.31.0)

    Rewrote CI workflow to use only GitHub-owned actions
    (replaced `dtolnay/rust-toolchain` and
    `Swatinem/rust-cache` with `rustup` + `actions/cache`).
    Split single validate job into parallel fmt, clippy, and
    multi-platform test jobs (ubuntu, windows, macos). Added
    `workflow_call` trigger so the release workflow can reuse
    CI. New `release.yml` triggers on `v*.*.*` tags, runs CI,
    builds release binaries for 5 targets (linux x86_64,
    linux arm64, windows x86_64, macos x86_64, macos arm64),
    and publishes a GitHub Release with notes extracted from
    `CHANGELOG.md`. Added initial `CHANGELOG.md`.

### 2026-04-03

- Add `status` command for compact project dashboard (v0.31.0)

    New `rustwerk status` top-level command shows a compact
    project dashboard: completion bar, task counts by status,
    active tasks with assignees, bottleneck count, and
    remaining critical path length. Completes the CLI-VIZ
    WBS task.

- Add `tree` command for ASCII dependency tree (v0.30.0)

    New `rustwerk tree` command renders the dependency DAG
    as an ASCII tree with box-drawing characters and status
    indicators (✓/>!/~/ ). Shared tasks appear expanded
    under their first parent and as back-references under
    subsequent parents. `--remaining` flag excludes Done
    and OnHold tasks. Domain layer gains `task_tree()` and
    `task_tree_remaining()` on `Project`, producing a
    `TreeNode` enum (Task/Reference variants).

- Add ON_HOLD task status (v0.29.0)

    New `Status::OnHold` variant for intentionally deferred
    tasks. Valid transitions: TODO→ON_HOLD, IN_PROGRESS→
    ON_HOLD, ON_HOLD→TODO, ON_HOLD→IN_PROGRESS. On-hold
    tasks are excluded from `available_tasks()`,
    `gantt --remaining`, and the remaining critical path.
    They render as dim in Gantt charts and appear in
    summary/report counts with a dedicated label. Phase 5
    git tasks marked on-hold pending workflow design.

- Add task list filters: status, assignee, chain (v0.28.0)

    New `--status`, `--assignee`, and `--chain` flags on
    `task list`. Filters compose with each other and with
    existing `--available`/`--active` flags. Domain layer
    gains `tasks_by_status()`, `tasks_by_assignee()`, and
    `dependency_chain()` methods on `Project`. The chain
    filter walks transitive dependencies and returns results
    in topological order.

- Add `report bottlenecks` CLI command (v0.27.0)

    New `rustwerk report bottlenecks` command shows a PM-facing
    bottleneck report: tasks blocking the most downstream work,
    enriched with status, ready indicator, and assignee. The
    `Bottleneck` struct now carries `status`, `assignee`, and
    `ready` fields directly from the domain layer. Dynamic
    column widths adapt to task ID length.

- Add bottleneck detection query (v0.26.0)

    New `Project::bottlenecks()` method finds tasks with the
    most transitive downstream dependents. Uses DFS on a
    reverse dependency graph to count all tasks transitively
    blocked by each task. Returns results sorted by count
    descending. Excludes done tasks.

- Add blocked-by-deps auto-detection (v0.25.0)

    New `Project::dep_blocked_tasks()` method returns tasks
    that are TODO but have at least one incomplete dependency.
    Complements `available_tasks()` (all deps done) and
    `active_tasks()` (in-progress).

- Add `dev add` and `dev remove` commands (v0.24.0)

    New `rustwerk dev add` registers a developer with name,
    optional email and role. `rustwerk dev remove` unregisters
    a developer (blocked if any task is assigned to them).

- Add `report effort` command (v0.23.0)

    New `rustwerk report effort` command shows effort
    breakdown per developer with hours and percentage.

- Add `dev list` command (v0.22.0)

    New `rustwerk dev list` command shows all registered
    developers with name, email, and role.

- Add `report complete` command (v0.21.0)

    New `rustwerk report complete` command shows a PM-friendly
    completion summary: status breakdown, visual progress bar,
    estimated vs actual effort with burn rate, complexity
    totals, and remaining critical path with task IDs.

- Upgrade Gantt time axis to box-drawing chars (v0.20.0)

    Axis line now uses `┬` for tick marks and `─` for the
    horizontal rule, replacing plain `|` and spaces.

- Add `--remaining` flag to `gantt` command (v0.19.0)

    `rustwerk gantt --remaining` filters out done tasks,
    showing only the remaining work. Filtering happens after
    scheduling so bar positions reflect the full timeline.

- Red Gantt bars for critical path tasks (v0.18.0)

    Critical path tasks now render the entire line (marker,
    ID, and bar) in red, overriding the status-based color.
    Extracted `bar_style()` function for testability with 4
    new unit tests. Red chosen over bold/underline because
    those are not visible enough on most terminal themes.

- Fix Gantt chart alignment and bar overlap (v0.17.1)

    Three bugs fixed: (1) `TaskId::Display` didn't forward
    format specifiers, causing ID column padding to be ignored
    and bars to start at wrong columns. (2) Bar caps were added
    outside the scaled width, causing consecutive bars to overlap
    by 1 column. (3) Header tick marks were misaligned with bar
    positions. Added 4 visual integration tests for Gantt layout.

- Upgrade Gantt bars to Unicode blocks with caps (v0.17.0)

    Gantt bars now use Unicode block characters instead of ASCII:
    `█` (full block) for done/blocked, `▓` (dark shade) for
    in-progress filled portion, `░` (light shade) for remaining/todo.
    Bar brackets `[]` replaced with half-block caps `▐` `▌` for a
    polished look. New `left_cap()` and `right_cap()` methods on
    `GanttRow`. 7 new tests for character selection.

- Add terminal-width-aware Gantt scaling (v0.16.0)

    `rustwerk gantt` now detects terminal width via the `terminal_size`
    crate and scales bars proportionally when the chart would overflow.
    Scale factor capped at 1.0 (never stretches beyond 1:1). Tick
    interval widens at small scales. Minimum bar width of 1 character
    ensures no task disappears.

- Add Developer domain type and project registry (v0.15.0)

    New `Developer` struct with name, optional email, role, and
    specialties. `DeveloperId` newtype (lowercase ASCII alphanumeric).
    `Project` gains `developers` map with `add_developer` and
    `remove_developer` methods. Removal blocked if any task is
    assigned to the developer. JSON serialization round-trips.
    15 new tests (8 for DeveloperId/Developer, 7 for Project
    integration).

- Add ANSI colors to Gantt chart (v0.14.0)

    `rustwerk gantt` now renders with ANSI colors: green for done,
    yellow/bold for in-progress, red for blocked, dim for todo, cyan
    for critical-path markers. Auto-detects terminal via
    `std::io::IsTerminal`; respects `NO_COLOR` env var. Scale header
    rendered in dim. No external dependencies.

- Fix `--available` to show TODO only, add `--active` flag (v0.13.1)

    `task list --available` now shows only TODO tasks whose deps are
    all done (previously included IN_PROGRESS). New `task list --active`
    shows only IN_PROGRESS tasks. `active_tasks()` query on `Project`.
    3 new tests.

- Add ASCII Gantt chart command (v0.13.0)

    `rustwerk gantt` renders a dependency-aware Gantt chart. Tasks
    positioned by topological sort — start column = max(end of deps).
    Bar width = complexity score. Fill shows status: `#` done, `#.`
    in-progress, `.` todo, `!` blocked. Critical path tasks marked
    with `*`. Scale header with column markers every 5 units.
    `gantt_schedule()` on `Project` returns `Vec<GanttRow>`. 6 new
    tests.

- Add WBS import/export schema for AI agents (v0.12.0)

    New `ai::wbs_schema` module with `WbsTaskEntry` struct for
    bulk task creation. `parse_wbs`/`serialize_wbs` for JSON I/O.
    `import_into_project` creates tasks then adds dependencies
    (two-pass, idempotent — skips existing IDs). `export_from_project`
    serializes current tasks back to WBS format. Rejects cycles
    during import. 8 new tests including round-trip and cycle
    detection.

- Add code coverage enforcement and red team findings log

    `cargo xtask coverage` runs `cargo-llvm-cov` and enforces a 90%
    line coverage threshold. `cargo xtask validate` now includes
    coverage as the final step (clippy → tests → coverage). Added
    22 CLI integration tests (`crates/rustwerk/tests/cli_integration.rs`)
    and 18 in-process batch command tests. Coverage went from 68% to
    94.9%. Added `docs/developer/redteam-log.md` with all 12 historical
    findings backfilled. Updated `/commit` to maintain the log and warn
    when 10+ findings are open.

- Add atomic batch command execution (v0.11.0)

    `rustwerk batch [--file path]` executes a JSON array of commands
    atomically — loads project once, runs all commands in-memory, saves
    only if all succeed. On any failure, nothing is persisted and the
    error is reported as JSON with the failing command index. Reads from
    file or stdin. Supports all 10 command types (`task.add`,
    `task.status`, `task.depend`, `effort.log`, etc.). Designed for AI
    agent integration — agents can pipe structured JSON to execute
    complex multi-step operations in a single atomic call.

- Add project status summary to `show` command (v0.10.0)

    `Project::summary()` returns `ProjectSummary` with task counts
    by status, % complete, total estimated/actual effort hours, and
    total complexity. `show` command now displays a full project
    dashboard. Updated `/next-task` to use direct binary, `/commit`
    to always run red team on code changes. 3 new tests.

- Add effort logging and estimation (v0.9.0)

    `effort log ID AMOUNT --dev NAME` logs effort on IN_PROGRESS tasks.
    `effort estimate ID AMOUNT` sets estimated effort. `Effort::to_hours()`
    converts all units to hours (1D=8H, 1W=40H, 1M=160H).
    `Task::total_actual_effort_hours()` sums logged entries. 5 new tests.

### 2026-04-02

- Add assignee management and `/next-task` command (v0.8.0)

    `task assign ID --to NAME` and `task unassign ID` CLI commands
    with `assign`/`unassign` domain methods on `Project`. Added
    `/next-task` Claude Code skill that lists available WBS tasks,
    lets the user pick one, marks it in-progress, plans if needed,
    implements with TDD, and commits. 6 new tests.

- Add `--force` flag to `task status` (v0.7.0)

    `task status ID STATUS --force` bypasses transition validation,
    allowing corrections like DONE→TODO. `set_status` domain method
    now takes a `force: bool` parameter.

- Add task remove and update commands (v0.6.0)

    `task remove` deletes a task, guarded by dependency check — cannot
    remove a task that others depend on. `task update` changes title
    and/or description (use `--desc ""` to clear). Domain methods
    `remove_task` and `update_task` on `Project`. 9 new tests.

- Add topological sort, critical path analysis, and `*` marker (v0.5.0)

    `topological_sort()` via Kahn's algorithm returns tasks in
    dependency order. `critical_path()` finds the longest chain by
    complexity weight using DP on the topological order.
    `critical_path_set()` returns the set for O(1) membership checks.
    `task list` now marks critical-path tasks with `*`. 7 new tests.

- Add dependency management and available task filtering (v0.4.0)

    `task depend` and `task undepend` CLI commands manage task
    dependencies. `add_dependency` validates both task IDs exist,
    rejects self-dependencies and cycles via DFS. `task list
    --available` shows only tasks whose dependencies are all done.
    `available_tasks()` query on `Project` aggregate. All WBS
    dependencies imported into dogfooding project file. 15 new
    tests for dependency CRUD, cycle detection, and availability
    filtering.

- Add task management CLI commands (v0.3.0)

    `task add` creates tasks with optional mnemonic ID, description,
    complexity, and effort estimate. Auto-generates sequential IDs
    (T1, T2...) when no ID is provided. `task status` sets task status
    with transition validation. `task list` displays all tasks with
    status and complexity. Domain methods `add_task`, `add_task_auto`,
    and `set_status` on `Project` aggregate. Enables dogfooding —
    rustwerk can now track its own development tasks.

- Implement Phase 1: core domain, persistence, CLI init/show (v0.2.0)

    Added DDD domain model: `Project` aggregate, `Task` with `Status`
    enum, `Effort` with time-unit parsing ("2.5H", "1D", "0.5W",
    "1M"), `DomainError` via `thiserror`. JSON persistence layer with
    file-based `ProjectStore` saving to `.rustwerk/project.json`. CLI
    `init` creates a new project file, `show` displays project summary.
    44 unit tests covering domain types, serialization round-trips, and
    file store operations.

- Initial project scaffold (v0.1.0)

    Set up workspace with `rustwerk` library/binary crate and `xtask`
    build tooling. CLI skeleton using `clap` with `serde`/`serde_json`
    for structured I/O. Workspace-level `#[deny(warnings)]` and
    clippy pedantic lints enabled.

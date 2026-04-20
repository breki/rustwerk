# Red Team Findings — Open

Open findings from red team reviews, newest first.
Fixed findings are moved to
[redteam-resolved.md](redteam-resolved.md).

**Next ID:** RT-136

---

### RT-128 — Flaky env-var race between `plugin_host::tests::discovery_dirs_{without,includes}_target_with_env`

- **Date:** 2026-04-20
- **Category:** Test hygiene
- **Commit context:** Observed during the
  PLG-JIRA-UPDATE review sweep (v0.50.0). Unrelated
  to the jira plugin; discovered while running the
  full workspace test suite.
- **Description:** The two tests at
  `crates/rustwerk/src/bin/rustwerk/plugin_host.rs:425-456`
  racily mutate the `RUSTWERK_DEV_DIRS` env var.
  `discovery_dirs_without_env` calls `env::remove_var`,
  `discovery_dirs_includes_target_with_env` calls
  `env::set_var` and asserts the target-dir presence.
  Running under `cargo test --workspace` (which does
  not serialize tests in the same binary) produces
  intermittent failures of the `…_with_env` test
  because the sibling removes the var mid-run.
  Pass/fail depends on scheduling.
- **Impact:** Occasional CI red herrings on the
  workspace test path; no production impact.
- **Suggested fix:** `#[serial_test::serial]` (add
  the `serial_test` dev-dep) or a `std::sync::Mutex`
  guard shared by the two tests. Alternative: fold
  both assertions into one `#[test]` that
  set → assert → remove → assert, eliminating the
  race.

---

### RT-127 — `jira_url` not validated for scheme / userinfo at config boundary

- **Date:** 2026-04-20
- **Category:** Security (token exfiltration via config poisoning)
- **Commit context:** Raised during the
  PLG-JIRA-UPDATE review sweep (v0.50.0); deferred
  because the validation site is `JiraConfig::from_json`
  (`crates/rustwerk-jira-plugin/src/config.rs`),
  which is out of scope for this PR.
- **Description:** `direct_issue_url` and
  `gateway_issue_url` interpolate `jira_url` into
  URLs with no scheme / userinfo check. A
  `jira_url` embedding credentials
  (`https://attacker:pwd@evil.com`) would cause
  `basic_auth_header` to leak the user's Jira token
  to `evil.com`. `ureq` rejects non-http(s) at
  transport time, but defense-in-depth at the
  config boundary is cheap.
- **Impact:** Token exfiltration when an attacker
  controls config-file content.
- **Suggested fix:** In `JiraConfig::from_json`,
  parse `jira_url` with `url::Url::parse`, assert
  scheme ∈ `{http, https}` and
  `username().is_empty() && password().is_none()`.
  Reject with a clear error otherwise.

---

### RT-126 — No regression test for probe-then-delete TOCTOU race

- **Date:** 2026-04-20
- **Category:** Test coverage of documented behavior
- **Commit context:** Noted during the
  PLG-JIRA-UPDATE review sweep (v0.50.0); the
  behavior is correct but unpinned.
- **Description:** If an issue is deleted between
  the probe (200) and the subsequent PUT (404), the
  current behavior is "fail this push, recreate on
  next push." That's the right call — the PUT 404
  is a late signal and we can't distinguish it from
  a genuine write-scope 404 — but nothing in the
  test suite locks the behavior in.
- **Impact:** A future refactor could silently flip
  PUT-404 → recreate without any test failing.
- **Suggested fix:** Add
  `probe_200_put_404_fails_without_touching_state`
  at `lib.rs` test module: queue
  `ok(200, probe_body)` + `ok(404, "gone")`; assert
  `!rs[0].success`, `plugin_state_update.is_none()`,
  message mentions `"update"` and `"404"`.

---

### RT-124 — Probe 2xx trusted without body validation

- **Date:** 2026-04-20
- **Category:** Correctness (MITM / captive-portal false-success)
- **Commit context:** Raised during the
  PLG-JIRA-UPDATE review sweep (v0.50.0); deferred
  as lower-priority than RT-121/122/123.
- **Description:** `push_one_update`'s probe
  dispatch (`lib.rs`, `ProbeOutcome::Exists` arm)
  checks only the HTTP status. An intercepting
  proxy returning 200 HTML ("please log in") for
  the probe and 204 for the subsequent PUT would
  produce a reported success while the Jira issue
  is never touched — and `last_pushed_at` is
  refreshed, suppressing future retries.
- **Impact:** Silent no-op writes behind a corporate
  MITM; harder to diagnose because
  `plugin_state.jira` looks fresh.
- **Suggested fix:** Parse the probe body minimally
  and require `response.get("key").as_str() ==
  Some(key.as_str())` before treating as
  `Exists`. Reject `Content-Type: text/html` if we
  add header inspection to `HttpResponse`. Low
  priority; ship when we do PLG-JIRA-E2E and have
  real-world evidence of what the response body
  actually looks like.

---

### RT-117 — API-version bump strands out-of-tree v1 plugin binaries with no compat shim

- **Date:** 2026-04-20
- **Category:** API-version policy
- **Commit context:** feat: per-task plugin-state
  round-trip in the plugin API (v0.48.0)
- **Description:** `load_plugin`'s version check
  rejects v1 plugins outright; the new message tells
  authors to rebuild. The v1 wire format is a proper
  subset of v2 thanks to `#[serde(default)]` on the
  new fields, so a compat shim is cheap: treat v1 as
  "no plugin_state support" and still load.
- **Why not fixed in-commit:** No out-of-tree v1
  plugin exists today. Closing this before the first
  external plugin ships is sufficient; doing it
  speculatively adds branching the in-tree jira
  plugin doesn't exercise.
- **Suggested fix:** In `load_plugin`, branch on
  `version == 1 || version == 2`; when v1, log a
  deprecation warning and still construct a
  `LoadedPlugin` that passes `plugin_state: None` in
  and ignores any `plugin_state_update` in responses.
  Revisit when API v3 lands.

### RT-116 — Concurrent `plugin push` invocations race on project.json

- **Date:** 2026-04-20
- **Category:** Concurrency
- **Commit context:** feat: per-task plugin-state
  round-trip in the plugin API (v0.48.0)
- **Description:** `file_store::save` is called with
  no file lock. Two concurrent `rustwerk plugin push`
  processes can both load project.json, both push to
  external systems, then the later save silently
  overwrites the earlier one's state updates.
- **Why not fixed in-commit:** General rustwerk
  concurrency concern; fixing just the plugin-push
  path is a partial fix. A crate-wide advisory file
  lock on project.json is the right scope.
- **Suggested fix:** `fs2::FileExt::try_lock_exclusive`
  on project.json at load time (or in `file_store`),
  with a clear error when another process holds the
  lock. Alternatively document concurrent rustwerk
  invocations as unsupported.

### RT-115 — Plugin names are case-sensitive; `jira` and `Jira` create parallel state namespaces

- **Date:** 2026-04-20
- **Category:** Edge case
- **Commit context:** feat: per-task plugin-state
  round-trip in the plugin API (v0.48.0)
- **Description:** `validate_plugin_name` allows up
  to 64 chars of `[A-Za-z0-9_-]` case-sensitively.
  `discover_plugins` shadowing check uses `==`. Two
  plugins self-identifying as `jira` and `Jira` can
  both load without shadowing detection, producing
  separate `plugin_state[jira]` and
  `plugin_state[Jira]` namespaces in project.json
  that look like one plugin to the user.
- **Why not fixed in-commit:** Pre-existing issue of
  the plugin-host layer; PLG-API-STATE makes it more
  visible via BTreeMap keys in project.json but is
  not the root cause. Fix belongs in `plugin_host`.
- **Suggested fix:** Lowercase-normalize in
  `validate_plugin_name` (`plugin_host.rs:365-381`),
  and use `eq_ignore_ascii_case` in the shadowing
  check at `plugin_host.rs:273-281`.

### RT-109 — TOCTOU between `dest.exists()` and `fs::copy`

- **Date:** 2026-04-20
- **Category:** Correctness (racy `--force` gate)
- **Commit context:** feat: add `rustwerk plugin
  install` subcommand (v0.47.0)
- **Description:** `install_from_path` checks
  `dest.exists()` then calls `fs::copy(source, &dest)`.
  On the `!force` path, a concurrent writer can create
  `dest` between the two calls and `fs::copy` silently
  overwrites (uses `O_TRUNC` / `CREATE_ALWAYS`
  semantics). The advertised protection "passing
  `--force` is required to overwrite an existing
  plugin" can therefore be defeated by a race.
- **Why not fixed in-commit:** `plugin install` is a
  local, single-project, developer-initiated
  operation — two concurrent `plugin install` runs
  aren't a realistic threat model. The symlink vector
  that shared this code path (RT-106) *was* fixed in
  this commit. When the install flow migrates to a
  temp-file + `fs::rename` pattern, this finding gets
  closed automatically.
- **Suggested fix:** Write to
  `dest_dir/.<filename>.tmp`, verify, then atomically
  `fs::rename` into place. Closes this finding and
  any remaining variants of RT-106 in one stroke.

### RT-108 — Windows-reserved filenames + trailing-dot names pass validation

- **Date:** 2026-04-20
- **Category:** Platform-specific UX
- **Commit context:** feat: add `rustwerk plugin
  install` subcommand (v0.47.0)
- **Description:** `source.file_name()` is copied
  verbatim into `dest_dir.join(filename)`. A source
  named `CON.dll`, `AUX.dll`, `COM1.dll`, or a
  filename with a trailing dot/space on Windows
  either produces a cryptic OS error or results in a
  file that later `discover_plugins` can't find
  (Windows silently strips trailing dots at the
  filesystem level, so the installed and looked-up
  paths diverge).
- **Why not fixed in-commit:** No plugin author has
  produced a reserved-name cdylib yet; surfacing this
  to the validation layer is a pure hardening task,
  not a user-facing bug today.
- **Suggested fix:** In `validate_cdylib_extension`
  (or a sibling `validate_cdylib_filename`), reject
  Windows-reserved stems (`CON`, `PRN`, `AUX`, `NUL`,
  `COM1..9`, `LPT1..9`) and filenames ending in `.`
  or space. Reuse the same allowlist shape as
  `validate_plugin_name` in `plugin_host`.

**Threshold:** when 10+ findings are open, a full-codebase
red team review is required before continuing feature work.

---

### RT-092 — Failed plugin loads are only surfaced via `eprintln!`

- **Date:** 2026-04-19
- **Category:** Incident detection (deferred)
- **Commit context:** feat: add dynamic plugin host
  (v0.43.0)
- **Description:** When
  `plugin_host::discover_plugins` encounters a plugin
  that fails to load (malicious DLL, architecture
  mismatch, missing symbol, version mismatch), the
  failure is reported as a single line on stderr and
  scan continues. There is no structured log, no
  persistent record, and nothing surfaces in normal
  CLI output.
- **Impact:** A user who unknowingly has a malicious
  `.so`/`.dll` in a plugin dir gets a one-line
  warning easy to miss in CLI noise. Incident
  detection is weak.
- **Fix options:** Once PLG-CLI lands, add `rustwerk
  plugin list` that surfaces both loaded and failed
  plugins with full paths, errors, and (ideally)
  SHA256 hashes. Consider persisting a structured log
  file in `.rustwerk/plugins-failed.log`.
- **Deferred rationale:** Requires the PLG-CLI
  surface and a broader decision on where structured
  logging lives; not worth retrofitting in PLG-HOST.

---

### RT-091 — Library constructors run before API-version check

- **Date:** 2026-04-19
- **Category:** Trust model (documented, inherent)
- **Commit context:** feat: add dynamic plugin host
  (v0.43.0)
- **Description:** `Library::new(path)` executes the
  shared object's initializers (ELF `.init_array`,
  Windows `DllMain`) before the plugin host can call
  `rustwerk_plugin_api_version`. A malicious library
  in a discovery directory has already executed
  arbitrary code by the time the host is in a
  position to "reject" it. This is inherent to
  dynamic loading on all supported platforms.
- **Impact:** The version-check guarantee is
  cosmetic for security purposes — it prevents API
  misuse after the fact, not compromise. Every file
  in a discovery directory is implicitly trusted to
  run code as the current user.
- **Fix options:** (a) sign plugins and verify
  signatures *before* `Library::new`; (b) maintain a
  user-scoped allowlist of plugin hashes; (c) both.
  All require new infrastructure.
- **Deferred rationale:** Addressed through trust
  narrowing for now — `target/*` dirs are no longer
  scanned by default (gated behind
  `RUSTWERK_PLUGIN_DEV=1`); only
  `<project>/.rustwerk/plugins/` and
  `~/.rustwerk/plugins/` are scanned. Documented
  explicitly in the module's trust-model section.

---

### RT-090 — jira-plugin lint block duplicated instead of inheriting workspace

- **Date:** 2026-04-19
- **Category:** Correctness (deferred)
- **Commit context:** feat: wire plugin crates into
  workspace (v0.41.0)
- **Description:** `crates/rustwerk-jira-plugin/Cargo.toml`
  copies the full `[lints.rust]` + `[lints.clippy]` block
  from the workspace root instead of using `[lints]
  workspace = true`. Only reason to deviate is
  `unsafe_code = "allow"` (cdylib FFI).
- **Impact:** Future workspace lint additions (e.g.
  tightening a rust lint) silently skip the one crate
  allowed to contain `unsafe`, weakening the security
  posture of the very crate that most needs it.
- **Constraint:** Cargo does not support overriding a
  single workspace lint. Workspace has `unsafe_code =
  "forbid"` which cannot be relaxed via `#![allow]`
  (forbid is strict). So the duplication is the only
  option unless we downgrade workspace to `deny` and
  add `#[allow(unsafe_code)]` at each FFI use site.
- **Fix options:** (a) accept current duplication and
  add a comment in jira-plugin's Cargo.toml noting the
  Cargo limitation; (b) downgrade workspace
  `unsafe_code` to `deny` so crates inherit + `#[allow]`
  at use sites.

---

### RT-089 — `unsafe_code = allow` relaxed on empty jira-plugin crate

- **Date:** 2026-04-19 (revised during backlog sweep)
- **Category:** Correctness / Hygiene (deferred — by design)
- **Commit context:** feat: wire plugin crates into
  workspace (v0.41.0)
- **Description:** `rustwerk-jira-plugin` sets
  `unsafe_code = "allow"` on what is currently a
  doc-comment-only crate. The relaxation is intended
  for the future `extern "C"` entry points (PLG-JIRA)
  but a stray `unsafe` block could land in the crate
  in the interim without any lint firing.
- **Partial resolution:** The originally-reported
  "feature flag is inert" and "SBOM sees libloading
  with no loader" sub-findings were resolved by
  PLG-HOST (v0.43.0) — the main binary now consumes
  `libloading` and `rustwerk-plugin-api` through the
  gated plugin_host module.
- **Remaining fix:** Revisit when PLG-JIRA ships and
  actual FFI code arrives. If jira-plugin ends up not
  needing `unsafe` (e.g. by deriving everything
  through `rustwerk-plugin-api` helpers), tighten
  back to workspace-inherit.

---

### RT-083 — Installer checksums are not signed

- **Date:** 2026-04-19
- **Category:** Security (deferred)
- **Commit context:** chore: add cross-platform install
  scripts
- **Description:** `scripts/install.sh` and
  `scripts/install.ps1` verify the downloaded release
  archive against a `SHA256SUMS` file fetched from the
  same GitHub release. This protects against transport
  corruption and non-repo MITM, but not against a
  compromise of the release pipeline itself — a stolen
  `GITHUB_TOKEN`, a malicious workflow edit, or a
  push-access compromise lets an attacker publish a
  matching `<archive, SHA256SUMS>` pair. End users
  piping the installer to `sh`/`iex` have no way to
  detect such a swap.
- **Impact:** Supply-chain trust floor is "GitHub
  release integrity." For a project that is intended
  to be installed by third-party developers and run
  with full user privileges, this is the most likely
  escalation path.
- **Suggested fix:** Sign `SHA256SUMS` in the release
  workflow (cosign keyless with Sigstore, or minisign
  with a committed public key), and have both installer
  scripts verify the signature before trusting the
  sums file. Keyless cosign is the lightest option —
  no key management, verification tool is a single
  static binary.

---

### RT-071 — `/template-sync` "all" option bypasses per-file review

- **Date:** 2026-04-19
- **Category:** Security (prompt-injection surface)
- **Commit context:** chore: adopt rustbase template
- **Description:** `.claude/commands/template-sync.md` step 6
  allows the user to accept "all" as the selection, applying
  every recommended template change in one pass. Because the
  agent reads raw upstream diff content (commit messages,
  file bodies) as input during categorization, an injected
  instruction in upstream content can influence decisions
  during bulk application.
- **Impact:** If rustbase is compromised or a malicious PR
  lands there, a single `/template-sync; all` run in
  rustwerk would apply attacker-directed edits without
  per-file review.
- **Suggested fix:** Remove the "all" option, or require
  per-file confirmation for any path outside a hardcoded
  allowlist (`.github/`, `xtask/`, `scripts/`). Also logged
  as upstream feedback in
  `docs/developer/template-feedback.md`.

### RT-070 — `/template-sync` uses untrusted URL from `.template-sync.toml`

- **Date:** 2026-04-19
- **Category:** Security (supply chain)
- **Commit context:** chore: adopt rustbase template
- **Description:** `.claude/commands/template-sync.md:24-26`
  reads `repo` from `.template-sync.toml` and passes it
  verbatim to `git remote add template <url>`. A malicious
  or accidentally-merged PR that swaps the URL for a
  lookalike/attacker repo would redirect all future
  `/template-sync` runs with no signal.
- **Impact:** Supply-chain redirect; attacker content gets
  surfaced for apply/skip selection.
- **Suggested fix:** Hard-code the expected upstream URL
  (`https://github.com/breki/rustbase`) in the slash command
  and assert `.template-sync.toml` matches before using it.
  Also logged as upstream feedback.

---


# Red Team Findings — Open

Open findings from red team reviews, newest first.
Fixed findings are moved to
[redteam-resolved.md](redteam-resolved.md).

**Next ID:** RT-091

**Threshold:** when 10+ findings are open, a full-codebase
red team review is required before continuing feature work.

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

### RT-089 — PLG-WORKSPACE half-wires the `plugins` feature

- **Date:** 2026-04-19
- **Category:** Correctness / Hygiene (deferred — by design)
- **Commit context:** feat: wire plugin crates into
  workspace (v0.41.0)
- **Description:** The `plugins` feature in
  `crates/rustwerk/Cargo.toml` gates only `libloading`
  (optional, default on), but nothing imports
  `libloading` yet. `rustwerk-plugin-api` is pulled in
  as a non-optional dep, and `rustwerk-jira-plugin`
  sets `unsafe_code = "allow"` on an empty crate.
  Triggers:
  1. `cargo build --no-default-features` produces an
     identical binary to the default — feature flag is
     inert.
  2. SBOM scanners see `libloading` in 0.41.0 with no
     loader present, suggesting plugin support that
     isn't there.
  3. `unsafe` guardrail is relaxed on jira-plugin
     before any `extern "C"` exists; a stray `unsafe`
     block could land in the interim with no lint.
- **Deferred rationale:** PLG-WORKSPACE is explicitly
  scaffolding per the WBS so PLG-HOST and PLG-JIRA can
  build on it. Findings resolve once PLG-HOST (loader)
  and PLG-JIRA (actual FFI code) land.
- **Fix:** Revisit when PLG-HOST + PLG-JIRA ship.
  Decide whether `rustwerk-plugin-api` should also be
  gated behind `plugins`, and whether `plugins` should
  remain a default feature.

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

### RT-082 — `rustwerk` 0.40.0 bumped without plugin loader consumer

- **Date:** 2026-04-19
- **Category:** Project Configuration (Low)
- **Commit context:** feat: add `rustwerk-plugin-api` crate
  (v0.40.0)
- **Description:** The host crate was minor-bumped to
  0.40.0 alongside the introduction of
  `rustwerk-plugin-api`, but `crates/rustwerk/Cargo.toml`
  does not yet depend on the new crate — no loader,
  no capability surface, no user-visible change. A user
  upgrading to 0.40.0 gets only a larger workspace and
  no functional delta. Decision was to keep the bump
  (new workspace crate is a forward-facing feature and
  the DIARY entry explicitly notes "API-only, no
  loader"), but the ambiguity is worth tracking so the
  next integration commit can revisit whether a
  CHANGELOG clarification is warranted.
- **Impact if not resolved:** Users may expect a plugin
  system to work in 0.40.0 when it does not.
- **Suggested resolution:** Either add a CHANGELOG
  entry under 0.40.0 explicitly labeling it as
  "plugin-api crate only; loader lands in a future
  release", or defer the next host-crate version bump
  until the loader actually lands and let the integration
  commit take credit for 0.41.0.

---

### RT-072 — Windows-reserved TaskIds (`CON`, `NUL`, `COM1`, etc.)

- **Date:** 2026-04-19
- **Category:** Correctness (Windows)
- **Commit context:** feat: `task rename` command (v0.39.0)
- **Description:** `TaskId::new` accepts arbitrary
  alphanumeric+`-_` strings and uppercases them, which means
  IDs like `CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`,
  `LPT1`–`LPT9` pass validation. `task_description_path`
  turns those into filenames like `.rustwerk/tasks/CON.md`,
  which have special semantics on Windows (console/device
  aliases). `task describe CON` and `task rename X CON` will
  either fail with confusing errors or produce odd I/O
  behavior on Windows. Pre-existing issue for `task add
  --id CON`; `task rename` widens the surface (rename into a
  reserved name after-the-fact). Low severity, Windows-only.
- **Example trigger:** On Windows, `rustwerk task add Foo
  --id CON` then `rustwerk task describe CON`.
- **Suggested fix:** Reject Windows-reserved names in
  `TaskId::new`. Out of scope for the v0.39.0 feat commit.

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

### RT-069 — `Bash(git checkout:*)` permission broader than workflow needs

- **Date:** 2026-04-19
- **Category:** Security (CLI/agent config)
- **Commit context:** chore: adopt rustbase template
- **Description:** `.claude/commands/template-sync.md:3`
  grants `Bash(git checkout:*)` in allowed-tools, but the
  documented workflow never invokes `git checkout`. The
  glob permits destructive variants (`checkout -f <ref>`,
  `checkout -- .`, `checkout -- <path>`) that can overwrite
  uncommitted work or move HEAD to an untrusted ref whose
  `.gitattributes`/hooks then activate on next git command.
- **Impact:** Prompt-injection escape: an instruction
  embedded in an upstream diff could induce a destructive
  checkout. The uncommitted-changes precondition is
  advisory, not a hard gate.
- **Suggested fix:** Remove `Bash(git checkout:*)` from
  the `allowed-tools` front-matter entirely. Also logged
  as upstream feedback.

### RT-068 — `cargo xtask check` reports "0 compilation error(s)" for non-rustc failures

- **Date:** 2026-04-19
- **Category:** Correctness / diagnostics
- **Commit context:** chore: adopt rustbase template (add
  `xtask check`)
- **Description:** `run_check` in `xtask/src/main.rs` only
  surfaces stderr lines matching `error[`/`error:`. When
  cargo exits non-zero for non-compilation reasons
  (manifest parse failure, lockfile corruption, missing
  registry network), the first diagnostic line may not
  match that pattern, leaving `errors.is_empty()` while
  status is non-zero. Output becomes `FAILED: 0
  compilation error(s)` with no body.
- **Impact:** Diagnostic black-hole for non-compile
  failures — user has no idea why the tool failed.
- **Suggested fix:** When `errors.is_empty()` but the
  process exited non-zero, fall back to printing the last
  ~20 lines of captured stderr. Also logged as upstream
  feedback.

---

### RT-040 — Cyclic deps silently vanish from --chain output

- **Date:** 2026-04-03
- **Category:** Correctness (Low)
- **Commit context:** v0.28.0 task list filters
- **Description:** If a dependency cycle somehow exists,
  `dependency_chain()` uses DFS post-order which may
  revisit or skip cycle participants. The `add_dependency`
  method already validates against cycles, so this is
  unreachable in normal operation.
- **Impact:** Low — defense-in-depth only.

### RT-038 — Dangling dependency refs truncate --chain

- **Date:** 2026-04-03
- **Category:** Correctness (Medium)
- **Commit context:** v0.28.0 task list filters
- **Description:** `dependency_chain()` silently skips
  dependency IDs that don't exist in `self.tasks`. If a
  task was removed without cleaning dependents (mitigated
  by DEP-GUARD which prevents this), the chain output
  would be incomplete with no warning.
- **Impact:** Medium — mitigated by existing DEP-GUARD.

---

### RT-024 — Cyclic graph in hand-edited JSON causes panic

- **Date:** 2026-04-03
- **Category:** Correctness
- **Commit context:** v0.13.0 Gantt chart
- **Description:** `topological_sort` silently returns
  fewer tasks when cycles exist in hand-edited JSON
  (runtime `add_dependency` prevents cycles but there's
  no validation on load). `critical_path` then panics
  accessing `dist[other_id]` for tasks not in the
  topological order.
- **Impact:** Hard crash on `rustwerk gantt` or
  `rustwerk task list` with corrupted project file.
- **Suggested fix:** Validate graph on load, or check
  `order.len() == tasks.len()` after topological sort.

### RT-014 — Batch `--file` reads any path (path traversal)

- **Date:** 2026-04-03
- **Category:** Security
- **Commit context:** v0.11.0 batch command / coverage
- **Description:** `--file` argument is passed directly to
  `fs::read_to_string` with no path validation. Any
  readable file on the system can be read. If the file
  isn't valid JSON, serde's error message may leak a
  fragment of the file content to stderr.
- **Impact:** Low for a CLI tool invoked by the user
  themselves. Higher risk if rustwerk is ever invoked by
  an orchestration layer with untrusted input.
- **Suggested fix:** Acceptable for current use case.
  Restrict path if rustwerk is ever used non-interactively.

### RT-013 — Batch rollback is implicit, no explicit snapshot

- **Date:** 2026-04-03
- **Category:** Code Quality
- **Commit context:** Coverage infrastructure
- **Description:** Batch "atomicity" relies on not calling
  `save_project` on error — there is no snapshot of the
  original project state that gets restored. If a future
  refactor moves the save earlier (e.g. for checkpointing),
  the atomicity guarantee silently breaks.
- **Impact:** Design debt — not a current bug but fragile
  for future changes.
- **Suggested fix:** Clone the project before the batch
  loop, restore the clone on error.

# Red Team Findings — Open

Open findings from red team reviews, newest first.
Fixed findings are moved to
[redteam-resolved.md](redteam-resolved.md).

**Next ID:** RT-093

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


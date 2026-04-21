# Template Feedback

Issues, improvements, and observations about the
[rustbase](https://github.com/breki/rustbase) template
discovered during development of this project.

Use this log to feed improvements back to the template.
Newest entries first.

rustwerk predates the rustbase template and was
retroactively linked to it; older rustwerk-driven
learnings have already been absorbed into the template.

---

## 2026-04-21

- **[Logged, not fixed locally] Template has no knowledge
  graph scaffolding, but every serious project grows
  one.** Rustwerk just built a browsable KG system from
  scratch: Zola site under `tools/kg/site/`, authoring
  notes under `knowledge/` with TOML frontmatter, four
  typed sections (`architecture/`, `concepts/`,
  `decisions/`, `integrations/`), a typed-edge
  convention (`extra.links = [{ relation, target }]`),
  custom Tera templates rendering badges / related /
  backlinks, shell wrappers under `tools/kg/scripts/`,
  and an `xtask kg {build,serve}` subcommand that
  auto-downloads the `zola` binary into
  `tools/kg/bin/`. All of this is generic: none of it
  is rustwerk-specific except the note content itself.
  Any rustbase-derived project eventually wants the
  same thing, and the authoring conventions (note
  types, edge relations) set a useful shared shape
  across projects. **Fix:** fold the scaffolding into
  rustbase — empty `knowledge/{architecture,concepts,
  decisions,integrations}/_index.md`, `tools/kg/`
  directory with templates + sass + scripts, the
  `xtask kg` module, a `knowledge/search.md` stub, the
  KG section in the template's `CLAUDE.md`, and the
  gitignore entries. Ship it dormant: an empty but
  working KG that renders a placeholder homepage until
  the first note is authored.

- **[Logged, not fixed locally] No pwsh twin
  convention for `.sh` scripts.** Rustwerk targets
  Windows as a first-class platform, but the template
  has no convention for how `scripts/*.sh` files
  should reach Windows users. Two options in tension:
  (a) ship a `.ps1` twin for every `.sh` — doubles the
  surface area and means every bugfix lands twice;
  (b) route everything through `cargo xtask` so the
  `.sh` / `.ps1` files are trivial 2-line wrappers
  with no logic to keep in sync. Rustwerk landed on
  (b) for the new KG scripts (see
  `tools/kg/scripts/kg-{build,serve}.{sh,ps1}`), but
  only after first writing a 70-line pwsh port that
  duplicated bash logic. **Fix:** document the
  convention in the template's `CLAUDE.md` —
  "non-trivial script logic lives in xtask; shell
  files are wrappers only" — and provide the canonical
  wrapper shapes. On the bash side:
  `exec cargo xtask <cmd> -- "$@"`. On the pwsh side:
  `& cargo xtask <cmd> -- @args; exit $LASTEXITCODE`
  with `$ErrorActionPreference = 'Stop'`.

- **[Logged, not fixed locally] `clippy::pedantic`
  trips on `PowerShell`, `FFI`, `JSON`, `WebSocket`
  and similar well-known identifiers via
  `clippy::doc_markdown`.** Writing doc comments that
  mention these terms requires each occurrence to be
  backticked, producing output like ``PowerShell on
  Windows, `curl` + `tar` on Unix`` where only
  `PowerShell` needed backticks for the lint, not for
  the reader. I hit this exactly once while writing
  `xtask/src/kg.rs`. The fix is already a documented
  clippy feature — a `clippy.toml` with a
  `doc-valid-idents` allowlist. **Fix:** ship
  `clippy.toml` in the template with a curated list
  of identifiers common in Rust infra code — e.g.
  `["PowerShell", "FFI", "JSON", "ABI", "WebSocket",
  "macOS", "GitHub", "OAuth", "stdin", "stdout",
  "stderr", "cdylib", "ADF"]`. Projects append
  domain-specific entries (`Jira`, `rustwerk`, `WBS`)
  on top.

---

## 2026-04-19

- **[Fixed locally] `xtask check` filter drops user errors
  mentioning "aborting".** `extract_check_errors` in
  `xtask/src/check.rs` (upstream) uses
  `!l.contains("aborting")` to strip the rustc summary
  line `error: aborting due to N previous errors`. Any
  legitimate error whose message contains the substring
  "aborting" — e.g.
  `compile_error!("aborting: feature X required")` —
  is also silently filtered out, so the user sees
  `FAILED: 0 compilation error(s)` with no body.
  **Fix (applied locally, suggested for template):**
  anchor the filter to the exact terminator:
  `!l.starts_with("error: aborting due to")`.

- **[Logged, not fixed locally] `xtask check` reports
  "0 compilation error(s)" for non-rustc failures.** When
  cargo exits non-zero for reasons other than compile
  errors (manifest parse failure, corrupted `Cargo.lock`,
  missing registry network, unsupported flag on older
  cargo), the first diagnostic line typically does not
  match the `error[`/`error:` prefix filter. Output
  becomes `FAILED: 0 compilation error(s)` with nothing
  to diagnose. **Fix:** when `errors.is_empty()` but the
  process exited non-zero, fall back to printing the last
  ~20 lines of captured stderr verbatim.

- **[Logged, not fixed locally] `/template-sync` grants
  `Bash(git checkout:*)` it never uses.** The slash
  command's `allowed-tools` front-matter includes
  `Bash(git checkout:*)`, but the documented workflow
  only uses `git diff`, `git show`, `git log`,
  `git rev-parse`, and `git fetch` for reading, plus
  `Edit`/`Write` for applying. The `checkout:*` glob
  permits destructive variants (`checkout -f <ref>`,
  `checkout -- .`, `checkout -- <path>`) that can destroy
  uncommitted work or move HEAD to an untrusted template
  ref whose `.gitattributes` / hooks activate on the next
  git command. Combined with the fact that diff content
  is treated as input to an LLM agent (a known
  prompt-injection surface), this is an escape vector.
  **Fix:** remove `Bash(git checkout:*)` from the
  allowed-tools entirely.

- **[Logged, not fixed locally] `/template-sync` uses
  untrusted upstream URL from `.template-sync.toml`.**
  Step 3 of the workflow reads `repo` from
  `.template-sync.toml` and feeds it to
  `git remote add template <url>`. A malicious PR
  changing that field (or a typo-squat lookalike) would
  silently redirect all future syncs to an attacker repo.
  Also, `git remote add` historically accepted hostile
  URL forms (`ext::sh`, `--upload-pack=...`). **Fix:**
  hard-code the expected upstream URL
  (`https://github.com/breki/rustbase`) in the slash
  command and assert `.template-sync.toml` matches before
  proceeding; refuse URLs outside the
  `https://github.com/breki/` prefix.

- **[Logged, not fixed locally] `/template-sync` "all"
  option bypasses per-file review.** Step 6 of the
  workflow accepts "all" to apply every recommended
  change in bulk. Because the agent reads raw upstream
  commit messages and diff bodies (prompt-injection
  surface) during step 5 categorization, a single
  compromised upstream commit could instruct the agent
  during bulk apply. **Fix:** remove "all" as an option,
  or gate it behind a hardcoded path allowlist (e.g.
  `.github/`, `xtask/`, `scripts/`); require per-file
  confirmation for anything else.

- **[N/A for rustwerk] rustwerk is CLI-only — frontend /
  e2e / `.mise.toml` / `crates/rustbase-web` not ported.**
  Not a template bug; just a note that the template's
  full-stack shape implies friction for projects that
  don't need a frontend. Suggest that the template
  README call out explicitly which directories are
  safe to delete for a CLI-only downstream, and that
  `/template-sync` default those paths to "skip" when
  they don't exist downstream. The current
  `template-sync.md` does not instruct the agent to
  notice and adapt to that shape mismatch.

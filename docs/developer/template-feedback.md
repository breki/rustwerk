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

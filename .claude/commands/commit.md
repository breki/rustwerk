---
description: Commit current changes following project conventions
allowed-tools: Bash(git status:*), Bash(git diff:*), Bash(git log:*), Bash(git add:*), Bash(git commit:*), Bash(cargo xtask validate*), Bash(sed -i*), Read, Edit, Agent
---

Commit the current changes following the project's git commit
conventions.

## Instructions

1. **Analyze current state** - Run these commands in parallel:
   - `git status` (never use -uall flag)
   - `git diff` for unstaged changes
   - `git diff --cached` for staged changes
   - `git log --oneline -5` for recent commit style reference

2. **Review changes** - Analyze what was changed and determine:
   - The commit type: feat, fix, chore, refactor, docs, test,
     style, perf
   - A concise subject line (imperative mood, no period)
   - A brief body explaining what and why

3. **Bump version** (for feat, fix, perf commits):
   - Read the current version from `crates/rustwerk/Cargo.toml`
   - Bump according to commit type:
     - `feat` → **minor** bump (0.1.0 → 0.2.0)
     - `fix`, `perf` → **patch** bump (0.1.0 → 0.1.1)
   - Edit `crates/rustwerk/Cargo.toml` to update the version
   - Run `cargo generate-lockfile` to update `Cargo.lock`
   - Include both files in staged files
   - Skip version bump for: docs, test, refactor, chore, style

4. **Validate** (when version was bumped in step 3):
   - Run `cargo xtask validate` to ensure clippy + tests pass
   - If validation **fails**, ask the user whether to commit
     anyway or abort. Wait for their answer before proceeding.
   - Skip this step if no version bump occurred

5. **Update development diary** (for significant changes):
   - Read `docs/developer/DIARY.md` to see format and recent
     entries
   - Add an entry for:
     - `feat`, `fix`, `perf` commits (functional changes)
     - Infrastructure/setup changes that affect developer
       workflow (build tools, CI/CD, dependencies)
   - Entries are in reverse chronological order (newest first)
   - Merge entries for the same day under one `### YYYY-MM-DD`
     heading
   - Attach the version to each entry title, not the date
     heading: `- Entry title (vX.Y.Z)` (use the version
     **after** the bump from step 3)
   - Use backticks for technical terms (function names, files,
     etc.)
   - Skip diary update for: docs, style, test, refactor, minor
     chores

6. **Code review** — Before staging, spawn **two** AI
   agents **in parallel** (in a single message with two
   Agent tool calls). Both read the source files but do
   not modify them.

   **IMPORTANT:** Always run both reviews when the diff
   contains code changes (`.rs`, `.toml`, etc.). Never
   skip them — even for "straightforward" CRUD or simple
   changes. The only exception is commits that contain no
   code at all (docs-only, config-only, project state
   only).

   **Agent A — Red Team** (security & correctness):

   > You are a red team reviewer. Analyze the code changes
   > for a Rust CLI project. Report issues in two
   > categories:
   >
   > **Correctness**: logic bugs, unhandled edge cases,
   > missing error handling, off-by-one errors, incorrect
   > assumptions, dead code, unclear semantics.
   >
   > **Security**: command injection, path traversal,
   > unsafe deserialization, unvalidated input, TOCTOU
   > races, information leaks, denial of service vectors.
   >
   > Be adversarial — assume the code is wrong and try to
   > prove it. Only report real, actionable issues with
   > specific line references. Do NOT report style nits,
   > missing docs, or hypothetical concerns. If you find
   > nothing, say "No issues found."
   >
   > For each finding, include:
   > 1. **What**: the specific issue with file:line ref
   > 2. **Why it matters**: concrete impact (data loss,
   >    panic, wrong output, security hole, etc.)
   > 3. **Example trigger**: a specific input, state, or
   >    sequence that demonstrates the problem
   > 4. **Suggested fix**: how to resolve it

   **Agent B — Artisan** (code quality & craftsmanship):

   > You are the Artisan — a code quality reviewer for a
   > Rust CLI project. You focus on craftsmanship beyond
   > what clippy catches. Analyze the code changes and
   > report issues in these categories:
   >
   > **Error Handling & Messages**: error types missing
   > Display, capitalized/punctuated error messages,
   > error chains leaking library types, Debug shown to
   > users instead of Display.
   >
   > **API Design**: functions accepting concrete types
   > instead of trait bounds, inconsistent parameter
   > patterns (some borrow, similar ones own), ownership
   > semantics unclear, missing builder patterns for
   > complex initialization.
   >
   > **Abstraction Boundaries**: public modules exposing
   > internal types, dependency types leaked in public
   > APIs, inconsistent abstraction levels within the
   > same module, business logic in the binary instead
   > of the library.
   >
   > **Type Safety**: missing Display/Debug on public
   > types, stringly-typed APIs where enums/newtypes
   > would be safer, unnecessary clones or allocations.
   >
   > Only report real, actionable issues with specific
   > line references. Do NOT duplicate clippy warnings
   > or red team security findings. Do NOT report
   > missing doc comments (CLAUDE.md already requires
   > them). If you find nothing, say "No issues found."
   >
   > For each finding, include:
   > 1. **What**: the specific issue with file:line ref
   > 2. **Why it matters**: concrete impact on
   >    maintainability, usability, or correctness
   > 3. **Better approach**: specific code change or
   >    pattern to use instead
   >
   > Reference the Rust API Guidelines
   > (rust-lang.github.io/api-guidelines/) where
   > applicable.

   Pass the full `git diff` output to both agents and
   tell them to read the relevant source files.

   **Presenting findings to the user:**
   - Present each finding with the **same level of
     detail** that goes into the log files. For each
     finding show:
     - **ID and title** (e.g. RT-023 or AQ-001)
     - **Source**: Red Team or Artisan
     - **Category**
     - **Description**: enough detail to understand
       without reading the code
     - **Impact / Why it matters**
     - **Example trigger** (red team) or **Better
       approach** (artisan)
     - **Suggested fix**
   - Do NOT summarize findings into a compact table
     with one-liner descriptions — the user needs the
     full context to decide
   - Ask whether to fix them before committing, commit
     anyway, or abort
   - Wait for the user's answer before proceeding

   **If no issues found by either agent:** continue.

   **Findings logs:**

   Red team findings use two files:
   - `docs/developer/redteam-log.md` — open (RT-NNN)
   - `docs/developer/redteam-resolved.md` — fixed

   Artisan findings use two files:
   - `docs/developer/artisan-log.md` — open (AQ-NNN)
   - `docs/developer/artisan-resolved.md` — fixed

   Both pairs are in **reverse chronological order**
   (newest first). New entries go right after the `---`
   separator.

   After the review:
   - Read each log to get the next ID (noted in the
     "Next ID" field at the top of each open log)
   - For each **new** finding, insert at the **top**
     of the relevant open log (right after `---`)
     with the next ID, date, commit context, full
     description, and category. Increment "Next ID".
   - For findings the user chose to **fix**, remove
     from the open log and insert at the **top** of
     the resolved log with the fix date and resolution
   - Include all changed log files in staged files
   - **Threshold warning:** if 10 or more findings
     are open in either log, tell the user that a
     comprehensive full-codebase review is needed
     before continuing feature work

7. **Fix line endings** - After staging, check for CRLF
   warnings. If `git add` produces any "LF will be replaced
   by CRLF" or "CRLF will be replaced by LF" warnings, fix
   the offending files before committing:
   - Run `dos2unix <file>` (or equivalent) to convert to LF
   - Re-stage the fixed files
   - All text files in this repo must use LF line endings

8. **Stage files** - Add specific files by name (avoid
   `git add -A` or `git add .`). Never commit sensitive files
   (.env, credentials, etc.). Include `docs/developer/DIARY.md`
   if it was updated.

9. **Commit** using this exact format (use HEREDOC for proper
   formatting):

```bash
git commit -m "$(cat <<'EOF'
<type>: <subject>

<body>

AI-Generated: Claude Code (<ModelName> <YYYY-MM-DD>)
EOF
)"
```

## Rules

- DO NOT include "Co-Authored-By" lines
- DO NOT include "Generated with [Claude Code]" lines
- Use the AI-Generated footer format shown above
- If no changes to commit, inform the user
- If changes look incomplete or risky, ask before committing

## Commit Types

- `feat`: New feature (minor version bump)
- `fix`: Bug fix (patch version bump)
- `perf`: Performance improvement (patch version bump)
- `chore`: Maintenance, tooling, dependencies (no bump)
- `refactor`: Code restructuring without behavior change (no
  bump)
- `docs`: Documentation only (no bump)
- `test`: Adding or updating tests (no bump)
- `style`: Formatting, whitespace (no code change) (no bump)

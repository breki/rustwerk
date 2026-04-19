---
description: Sync upstream template changes into this project
allowed-tools: Bash(git remote:*), Bash(git fetch:*), Bash(git log:*), Bash(git diff:*), Bash(git show:*), Bash(git rev-parse:*), Bash(git checkout:*), Bash(git status:*), Bash(cargo xtask validate*), Read, Edit, Write, Grep, Glob, AskUserQuestion
---

Sync changes from the upstream rustbase template into
this project.

## Instructions

1. **Read sync state** -- Read `.template-sync.toml` in
   the project root. If it does not exist, run the
   bootstrap flow (see below).

2. **Check preconditions**:
   - Run `git status` -- abort if there are uncommitted
     changes. Tell the user to commit or stash first.
   - Check if `origin` URL contains `breki/rustbase`.
     If so, this IS the template repo -- inform the user
     and offer only to update `.template-sync.toml` to
     the current HEAD (mark as synced).

3. **Fetch upstream**:
   - Read the `repo` URL from `.template-sync.toml`.
   - Check if a `template` remote exists
     (`git remote get-url template`). If not, add it:
     `git remote add template <repo URL>`
   - Run `git fetch template main`

4. **Compare versions**:
   - Get the `last-synced` SHA from `.template-sync.toml`
   - Get the template HEAD:
     `git rev-parse template/main`
   - If they match, report "Already up to date" and stop

5. **Analyze changes**:
   - Run `git log --oneline <last-synced>..template/main`
   - Run `git diff --stat <last-synced>..template/main`
   - Run `git diff <last-synced>..template/main`
   - Categorize each changed file:
     - **Infrastructure**: CI, xtask, build.ps1,
       scripts/, .github/, rust-toolchain.toml,
       rustfmt.toml
     - **Claude config**: CLAUDE.md, .claude/
     - **Docs**: docs/, README.md, llms.txt,
       CHANGELOG.md
     - **Boilerplate**: sample code in crates/,
       frontend/, e2e/
     - **Project config**: root Cargo.toml, .gitignore,
       .editorconfig
   - Present a summary table to the user:
     file | category | description | recommendation
   - Recommendation is one of:
     - **apply** -- safe, universally useful
     - **review** -- likely useful but needs inspection
     - **skip** -- boilerplate unlikely to apply
   - rustwerk is CLI-only: treat template changes under
     `frontend/`, `e2e/`, `crates/rustbase-web/`,
     `scripts/e2e*`, and `.mise.toml` as **skip** by
     default.

6. **Ask the user** which changes to apply. Accept:
   - "all" -- apply everything recommended
   - Category names -- apply all in that category
   - Specific file paths -- apply only those files
   - "none" -- skip all, just update sync marker

7. **Apply changes** for each selected file:
   - Read the template diff for that file:
     `git diff <last-synced>..template/main -- <file>`
   - Read the project's current version of the file
   - If the file is **unchanged in the project** since
     the template base: apply the template version
     directly via Edit or Write
   - If the file has **local modifications**: read both
     the template diff and the local file, then
     intelligently merge the template changes while
     preserving project customizations. Explain each
     conflict or adaptation to the user.
   - If the file is **new in the template**: add it
   - If the file was **deleted in the template**: ask
     the user whether to remove it
   - If the file uses `rustbase` naming that the project
     has renamed: the project's crate is named
     `rustwerk` -- adapt template references
     accordingly when applying them.
   - Use Edit to apply changes (never overwrite whole
     files blindly)

8. **Validate** -- Run `cargo xtask validate` to check
   that applied changes don't break the build. If
   validation fails, help the user fix issues before
   proceeding.

9. **Update sync marker** -- Edit `.template-sync.toml`:
   - Set `last-synced` to `template/main` HEAD SHA
   - Set `last-synced-version` to the version from the
     template's `crates/rustbase/Cargo.toml` at that SHA
     (use `git show template/main:crates/rustbase/Cargo.toml`
     to read it)

10. **Summary** -- Show:
    - Files applied
    - Files skipped
    - Previous sync version -> new sync version
    - Remind the user to review changes and commit
      with `/commit`

## Bootstrap Flow

When `.template-sync.toml` does not exist:

1. Inform the user this is first-time template sync
   setup.

2. Add the `template` remote:
   `git remote add template https://github.com/breki/rustbase.git`

3. Fetch: `git fetch template main`

4. Show `git log --oneline template/main` and ask the
   user which commit their project was created from.
   Offer options:
   - Pick a specific commit SHA from the list
   - Use "latest" to start tracking from now (skip
     retroactive sync, only get future changes)

5. Create `.template-sync.toml` with the chosen commit
   as both `created-from` and `last-synced`. Read the
   template version from that commit.

6. Proceed to step 4 of the main flow.

## Rules

- NEVER force-push or rewrite history
- NEVER auto-commit -- leave changes for the user to
  review and commit via `/commit`
- NEVER apply changes without user confirmation
- Always preserve project-specific customizations when
  merging
- Adapt `rustbase` references to `rustwerk` when
  applying template changes
- All text files must use LF line endings

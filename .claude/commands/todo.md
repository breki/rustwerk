---
description: Process the next pending TODO item from TODO.md
allowed-tools: Bash(target/debug/rustwerk*), Bash(cargo xtask*), Bash(cargo build*), Bash(mkdir*), Bash(wc*), Read, Edit, Write, Glob, Grep, Agent, AskUserQuestion, EnterPlanMode, ExitPlanMode
---

Process the next pending TODO item from `TODO.md`.

## Instructions

1. Read `TODO.md` and identify the first pending item
   (items under "Done" are already completed).

2. If the item is ambiguous or has multiple possible
   approaches, use AskUserQuestion to clarify before
   starting work.

3. Implement the item. Follow all project rules from
   CLAUDE.md (TDD, DDD, etc.).

4. Run the acceptance checks:
   ```
   cargo xtask validate
   ```

5. Move the completed item from the pending list to
   the `## Done` section at the bottom of `TODO.md`.
   Add the completion date in parentheses, e.g.:
   ```
   ## Done
   - fix CLI help text (2026-04-03)
   ```

6. Run `/commit` to commit the changes.

## Notes

- This is for ad-hoc tasks not tracked in the WBS.
  For WBS tasks, use `/next-task` instead.
- If the TODO item should be a WBS task, tell the
  user and suggest using `/next-task`.

---
description: Pick and implement the next available WBS task
allowed-tools: Bash(target/debug/rustwerk*), Bash(cargo xtask*), Bash(cargo build*), Bash(mkdir*), Bash(wc*), Read, Edit, Write, Glob, Grep, Agent, AskUserQuestion, EnterPlanMode, ExitPlanMode
---

Pick the next task from the WBS, plan it, and implement it.

## CLI invocation

Use the pre-built binary directly:
```
target/debug/rustwerk <args>
```
If the binary is stale or missing, rebuild first:
```
cargo build -p rustwerk
```

## Instructions

1. **List available tasks** — Run:
   ```
   target/debug/rustwerk task list --available
   ```

2. **Let the user choose** — Use AskUserQuestion to present
   the available tasks and let the user pick one (or suggest
   the critical-path task marked with `*`). If only one task
   is available, confirm it with the user before proceeding.

3. **Mark in-progress** — Run:
   ```
   target/debug/rustwerk task status <ID> in-progress
   ```

4. **Read the task description** — Run:
   ```
   target/debug/rustwerk task describe <ID>
   ```
   If a description file exists (`.rustwerk/tasks/<ID>.md`),
   use it as the primary source for implementation details,
   acceptance criteria, and context. Also check
   `docs/planning/wbs.md` for additional context if needed.

5. **Plan if needed** — For tasks with complexity >= 5, or
   if the implementation approach is unclear, use
   EnterPlanMode to design the approach and get user
   approval. For simple tasks (complexity 1-3 with obvious
   implementation), skip planning and go straight to
   implementation.

6. **Implement** — Follow TDD:
   - Write failing tests first (red)
   - Implement until tests pass (green)
   - Refactor if needed
   - Run `cargo xtask validate` to confirm
   - Rebuild the binary: `cargo build -p rustwerk`

7. **Mark done** — Run:
   ```
   target/debug/rustwerk task status <ID> done
   ```

8. **Commit** — Use `/commit` to commit the changes.

## Rules

- One task per invocation. Don't chain multiple tasks.
- Always validate with `cargo xtask validate` before
  marking done.
- Rebuild with `cargo build -p rustwerk` after code
  changes before using the CLI for task management.
- If the task depends on other tasks, verify those
  dependencies are actually done before starting.
- If implementation reveals the task should be split,
  ask the user before proceeding.

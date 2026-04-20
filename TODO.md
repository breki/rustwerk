# TODO

- Can we use a proper TUI for walking the Gantt chart, so it would look
  something like Superfile or lazygit?
- Investigate whether JSON is really the best format for the project file - 
  where and how to store a more complex task descriptions?
- Fix env-var race in `plugin_host::tests::discovery_dirs_*` pair
  (`DEV_DIRS_ENV` leaks across parallel tests; blocks `cargo xtask
  validate` intermittently). Serialize the env-touching tests via a
  mutex or mark them `#[serial]`.

## Done

- Add --version flag to CLI (2026-04-07)
- Add batch commands dev.add and dev.remove (2026-04-04)
- Add project file format specification to docs (2026-04-04)
- Add GitHub Actions and config review to red team in /commit (2026-04-04)
- Red team baseline review of project config: 6 findings fixed (2026-04-04)
- Fix GitHub Actions CI and add release workflow (2026-04-04)
- Clean up CLAUDE.md: remove redundant sections, add skills table (2026-04-04)
- Add /rustwerk skill for CLI project management reference (2026-04-04)
- Add WBS tasks for tag support: DOM-TAG, CLI-TAG-SET, CLI-TAG-FILTER (2026-04-04)
- Add ON_HOLD status and mark Phase 5 git tasks on-hold (2026-04-03)
- Describe git tasks in detail; defer Phase 5 pending workflow design decisions (2026-04-03)
- Add --remaining flag to gantt command (2026-04-03)
- Red Gantt bars for critical path tasks (2026-04-03)
- Save TUI rendering lessons as /tui-expert skill (2026-04-03)
- Add module size check (>500 LOC) to Artisan reviewer (2026-04-03)
- Investigate Gantt rendering: surveyed 6 tools (andrew-ls/gantt, gantt-cli/ratatui, taskdog, Taskwarrior, TaskFalcon, Pla). Key findings: Unicode blocks (█▓▒░) for status, dual-char bars for uncertainty, terminal-width-aware scaling, ratatui for optional interactive mode. Full report in docs/research/gantt-rendering.md (2026-04-03)
- Add developer management tasks to WBS (DOM-DEV, CLI-DEV-ADD/REMOVE/LIST, DEV-ASSIGN) (2026-04-03)
- Add ANSI colors to Gantt chart with auto-detect and NO_COLOR support (2026-04-03)
- Fix --available to show TODO only, add --active for IN_PROGRESS (2026-04-03)
- Add Artisan code quality reviewer to /commit (2026-04-03)

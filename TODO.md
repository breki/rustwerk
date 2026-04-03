# TODO

- Can we use a proper TUI for walking the Gantt chart, so it would look
  something like Superfile or lazygit?
- Investigate whether JSON is really the best format for the project file - 
  where and how to store a more complex task descriptions?

## Done

- Add module size check (>500 LOC) to Artisan reviewer (2026-04-03)
- Investigate Gantt rendering: surveyed 6 tools (andrew-ls/gantt, gantt-cli/ratatui, taskdog, Taskwarrior, TaskFalcon, Pla). Key findings: Unicode blocks (█▓▒░) for status, dual-char bars for uncertainty, terminal-width-aware scaling, ratatui for optional interactive mode. Full report in docs/research/gantt-rendering.md (2026-04-03)
- Add developer management tasks to WBS (DOM-DEV, CLI-DEV-ADD/REMOVE/LIST, DEV-ASSIGN) (2026-04-03)
- Add ANSI colors to Gantt chart with auto-detect and NO_COLOR support (2026-04-03)
- Fix --available to show TODO only, add --active for IN_PROGRESS (2026-04-03)
- Add Artisan code quality reviewer to /commit (2026-04-03)

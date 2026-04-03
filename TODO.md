# TODO

- Regarding git-related tasks: describe to me in detail what each of these 
  tasks would try to accomplish. I'm still considering the proper approach
  with regard to git workflows - for example, what if the projec spans 
  multiple git repositories? Or if we want to support both git and non-git workflows?
- Can we use a proper TUI for walking the Gantt chart, so it would look
  something like Superfile or lazygit?
- Investigate whether JSON is really the best format for the project file - 
  where and how to store a more complex task descriptions?
- Add missing batch commands: dev.add, dev.remove (developer CRUD needed for
  batch project setup)

## Done

- Add --remaining flag to gantt command (2026-04-03)
- Red Gantt bars for critical path tasks (2026-04-03)
- Save TUI rendering lessons as /tui-expert skill (2026-04-03)
- Add module size check (>500 LOC) to Artisan reviewer (2026-04-03)
- Investigate Gantt rendering: surveyed 6 tools (andrew-ls/gantt, gantt-cli/ratatui, taskdog, Taskwarrior, TaskFalcon, Pla). Key findings: Unicode blocks (█▓▒░) for status, dual-char bars for uncertainty, terminal-width-aware scaling, ratatui for optional interactive mode. Full report in docs/research/gantt-rendering.md (2026-04-03)
- Add developer management tasks to WBS (DOM-DEV, CLI-DEV-ADD/REMOVE/LIST, DEV-ASSIGN) (2026-04-03)
- Add ANSI colors to Gantt chart with auto-detect and NO_COLOR support (2026-04-03)
- Fix --available to show TODO only, add --active for IN_PROGRESS (2026-04-03)
- Add Artisan code quality reviewer to /commit (2026-04-03)

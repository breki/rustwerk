# Terminal Gantt Chart Rendering — Research

Research into how existing tools render Gantt charts in
the terminal. Conducted 2026-04-03.

## Existing tools surveyed

### andrew-ls/gantt (Haskell)

Static output. Uses Unicode block characters:
- `█` (full block) for minimum estimate bars
- `▒` (medium shade) for maximum estimate (uncertainty)
- `•` (bullet) for timeline header/footer decoration

Example output:
```
             0d      1d      2d      3d
             ••••••••••••••••••••••••••
Design UX        █▒▒▒▒▒ 1-6
Implement        ████████████████████▒▒▒▒ 20-24
Test Live   ████ 4
             ••••••••••••••••••••••••••
```

Key ideas: **dual-character bars** (filled vs uncertain),
timeline ruler with bullet dots, task labels left-aligned.

### gantt-cli (Rust, ratatui)

Interactive TUI. Built with ratatui. Features:
- Scrollable timeline (left/right navigation)
- Hierarchical tasks with expand/collapse
- Dependency tracking with topological sort
- Urgency highlighting with color intensity
- Vim-style keyboard controls
- Undo/redo

Key ideas: **interactive scrolling** solves the
terminal-width problem. Color intensity for urgency.

### taskdog (Python)

TUI + CLI. Schedule optimization engine that
auto-generates daily schedules from deadlines and
priorities. Includes:
- 9 scheduling algorithms
- Gantt chart for workload analysis
- Task dependencies with cycle detection
- Time tracking (planned vs actual)
- AI integration via REST API + MCP

Key ideas: **AI-agent integration via MCP**, schedule
*optimization* (not just visualization), planned vs
actual comparison.

### Taskwarrior burndown

Static chart. Renders in terminal using:
- ANSI escape sequences for color
- Terminal-width auto-detection
- Three data series: pending, active, completed
- Completion date prediction from historical rate

Key ideas: **terminal-width-aware** rendering,
**predictive completion** from task velocity.

### TaskFalcon

Exports to PNG/SVG/PDF (not terminal). Uses Cairo
graphics library. Shows dependencies as connection
lines. Not relevant for terminal rendering but good
for HTML export ideas.

### Pla

Exports to PNG/EPS/PDF/SVG. Cairo-based. Hierarchical
task structure with color coding. Also not terminal
but good for graphical export.

## Rendering techniques

### Character palette

| Character | Use |
|-----------|-----|
| `█` (U+2588) | Filled bar (done) |
| `▓` (U+2593) | Dense shade (in-progress) |
| `▒` (U+2592) | Medium shade (uncertainty/float) |
| `░` (U+2591) | Light shade (todo/planned) |
| `▌` (U+258C) | Left half block (sub-char start) |
| `▐` (U+2590) | Right half block (sub-char end) |
| `─` `│` `┬` `┴` | Box-drawing for axis/grid |
| `◆` `▶` `◇` | Milestone markers |
| `•` | Timeline decoration |

### Color usage

- Green: completed tasks
- Yellow/bold: in-progress
- Red: blocked/overdue
- Dim: planned/future
- Cyan/bold: critical path
- Color intensity: urgency level (gantt-cli idea)

### Layout approaches

1. **Static two-column**: labels left, bars right.
   Simple, works with piping. Our current approach.

2. **Interactive TUI (ratatui)**: scrollable timeline,
   expandable tasks. Solves width problem but breaks
   piping and agent workflows.

3. **Terminal-width-aware**: detect terminal width,
   scale bars proportionally. Taskwarrior approach.

## Recommendations for rustwerk

### Short term (current)

Keep static output. Upgrade characters:
- `█` instead of `#` for done
- `▓` instead of `#.` for in-progress
- `░` instead of `.` for todo
- `▐` and `▌` for bar caps
- `─│┬` for the time axis

### Medium term

- Terminal-width-aware scaling (detect width, scale
  bars proportionally)
- Planned vs actual overlay (dual-character bars like
  andrew-ls/gantt)
- Milestone markers (`◆`) for key deliverables
- Color intensity for urgency/overdue

### Long term

- Optional `--interactive` mode using ratatui for
  scrollable, zoomable Gantt with keyboard navigation
- Keep static mode as default for agent workflows

## Sources

- [andrew-ls/gantt](https://github.com/andrew-ls/gantt)
- [gantt-cli (ratatui)](https://github.com/zhangjinshui-nerveee/gantt-cli)
- [taskdog](https://github.com/Kohei-Wada/taskdog)
- [Taskwarrior burndown](https://taskwarrior.org/docs/commands/burndown/)
- [TaskFalcon](https://taskfalcon.org/doc/command_line/index.html)
- [Pla](https://www.arpalert.org/pla.html)
- [plotext (Python)](https://github.com/piccolomo/plotext)
- [ratatui](https://ratatui.rs/)

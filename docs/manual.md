# RustWerk User Manual

RustWerk is a git-native, AI-agent-friendly project
orchestration CLI. It manages tasks, dependencies,
effort tracking, and schedule visualization from the
command line.

## Why RustWerk?

Most project management tools live in the cloud — web
dashboards that AI agents can't easily interact with
and that drift out of sync with the actual codebase.
RustWerk takes a different approach:

- **Git-native.** All project state lives in a single
  JSON file inside your repository. Task status,
  dependencies, and effort data travel with the code
  and are versioned alongside it.
- **AI-agent-friendly.** Structured JSON I/O and batch
  commands let AI coding agents read and update project
  state programmatically, without scraping web UIs.
- **Single binary, zero config.** One portable Rust
  binary. No database, no server, no accounts. Run it
  in any directory with a `.rustwerk/` folder.
- **Developer-first.** Built for developers who live in
  the terminal. ASCII Gantt charts, critical path
  analysis, and effort tracking — all from the command
  line.

RustWerk is designed for small-to-medium projects where
the overhead of a full project management suite is not
justified, but you still want dependency-aware scheduling,
critical path visibility, and a structured workflow.

## Table of Contents

- [Getting Started](#getting-started)
- [Sample Project Walkthrough](#sample-project-walkthrough)
- [Task Management](#task-management)
  - [Add a Task](#add-a-task)
  - [List Tasks](#list-tasks)
  - [Update Task Status](#update-task-status)
  - [Update a Task](#update-a-task)
  - [Remove a Task](#remove-a-task)
  - [Assign a Task](#assign-a-task)
- [Developer Management](#developer-management)
- [Dependencies](#dependencies)
- [Effort Tracking](#effort-tracking)
- [Gantt Chart](#gantt-chart)
  - [View Schedule](#view-schedule)
  - [Show Remaining Work Only](#show-remaining-work-only)
  - [Color Behavior](#color-behavior)
- [Reports](#reports)
- [Batch Commands](#batch-commands)
- [Project File](#project-file)

## Getting Started

### Initialize a Project

Create a new project in the current directory:

```
rustwerk init "My Project"
```

This creates a `.rustwerk/` directory with a
`project.json` file. All project state lives in this
single file.

### View Project Summary

```
rustwerk show
```

Displays task counts by status, completion percentage,
complexity totals, and effort summaries.

## Sample Project Walkthrough

Here is a complete example showing a small web
application project:

```
$ rustwerk init "WebApp"
Initialized project: WebApp

$ rustwerk task add "Design database schema" \
    --id DB-SCHEMA --complexity 5 --effort 2D
$ rustwerk task add "Build REST API" \
    --id API-BUILD --complexity 8 --effort 3D
$ rustwerk task add "Create frontend" \
    --id UI-BUILD --complexity 5 --effort 2D
$ rustwerk task add "Write tests" \
    --id TEST-ALL --complexity 3 --effort 1D
$ rustwerk task add "Deploy to staging" \
    --id DEPLOY --complexity 2 --effort 4H

$ rustwerk task depend API-BUILD DB-SCHEMA
$ rustwerk task depend UI-BUILD API-BUILD
$ rustwerk task depend TEST-ALL API-BUILD
$ rustwerk task depend TEST-ALL UI-BUILD
$ rustwerk task depend DEPLOY TEST-ALL
```

After completing the schema and starting the API:

```
$ rustwerk show
Project: WebApp

Tasks:    5 total  (1 done, 1 in-progress,
          3 todo, 0 blocked)
Complete: 20%
Complexity: 23 total
Effort:   68.0H estimated, 0.0H actual
Created:  2026-04-03 14:57 UTC
```

Task list with status and critical path markers:

```
$ rustwerk task list
 *API-BUILD IN_PROGRESS Build REST API [8]
  DB-SCHEMA DONE        Design database schema [5]
 *DEPLOY    TODO        Deploy to staging [2]
 *TEST-ALL  TODO        Write tests [3]
 *UI-BUILD  TODO        Create frontend [5]
```

Full Gantt chart showing the schedule:

```
$ rustwerk gantt
           0              5              10             15             20
           ┬──────────────┬──────────────┬──────────────┬──────────────┬───
*DB-SCHEMA ▐█████████████▌
*API-BUILD                ▐▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░░▌
*UI-BUILD                                       ▐░░░░░░░░░░░░░▌
*TEST-ALL                                                      ▐░░░░░░░▌
*DEPLOY                                                                 ▐░░░▌
```

Remaining work only (done tasks removed, timeline
rescheduled):

```
$ rustwerk gantt --remaining
           0                  5                  10                 15
           ┬──────────────────┬──────────────────┬──────────────────┬─────
*API-BUILD ▐▓▓▓▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░░░░░▌
*UI-BUILD                               ▐░░░░░░░░░░░░░░░░░▌
*TEST-ALL                                                  ▐░░░░░░░░░▌
*DEPLOY                                                               ▐░░░░░▌
```

Bar legend:
- `█` done — `▓` in-progress — `▒` blocked — `░` todo
- `*` marks tasks on the critical path (rendered in
  red with color enabled)

## Task Management

### Add a Task

```
rustwerk task add "Implement login" --id AUTH-LOGIN
```

Options:
- `--id <ID>` — Mnemonic task ID (uppercase letters,
  digits, hyphens). Auto-generated as T0001, T0002, ...
  if omitted.
- `--desc <DESC>` — Optional description.
- `--complexity <N>` — Complexity score (e.g. Fibonacci:
  1, 2, 3, 5, 8, 13). Used for scheduling and critical
  path calculation.
- `--effort <AMOUNT>` — Effort estimate (e.g. `5H`,
  `1D`, `2W`).

Examples:

```
rustwerk task add "Design API" --id API-DESIGN \
  --complexity 5 --effort 2D --desc "REST API design"

rustwerk task add "Quick fix"
# Auto-generates ID: T0001
```

### List Tasks

```
rustwerk task list
```

Shows all tasks with status, title, and complexity.
Critical path tasks are marked with `*`.

Filters:
- `--available` — Show only tasks ready to start (all
  dependencies done, task is TODO).
- `--active` — Show only tasks currently in progress.

```
rustwerk task list --available
rustwerk task list --active
```

### Update Task Status

Tasks follow a workflow with valid transitions:

```
TODO → IN_PROGRESS → DONE
                   → BLOCKED → IN_PROGRESS
                             → TODO
```

Change status:

```
rustwerk task status AUTH-LOGIN in-progress
rustwerk task status AUTH-LOGIN done
```

Status values: `todo`, `in-progress` (also accepts
`in_progress`, `inprogress`), `blocked`, `done`.

Use `--force` to bypass transition validation:

```
rustwerk task status AUTH-LOGIN todo --force
```

### Update a Task

```
rustwerk task update AUTH-LOGIN --title "New title"
rustwerk task update AUTH-LOGIN --desc "New description"
rustwerk task update AUTH-LOGIN --desc ""  # clear desc
```

### Remove a Task

```
rustwerk task remove AUTH-LOGIN
```

Fails if other tasks depend on this one. Remove
dependencies first.

### Assign a Task

Assign a registered developer to a task:

```
rustwerk task assign AUTH-LOGIN alice
```

Remove the assignment:

```
rustwerk task unassign AUTH-LOGIN
```

Note: the developer must be registered in the project
(developer management is a planned feature).

## Developer Management

### Add a Developer

```
rustwerk dev add alice "Alice Smith" \
  --email alice@example.com --role lead
```

Options:
- `--email <EMAIL>` — Email address.
- `--role <ROLE>` — Role on the project (e.g. "lead",
  "developer").

### Remove a Developer

```
rustwerk dev remove alice
```

Fails if any task is assigned to this developer. Unassign
them first.

### List Developers

```
rustwerk dev list
```

Shows all registered developers with their name, email,
and role:

```
  alice  Alice Smith <alice@example.com> (lead)
  bob    Bob Jones
```

## Dependencies

### Add a Dependency

```
rustwerk task depend FRONTEND BACKEND
```

This means FRONTEND depends on BACKEND — BACKEND must
be completed before FRONTEND can start.

Circular dependencies are detected and rejected.
Duplicate edges are silently ignored (idempotent).

### Remove a Dependency

```
rustwerk task undepend FRONTEND BACKEND
```

## Effort Tracking

### Set an Estimate

```
rustwerk effort estimate AUTH-LOGIN 8H
```

Units: `H` (hours), `D` (days = 8H), `W` (weeks = 40H),
`M` (months = 160H). Fractional values are supported
(e.g. `2.5H`, `0.5D`).

### Log Actual Effort

```
rustwerk effort log AUTH-LOGIN 3H --dev alice
rustwerk effort log AUTH-LOGIN 1.5H --dev alice \
  --note "debugging auth flow"
```

The task must be IN_PROGRESS to log effort.

## Gantt Chart

### View Schedule

```
rustwerk gantt
```

Displays an ASCII Gantt chart with:
- Unicode block bars: `█` (done), `▓` (in-progress),
  `▒` (blocked), `░` (todo)
- Half-block caps: `▐` (left), `▌` (right)
- Box-drawing axis: `─` with `┬` tick marks
- Color coding: green (done), yellow (in-progress),
  red (blocked/critical path), dim (todo)
- Critical path tasks marked with `*` and rendered
  in red
- Terminal-width-aware scaling

### Show Remaining Work Only

```
rustwerk gantt --remaining
```

Filters out done tasks and reschedules the chart so
tasks whose dependencies are complete start at time 0.
The critical path is recalculated for remaining work
only.

### Color Behavior

Colors are enabled automatically when stdout is a
terminal. They are disabled when output is piped or
when the `NO_COLOR` environment variable is set:

```
NO_COLOR=1 rustwerk gantt
rustwerk gantt | less
```

## Reports

### Completion Summary

```
rustwerk report complete
```

Displays a PM-friendly completion summary:

```
Completion Report: WebApp
========================================

Status Breakdown
  Done:          1
  In Progress:   1
  Blocked:       0
  Todo:          3
  Total:         5

Completion: [======>                        ] 20%

Effort
  Estimated:   68.0H
  Actual:      0.0H
  Burn rate:   0%

Complexity:    23 total

Critical Path: 4 tasks, 18 complexity
  DB-SCHEMA → API-BUILD → UI-BUILD → TEST-ALL
```

Includes status breakdown, a visual progress bar,
estimated vs actual effort with burn rate, complexity
totals, and the remaining critical path with task IDs.

### Effort by Developer

```
rustwerk report effort
```

Shows effort breakdown per developer with hours and
percentage:

```
Effort by Developer
========================================
  alice                  4.5H (69%)
  bob                    2.0H (31%)
----------------------------------------
  Total                  6.5H
```

### Bottleneck Report

```
rustwerk report bottlenecks
```

Shows tasks that block the most downstream work, sorted
by impact. Each entry includes the task ID, number of
transitively blocked tasks, state, and assignee:

```
Bottleneck Report
  ID      Blocks  State         Assignee
------------------------------------------
  CORE         5  ready         alice
  AUTH         2  in progress   bob
  DB           1  blocked       (unassigned)
```

The "State" column combines status and readiness:
- **ready** — all dependencies done, can be started
- **in progress** — already being worked on
- **blocked** — waiting on upstream dependencies

Only non-done tasks are included. Tasks with no
downstream dependents are omitted.

## Batch Commands

Execute multiple commands atomically from a JSON file
or stdin. All commands succeed or none are applied.

### From a File

```
rustwerk batch --file commands.json
```

### From stdin

```
echo '[
  {"command": "task.add", "args": {"title": "A", "id": "A"}},
  {"command": "task.add", "args": {"title": "B", "id": "B"}},
  {"command": "task.depend", "args": {"from": "B", "to": "A"}}
]' | rustwerk batch
```

### Available Batch Commands

| Command | Required Args | Optional Args |
|---------|--------------|---------------|
| `task.add` | `title` | `id`, `desc`, `complexity`, `effort` |
| `task.remove` | `id` | |
| `task.update` | `id` | `title`, `desc` |
| `task.status` | `id`, `status` | `force` (bool) |
| `task.assign` | `id`, `to` | |
| `task.unassign` | `id` | |
| `task.depend` | `from`, `to` | |
| `task.undepend` | `from`, `to` | |
| `effort.log` | `id`, `amount`, `dev` | `note` |
| `effort.estimate` | `id`, `amount` | |

### Error Handling

If any command fails, execution stops immediately. The
project is NOT saved — no partial changes are applied.
A JSON error object is printed to stderr:

```json
{
  "error": "command 2 (task.status) failed: ...",
  "applied": 2
}
```

### Limits

- Maximum input size: 10 MB
- Maximum commands per batch: 1,000

## Project File

All state is stored in `.rustwerk/project.json`. This
is a plain JSON file that can be committed to git.

The file contains:
- Project metadata (name, timestamps)
- Tasks with status, dependencies, complexity, effort
- Developer registry

RustWerk looks for the `.rustwerk/` directory starting
from the current directory and walking up the directory
tree, similar to how git finds `.git/`.

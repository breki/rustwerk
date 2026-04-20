# RustWerk User Manual

RustWerk is a git-native, AI-agent-friendly project
orchestration CLI. It manages tasks, dependencies,
effort tracking, and schedule visualization from the
command line.

## Table of Contents

- [Why RustWerk?](#why-rustwerk)
- [Typical Workflow](#typical-workflow)
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
- [Project Status Dashboard](#project-status-dashboard)
- [Dependency Tree](#dependency-tree)
- [Gantt Chart](#gantt-chart)
  - [View Schedule](#view-schedule)
  - [Show Remaining Work Only](#show-remaining-work-only)
  - [Color Behavior](#color-behavior)
- [Reports](#reports)
- [Batch Commands](#batch-commands)
- [Plugins](#plugins)
- [Project File](#project-file)
- [Project File Specification](project-file-spec.md)

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

## Typical Workflow

RustWerk is designed to work hand-in-hand with an AI
coding agent. A typical workflow has two phases:

### 1. Project Setup (AI-Driven)

You describe a project or a large epic to your AI agent
and ask it to produce a Work Breakdown Structure (WBS).
The agent creates a structured plan with tasks,
dependencies, complexity estimates, and effort budgets.
It then uses the RustWerk CLI — typically via `batch`
commands — to populate the project in one shot:

```
rustwerk init "My Project"
rustwerk batch --file wbs.json
```

Because RustWerk speaks JSON natively, the agent can
generate the entire project definition without manual
data entry. The result is a fully wired dependency
graph with effort estimates, ready to execute.

### 2. Daily Development (Developer-Driven)

Once the project is set up, you use RustWerk as part of
your daily development loop:

1. **Pick a task** — find what's ready to work on:
   ```
   rustwerk task list --available
   ```
2. **Assign it** — claim the task (uses `RUSTWERK_USER`
   if set):
   ```
   rustwerk task assign AUTH-LOGIN
   ```
3. **Start work** — mark the task in progress:
   ```
   rustwerk task status AUTH-LOGIN in-progress
   ```
4. **Log effort** — track time as you go (uses
   `RUSTWERK_USER` if set):
   ```
   rustwerk effort log AUTH-LOGIN 2H
   ```
5. **Complete the task** — mark it done:
   ```
   rustwerk task status AUTH-LOGIN done
   ```
6. **Check progress** — review the dashboard and Gantt:
   ```
   rustwerk status
   rustwerk gantt --remaining
   ```

The project file is committed alongside your code, so
task state stays in sync with the codebase and is
visible in pull request diffs.

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
- `--tags <TAGS>` — Comma-separated tags for
  categorization. Tags are slug-like (lowercase
  alphanumeric + hyphens, max 50 chars each, max 20
  per task).

Examples:

```
rustwerk task add "Design API" --id API-DESIGN \
  --complexity 5 --effort 2D --desc "REST API design" \
  --tags "backend,api"

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
- `--status <STATUS>` — Filter by status (todo,
  in-progress, blocked, done, on-hold).
- `--assignee <ID>` — Filter by assigned developer.
- `--chain <TASK>` — Show a task and all its transitive
  dependencies.
- `--tag <TAG>` — Filter by tag (show only tasks with
  this tag).

Filters can be combined:

```
rustwerk task list --available
rustwerk task list --active
rustwerk task list --status todo
rustwerk task list --assignee alice
rustwerk task list --chain DEPLOY
rustwerk task list --tag backend
rustwerk task list --assignee alice --status in-progress
rustwerk task list --tag backend --status todo
```

### Update Task Status

Tasks follow a workflow with valid transitions:

```
TODO → IN_PROGRESS → DONE
     → ON_HOLD     → BLOCKED → IN_PROGRESS
                              → TODO
IN_PROGRESS → ON_HOLD → TODO
                      → IN_PROGRESS
```

Change status:

```
rustwerk task status AUTH-LOGIN in-progress
rustwerk task status AUTH-LOGIN done
rustwerk task status AUTH-LOGIN on-hold
```

Status values: `todo`, `in-progress` (also accepts
`in_progress`, `inprogress`), `blocked`, `done`,
`on-hold` (also accepts `on_hold`, `onhold`).

Use `--force` to bypass transition validation:

```
rustwerk task status AUTH-LOGIN todo --force
```

### Update a Task

```
rustwerk task update AUTH-LOGIN --title "New title"
rustwerk task update AUTH-LOGIN --desc "New description"
rustwerk task update AUTH-LOGIN --desc ""  # clear desc
rustwerk task update AUTH-LOGIN --tags "backend,auth"
rustwerk task update AUTH-LOGIN --tags ""  # clear tags
```

### View Task Description

Show the detailed description file for a task:

```
rustwerk task describe AUTH-LOGIN
```

Description files are stored at
`.rustwerk/tasks/<ID>.md`. If no file exists, the
command prints the expected path so you can create one.

### Rename a Task

Change a task's ID. Updates all dependency references
across other tasks and renames the description file at
`.rustwerk/tasks/<ID>.md` if it exists:

```
rustwerk task rename AUTH-LOGIN AUTH-SIGNIN
```

All embedded task data (status, effort log, tags,
assignee) is preserved. Fails if the new ID is already
in use. Task ID references in free-form text (effort
notes, description bodies) are not rewritten.

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

If the `RUSTWERK_USER` environment variable is set, the
developer argument can be omitted:

```
export RUSTWERK_USER=alice
rustwerk task assign AUTH-LOGIN
```

Remove the assignment:

```
rustwerk task unassign AUTH-LOGIN
```

The developer must be registered in the project (see
[Developer Management](#developer-management)).

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

If `RUSTWERK_USER` is set, `--dev` can be omitted:

```
rustwerk effort log AUTH-LOGIN 3H
```

The task must be IN_PROGRESS to log effort.

## Project Status Dashboard

```
rustwerk status
```

Shows a compact project status overview:

```
MyProject
[█████████████░░░░░░░] 65%

  done            45
  in-progress      1
  todo            12
  on-hold         11
  total           69

Active:
  AUTH-LOGIN  (alice)

3 bottlenecks
Critical path: 5 tasks, 21 complexity
```

Includes completion bar, task counts by status, active
tasks with assignees, bottleneck count, and remaining
critical path length.

## Dependency Tree

### View Tree

```
rustwerk tree
```

Shows the full dependency DAG as an ASCII tree with
status indicators:

```
RustWerk
├── [✓] FILE-SCHEMA
│   ├── [✓] DOM-PROJ
│   │   └── [✓] SER-JSON
│   └── [✓] DOM-TASK
│       └── [✓] SER-JSON → (see above)
└── [ ] GIT-CTX
    └── [~] GIT-SWITCH
```

Status indicators: `✓`=Done, `>`=InProgress,
`!`=Blocked, `~`=OnHold, ` `=Todo.

Tasks with multiple parents appear expanded under the
first parent and as a reference (`→ see above`) under
subsequent parents.

### Remaining Work

```
rustwerk tree --remaining
```

Excludes Done and OnHold tasks. Tasks whose
dependencies are all complete become new roots.

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

## JSON Output (`--json`)

Every command accepts a global `--json` flag. When set,
the command emits a single pretty-printed JSON object
to stdout instead of human-readable text. The flag can
appear before or after the subcommand:

```
rustwerk --json task list
rustwerk task list --json
```

JSON is intended for scripts and AI agents. It is the
stable, machine-readable contract; human text may
change. The exact shape per command is documented in
`llms.txt`. A few examples:

```
$ rustwerk task add "Title" --id T1 --json
{
  "id": "T1",
  "title": "Title"
}

$ rustwerk task list --json
{
  "tasks": [
    {
      "id": "T1",
      "title": "Title",
      "status": "todo",
      "critical": false,
      "tags": []
    }
  ]
}
```

Errors stay human-readable on stderr (structured error
output is a separate feature). `batch` already emits
JSON natively, so `--json` is a no-op there.

## Batch Commands

Execute multiple commands atomically from a JSON file
or stdin. All commands succeed or none are applied.

Batch commands are deterministic: all arguments must be
explicit in the JSON input. Environment variable
defaults (such as `RUSTWERK_USER`) do not apply.

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
| `task.add` | `title` | `id`, `desc`, `complexity`, `effort`, `tags` (array) |
| `task.remove` | `id` | |
| `task.update` | `id` | `title`, `desc`, `tags` (array) |
| `task.rename` | `old_id`, `new_id` | |
| `task.status` | `id`, `status` | `force` (bool) |
| `task.assign` | `id`, `to` | |
| `task.unassign` | `id` | |
| `task.depend` | `from`, `to` | |
| `task.undepend` | `from`, `to` | |
| `effort.log` | `id`, `amount`, `dev` | `note` |
| `effort.estimate` | `id`, `amount` | |
| `dev.add` | `id`, `name` | `email`, `role` |
| `dev.remove` | `id` | |

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

## Plugins

RustWerk supports optional plugins (dynamic libraries)
for integrating with external systems like Jira.
Plugins are discovered from
`.rustwerk/plugins/` (project-scoped) and
`~/.rustwerk/plugins/` (user-scoped).

### List Installed Plugins

```bash
rustwerk plugin list
```

Shows each plugin's name, version, description,
capabilities, and path on disk. Reports
"No plugins installed." when no plugins are found (not
an error).

### Install a Plugin

```bash
rustwerk plugin install <SOURCE> \
    [--scope <project|user>] \
    [--force]
```

Copies a pre-built cdylib into the plugin discovery
directory and verifies it loads.

- `<SOURCE>`: path to a built `.dll` (Windows), `.so`
  (Linux), or `.dylib` (macOS) — typically
  `target/debug/rustwerk_jira_plugin.dll` after running
  `cargo build -p rustwerk-jira-plugin`.
- `--scope project` (default): install into
  `./.rustwerk/plugins/`.
- `--scope user`: install into
  `$HOME/.rustwerk/plugins/` (or
  `%USERPROFILE%\.rustwerk\plugins\`) so the plugin is
  available to every project.
- `--force`: overwrite an existing plugin with the
  same filename. Without `--force`, an existing
  install is a clear error.

On success, the command prints the discovered
`name/version/capabilities` — the same thing
`plugin list` would show next. If the copy succeeds
but the library fails to load (wrong API version,
missing FFI symbols, not a valid dynamic library),
the partial install is removed automatically.

`rustwerk plugin install` only installs already-built
cdylibs. Building from source (e.g. `--from
<crate-name>`) is intentionally deferred to a
follow-up subcommand; use
`cargo build -p <plugin-crate>` followed by
`rustwerk plugin install <path>`.

### Push Tasks to a Plugin Backend

```bash
rustwerk plugin push <NAME> \
    [--project-key <KEY>] \
    [--tasks <ID,ID,...>] \
    [--dry-run]
```

- `<NAME>`: plugin name as shown by `plugin list`
  (e.g. `jira`).
- `--project-key`: external project key; passed
  through to the plugin (e.g. the Jira project key).
- `--tasks`: comma-separated task IDs. If omitted,
  every task in the project is sent.
- `--dry-run`: print the task list and which config
  keys resolved without actually invoking the plugin.
  Useful for confirming your environment before a
  real push.

The host assembles a JSON config from three sources
and hands it to the plugin — the plugin picks what it
needs:

- `jira_url` from the `JIRA_URL` environment variable
- `jira_token` from the `JIRA_TOKEN` environment
  variable
- `username` from `git config user.email`
- `project_key` from `--project-key`

Absent keys are omitted entirely. `--dry-run` prints
only the **key names** present — never the token
value.

On failure (`result.success == false`) the process
exits non-zero so the command composes with CI.

### Feature Flag

The `plugins` feature is enabled by default. To build
a plugin-free binary:

```bash
cargo build --no-default-features
```

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

For the full file format specification — including every
field, type, validation rule, and example — see the
[Project File Specification](project-file-spec.md).

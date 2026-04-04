---
name: rustwerk
description: >
  RustWerk CLI reference for project management. Use
  when querying, updating, or managing tasks, effort,
  dependencies, developers, or project status via the
  rustwerk CLI.
---

# RustWerk CLI Reference

Use the pre-built binary directly:

```
target/debug/rustwerk <command> [args]
```

If the binary is missing or stale, build first:

```
cargo build
```

## Commands at a Glance

| Command | Purpose |
|---------|---------|
| `init <name>` | Create a new project |
| `show` | Display project summary |
| `task` | Task CRUD and status management |
| `effort` | Effort logging and estimation |
| `dev` | Developer registry management |
| `gantt` | ASCII Gantt chart |
| `tree` | ASCII dependency tree |
| `status` | Compact project dashboard |
| `report` | PM reports (completion, effort, bottlenecks) |
| `batch` | Atomic multi-command execution |

## Task Management

### Add a task

```
rustwerk task add "Title" [--id ID] [--desc DESC] \
  [--complexity N] [--effort AMT] [--tags TAGS]
```

- `--id`: mnemonic ID (e.g. `DOM-TAG`); auto-generated
  if omitted
- `--complexity`: Fibonacci scale (1, 2, 3, 5, 8, 13)
- `--effort`: estimate like `2H`, `1D`, `0.5W`, `1M`
- `--tags`: comma-separated tags (slug-like: lowercase
  alphanumeric + hyphens, max 50 chars each, max 20
  per task). Example: `--tags "backend,urgent"`

### Update a task

```
rustwerk task update <ID> [--title TITLE] [--desc DESC] \
  [--tags TAGS]
```

Use `--desc ""` to clear the description.
Use `--tags ""` to clear all tags.
At least one of `--title`, `--desc`, or `--tags` is
required.

### View task description

```
rustwerk task describe <ID>
```

Shows the contents of `.rustwerk/tasks/<ID>.md`. If no
file exists, prints the expected path.

### Remove a task

```
rustwerk task remove <ID>
```

Fails if other tasks depend on it. Remove dependencies
first with `task undepend`.

### Set task status

```
rustwerk task status <ID> <STATUS> [--force]
```

Valid statuses: `todo`, `in-progress`, `blocked`,
`done`, `on-hold`.

Valid transitions:
- `todo` -> `in-progress`, `on-hold`
- `in-progress` -> `done`, `blocked`, `on-hold`
- `blocked` -> `in-progress`, `todo`
- `on-hold` -> `todo`, `in-progress`

Use `--force` to bypass transition validation.

### List tasks

```
rustwerk task list [OPTIONS]
```

Filters (combinable):
- `--available` — TODO tasks with all deps done
- `--active` — IN_PROGRESS tasks only
- `--status <STATUS>` — filter by specific status
- `--assignee <DEV>` — filter by developer ID
- `--chain <ID>` — show task and its transitive deps
- `--tag <TAG>` — filter by tag

`--available` and `--status` are mutually exclusive.

### Dependencies

```
rustwerk task depend <FROM> <TO>    # FROM depends on TO
rustwerk task undepend <FROM> <TO>  # remove dependency
```

Cycles are rejected automatically.

### Assignment

```
rustwerk task assign <ID> [DEV_ID]
rustwerk task unassign <ID>
```

Developer must be registered first (see `dev add`).
If `DEV_ID` is omitted, falls back to `RUSTWERK_USER`
environment variable.

## Effort Tracking

### Log effort

```
rustwerk effort log <ID> <AMOUNT> [--dev <DEV>] \
  [--note NOTE]
```

Task must be IN_PROGRESS. Amount format: `2H`, `0.5D`,
etc. If `--dev` is omitted, falls back to
`RUSTWERK_USER` environment variable.

### Set estimate

```
rustwerk effort estimate <ID> <AMOUNT>
```

## Developer Management

```
rustwerk dev add <ID> <NAME> [--email EMAIL] \
  [--role ROLE]
rustwerk dev remove <ID>
rustwerk dev list
```

- `<ID>`: lowercase alphanumeric + hyphens (e.g.
  `alice`, `bob-smith`)
- `<NAME>`: display name (e.g. `"Alice Smith"`)

## Visualization

### Gantt chart

```
rustwerk gantt [--remaining]
```

`--remaining` hides done tasks and recalculates the
critical path.

### Dependency tree

```
rustwerk tree [--remaining]
```

### Project dashboard

```
rustwerk status
```

## Reports

```
rustwerk report complete      # completion summary
rustwerk report effort        # effort per developer
rustwerk report bottlenecks   # blocking tasks
```

## Batch Commands

Execute multiple commands atomically (all-or-nothing):

```
echo '[
  {"command": "task.add", "args": {"title": "A", "id": "A"}},
  {"command": "task.add", "args": {"title": "B", "id": "B"}},
  {"command": "task.depend", "args": {"from": "B", "to": "A"}}
]' | rustwerk batch
```

Or from a file:

```
rustwerk batch --file commands.json
```

### Batch command reference

| Command | Required | Optional |
|---------|----------|----------|
| `task.add` | `title` | `id`, `desc`, `complexity`, `effort`, `tags` (array) |
| `task.remove` | `id` | |
| `task.update` | `id` | `title`, `desc`, `tags` (array) |
| `task.status` | `id`, `status` | `force` (bool) |
| `task.assign` | `id`, `to` | |
| `task.unassign` | `id` | |
| `task.depend` | `from`, `to` | |
| `task.undepend` | `from`, `to` | |
| `effort.log` | `id`, `amount`, `dev` | `note` |
| `effort.estimate` | `id`, `amount` | |
| `dev.add` | `id`, `name` | `email`, `role` |
| `dev.remove` | `id` | |

Limits: 10 MB input, 1000 commands per batch.

## Effort Units

| Unit | Meaning | Equivalent |
|------|---------|------------|
| `H` | Hours | — |
| `D` | Days | 8H |
| `W` | Weeks | 40H |
| `M` | Months | 160H |

Examples: `2.5H`, `1D`, `0.5W`.

## Task ID Conventions

- Alphanumeric, hyphens, underscores only
- Auto-uppercased
- Mnemonic prefix by area: `DOM-`, `CLI-`, `DEP-`,
  `VIZ-`, `QRY-`, `RPT-`, `SER-`, `STORE-`, `GIT-`,
  `PLG-`, `HTML-`, `AI-`
- Auto-generated IDs: `T0001`, `T0002`, etc.

## Common Workflows

### Start working on a task

```
rustwerk task status TASK-ID in-progress
```

### Complete a task

```
rustwerk task status TASK-ID done
```

### Check what's available to work on

```
rustwerk task list --available
```

### See project progress

```
rustwerk status
rustwerk report complete
```

### Add multiple tasks with dependencies

Use batch for atomicity:

```
echo '[
  {"command": "task.add", "args": {
    "title": "Domain model", "id": "DOM-FOO",
    "complexity": 3, "effort": "2H"
  }},
  {"command": "task.add", "args": {
    "title": "CLI command", "id": "CLI-FOO",
    "complexity": 3, "effort": "3H"
  }},
  {"command": "task.depend", "args": {
    "from": "CLI-FOO", "to": "DOM-FOO"
  }}
]' | rustwerk batch
```

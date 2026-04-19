# RustWerk

Git-native, AI-agent-friendly project orchestration CLI.

A deterministic CLI tool for managing software development
projects. It keeps project state (WBS, tasks, effort logs)
in a git-versioned format, designed from the ground up for
AI agent integration. Single portable Rust binary, no
runtime dependencies.

## Design Principles

- **Procedural logic first** — the CLI handles everything
  achievable with simple algorithmic logic (graph
  traversal, critical path, status tracking). AI agents
  are only invoked for tasks that require reasoning —
  like WBS generation, effort estimation, risk
  assessment, or alternative scheduling.
- **AI agent friendly** — all input/output uses structured
  formats (JSON) so AI agents can reliably parse and
  produce data. Commands are designed for incremental,
  composable use by agents.
- **Git-native** — project state lives in the repo, on
  the main branch, with atomic commits and immediate
  push. The repo is the single source of truth.
- **Incremental building** — the WBS is built
  incrementally via CLI commands, task by task, not
  generated all at once.
- **Single binary** — portable Rust binary, no runtime
  dependencies.

## WBS Data Model

Each task in the Work Breakdown Structure has:

| Field              | Description                          |
|--------------------|--------------------------------------|
| Unique ID          | For referencing in dependencies/CLI  |
| Title/description  | What the task is about               |
| Dependencies       | Links to other task IDs              |
| Effort (estimated) | Planned effort/complexity            |
| Effort (actual)    | Logged time spent, by whom           |
| Status             | TODO, IN_PROGRESS, BLOCKED, DONE     |
| Assignee           | Developer working on the task        |

The WBS forms a tree/graph structure that the CLI can
model, traverse, and query — find blocked tasks, compute
the critical path, identify bottlenecks.

## CLI Commands (Planned)

### Project & Task Management
- Initialize a project
- Add, remove, and update tasks incrementally
- Define dependencies between tasks
- Set task status (TODO → IN_PROGRESS → BLOCKED → DONE)
- Log effort per task (amount, developer)

### Querying
- Query project state (status, blockers, progress)
- List tasks by state, assignee, or dependency chain
- Find bottlenecks and dependency issues

### Visualization & Reporting
- ASCII tree view of the WBS
- Gantt charts
- Critical path analysis
- Static HTML reports and dashboards (Mermaid, PlantUML)

## Developer Dashboard

Per-developer view of:
- Active tasks and their status
- Pending PRs
- Review status
- Logged effort
- Current blockers

## PM / Tech Lead Reporting

### CLI Reports
- Task completion summary
- Effort per developer
- Bottlenecks and dependency issues

### Static Site Export
- Generate dashboards using Mermaid and PlantUML
- Static HTML reports for sharing with stakeholders

## Git & Branching Model

### Problem

Developers work in feature branches, but central project
state must remain consistent on main.

### Solution: Dual-Context CLI

The CLI operates in two contexts:
- **Feature branch** → code changes
- **Main branch** → project state (WBS, tasks, effort)

### Automated Flow

When reserving a task or logging effort:
1. Switch to main
2. Update the project definition
3. Commit + push (atomic)
4. Return to the feature branch

### Centralized State

Main branch is the single source of truth. The CLI
enforces atomic updates with immediate push, eliminating
task conflicts and double assignment.

## Activity Logging

All activity is stored in the repo:
- PR comments
- Decisions
- Progress logs

Version-controlled for auditable history and full context
preservation.

## Plugin Architecture

RustWerk is designed for extensibility through plugins:

### Jira Integration

Jira acts as a transparent backend — no explicit Jira
commands needed.

- **On task start:** if a Jira task exists, link it;
  otherwise create one, populate from the WBS, and store
  the mapping
- **During work:** sync effort logs, descriptions, and
  status bidirectionally
- **Dependencies:** auto-link Jira issues based on the
  WBS dependency graph

### Bitbucket Integration

- PR creation from tasks
- Reviewer assignment
- Status tracking
- Comment retrieval

### Other Plugins

The plugin system is open for additional integrations
(e.g. reporting, notifications).

## AI Agent Role

The CLI is the deterministic engine; the AI is the
planning and reasoning layer.

- **WBS generation** — agent produces an initial WBS in
  structured format, CLI ingests it
- **Effort estimation** — suggest estimates based on task
  complexity and historical data
- **Risk assessment** — identify scheduling risks and
  suggest mitigations
- **Orchestration** — agent calls CLI commands to
  incrementally build and update the project

The AI agent only handles what algorithmic logic cannot.

## Key Challenges

1. Git branch context switching (dual-context operations)
2. Centralized state consistency (atomic updates)
3. API integrations (Jira, Bitbucket)
4. Dependency graph correctness
5. Developer workflow ergonomics

## Install

Prebuilt binaries for Linux, macOS, and Windows are
published to each [GitHub Release][releases]. No Rust
toolchain required.

**Linux / macOS**

```bash
curl -fsSL https://raw.githubusercontent.com/breki/rustwerk/main/scripts/install.sh | sh
```

**Windows (PowerShell)**

```powershell
irm https://raw.githubusercontent.com/breki/rustwerk/main/scripts/install.ps1 | iex
```

Both installers detect OS/arch, download the matching
archive from the latest release, verify its SHA256
against the published `SHA256SUMS`, and install the
binary (defaults: `~/.local/bin` on Unix,
`%LOCALAPPDATA%\Programs\rustwerk\bin` on Windows).

**Environment overrides** (set before piping to the
shell):

- `RUSTWERK_VERSION` — install a specific version
  (e.g. `v0.40.0`) instead of the latest.
- `RUSTWERK_INSTALL_DIR` — override the install
  directory.
- `RUSTWERK_MODIFY_PATH=1` *(Windows only)* — append
  the install directory to the user's persistent PATH.
  Off by default; `install.sh` never edits shell rc
  files either.

Alternatively, with a Rust toolchain installed:

```bash
cargo install --git https://github.com/breki/rustwerk rustwerk
```

[releases]: https://github.com/breki/rustwerk/releases

## Build

```bash
cargo xtask validate   # clippy + tests
cargo xtask test       # tests only
cargo xtask clippy     # lint only
cargo xtask fmt        # format code
```

## License

MIT

# RustWerk — Work Breakdown Structure

Implementation roadmap using DDD bounded contexts and
Red/Green TDD. Each task: write failing tests (red),
implement (green), refactor, commit.

**Scope:** everything in README except Jira/Bitbucket
plugins.

**Complexity:** Fibonacci scale (1, 2, 3, 5, 8, 13).

---

## DDD Bounded Contexts

| Context | Aggregates | Introduced |
|---|---|---|
| **Project** | `Project`, `ProjectMetadata`, `Developer` | Phase 1 |
| **Task** | `Task`, `Status`, `Effort`, `Assignee` | Phase 1 |
| **Dependency Graph** | `DependencyGraph`, `CriticalPath` | Phase 3 |
| **Persistence** | `ProjectStore` | Phase 1 |
| **Git Operations** | `GitContext`, `AtomicUpdate` | Phase 5 |
| **Plugin** | `PluginRegistry`, `Plugin` trait | Phase 7 |

---

## Phase 1 — Project File Format and Init

**Goal:** define the project definition file format,
implement domain types to support it, create/load it
via CLI. After this phase `rustwerk init` and
`rustwerk show` work end-to-end.

| ID | Task | Complexity | Depends on |
|----|------|-----------|------------|
| FILE-SCHEMA | Design project definition file format (JSON schema for `.rustwerk/project.json` — project metadata, task list, dependencies, effort, assignees) | 5 | — |
| DOM-ERR | `DomainError` enum (`thiserror`) | 2 | — |
| DOM-PROJ | `Project` aggregate and `ProjectMetadata` value object | 3 | FILE-SCHEMA, DOM-ERR |
| DOM-TASK | `Task`, `TaskId`, `Status` enum, `Title` | 3 | FILE-SCHEMA, DOM-ERR |
| SER-JSON | JSON serialization round-trip for project and tasks | 3 | DOM-PROJ, DOM-TASK |
| STORE-FILE | File-based `ProjectStore` (save/load `.rustwerk/project.json`) | 3 | SER-JSON |
| CLI-INIT | CLI `init` — create project file with name | 2 | STORE-FILE |
| CLI-SHOW | CLI `show` — load and display project summary | 2 | STORE-FILE |

---

## Phase 2 — Task CRUD via CLI

**Goal:** add, remove, update tasks and manage status,
effort, assignees through CLI commands. Each command
reads the project file, mutates, and saves back.

| ID | Task | Complexity | Depends on |
|----|------|-----------|------------|
| DOM-EFFORT | `Effort`, `EffortEntry`, `Assignee` value objects | 2 | DOM-TASK |
| DOM-ADD | Add task to project (domain method + ID generation) | 3 | DOM-PROJ, DOM-TASK |
| DOM-REMOVE | Remove task from project | 2 | DOM-ADD |
| DOM-UPDATE | Update task fields (title, description) | 2 | DOM-ADD |
| DOM-STATUS | Status transitions with validation (TODO→IN_PROGRESS→DONE, etc.) | 3 | DOM-TASK |
| DOM-LOG | Effort logging (requires IN_PROGRESS status) | 3 | DOM-EFFORT, DOM-STATUS |
| DOM-ASSIGN | Assignee management (assign/unassign) | 2 | DOM-EFFORT, DOM-ADD |
| CLI-TASK | CLI `task add` / `task remove` / `task update` | 5 | DOM-ADD, DOM-REMOVE, DOM-UPDATE, STORE-FILE |
| CLI-STATUS | CLI `task status` (set status) | 2 | DOM-STATUS, STORE-FILE |
| CLI-EFFORT | CLI `effort log` / `effort estimate` | 3 | DOM-LOG, STORE-FILE |
| CLI-ASSIGN | CLI `task assign` / `task unassign` | 2 | DOM-ASSIGN, STORE-FILE |
| CLI-JSON | Global `--json` output flag for all commands | 5 | CLI-TASK |
| DOM-DEV | `Developer` struct (name, email?, role?, specialties?) in project definition | 3 | DOM-PROJ |
| CLI-DEV-ADD | CLI `dev add` — add a developer to the project | 2 | DOM-DEV, STORE-FILE |
| CLI-DEV-REMOVE | CLI `dev remove` — remove a developer | 2 | DOM-DEV, STORE-FILE |
| CLI-DEV-LIST | CLI `dev list` — list all developers | 1 | DOM-DEV, STORE-FILE |
| DEV-ASSIGN | Link `task assign` to validated developer IDs instead of free-text strings | 3 | DOM-DEV, DOM-ASSIGN |

---

## Phase 3 — Dependency Graph

**Goal:** tasks declare dependencies as a DAG. Cycle
detection, topological sort, critical path analysis.

| ID | Task | Complexity | Depends on |
|----|------|-----------|------------|
| DEP-ADD | Add dependency between tasks with cycle detection (DFS) | 5 | DOM-ADD |
| DEP-REMOVE | Remove dependency | 2 | DEP-ADD |
| DEP-TOPO | Topological sort (Kahn's algorithm) | 3 | DEP-ADD |
| DEP-BLOCK | Blocked status auto-detection (incomplete upstream deps) | 3 | DEP-ADD, DOM-STATUS |
| DEP-CRIT | Critical path analysis (longest-path on weighted DAG) | 5 | DEP-TOPO |
| DEP-GUARD | Prevent removal of tasks with dependents | 2 | DEP-ADD, DOM-REMOVE |
| CLI-DEP | CLI `depend add` / `depend remove` / `critical-path` | 3 | DEP-ADD, DEP-REMOVE, DEP-CRIT, STORE-FILE |

---

## Phase 4 — Querying and Visualization

**Goal:** list/filter tasks, ASCII tree, Gantt chart,
status summary.

| ID | Task | Complexity | Depends on |
|----|------|-----------|------------|
| QRY-LIST | List tasks with filters (by status, assignee, dependency chain) | 3 | DOM-ADD, DEP-ADD |
| QRY-BOTTLE | Bottleneck detection (tasks with most downstream dependents) | 3 | DEP-ADD |
| VIZ-TREE | ASCII tree view of WBS with status indicators | 5 | DEP-TOPO |
| VIZ-GANTT | ASCII Gantt chart (sequential/parallel lanes, critical path highlight) | 8 | DEP-CRIT |
| QRY-SUMMARY | Project status summary (counts, %, effort totals) | 2 | DOM-ADD, DOM-LOG |
| CLI-VIZ | CLI `list`, `tree`, `gantt`, `status` commands | 3 | QRY-LIST, QRY-BOTTLE, VIZ-TREE, VIZ-GANTT, QRY-SUMMARY |
| VIZ-UNICODE | Upgrade Gantt bars to Unicode blocks (█▓░) with bar caps (▐▌) | 3 | VIZ-GANTT |
| VIZ-AXIS | Upgrade Gantt time axis to box-drawing chars (─│┬┴) | 2 | VIZ-GANTT |
| VIZ-SCALE | Terminal-width-aware Gantt scaling (detect width, scale bars) | 5 | VIZ-GANTT |
| VIZ-MILESTONE | Milestone markers (◆) for zero-complexity tasks | 2 | VIZ-GANTT |
| VIZ-TUI | Optional --interactive Gantt mode with ratatui (scrollable, zoomable) | 13 | VIZ-GANTT, VIZ-UNICODE, VIZ-AXIS, VIZ-SCALE |

---

## Phase 5 — Git Operations

**Goal:** atomically commit project state to main from
any branch. Dual-context (feature branch ↔ main).

| ID | Task | Complexity | Depends on |
|----|------|-----------|------------|
| GIT-CTX | Git context detection (current branch, is-main check) | 3 | — |
| GIT-SWITCH | Stash + branch switch (RAII context manager for main) | 5 | GIT-CTX |
| GIT-ATOMIC | Atomic commit and push (add, commit, push; rollback on failure) | 8 | GIT-SWITCH |
| GIT-CONFLICT | Conflict resolution (pull --rebase before commit, abort on conflict) | 5 | GIT-ATOMIC |
| GIT-WIRE | Wire git into mutating CLI commands (`--git` flag) | 3 | GIT-ATOMIC, CLI-TASK |

---

## Phase 6 — Reporting and Dashboards

**Goal:** per-developer dashboard, PM reports, static
HTML export.

| ID | Task | Complexity | Depends on |
|----|------|-----------|------------|
| RPT-DEV | Developer dashboard query (active/pending/blocked tasks, effort) | 3 | DOM-LOG, DOM-ASSIGN, DEP-BLOCK |
| RPT-COMPLETE | PM completion summary (counts, %, estimated vs actual) | 2 | QRY-SUMMARY |
| RPT-EFFORT | Effort per developer report | 2 | DOM-LOG |
| RPT-BOTTLE | PM bottleneck report (bottleneck + assignee + status) | 3 | QRY-BOTTLE, DOM-ASSIGN |
| CLI-RPT | CLI `dashboard`, `report summary/effort/bottlenecks` | 3 | RPT-DEV, RPT-COMPLETE, RPT-EFFORT, RPT-BOTTLE |
| HTML-MERMAID | Static HTML export with Mermaid (Gantt + dependency graph) | 5 | VIZ-GANTT, DEP-ADD |
| HTML-PUML | Static HTML export with PlantUML (Gantt + WBS diagram) | 5 | VIZ-GANTT, DEP-ADD |
| HTML-SITE | Multi-page static site export (overview + per-dev pages) | 8 | CLI-RPT, HTML-MERMAID |

---

## Phase 7 — Plugin Architecture

**Goal:** extensible plugin system with hook points
and a built-in activity logging plugin as reference
implementation.

| ID | Task | Complexity | Depends on |
|----|------|-----------|------------|
| PLG-TRAIT | `Plugin` trait (object-safe: `id`, `name`, `on_event`, `hooks`) | 3 | — |
| PLG-REG | `PluginRegistry` (register, unregister, dispatch events) | 3 | PLG-TRAIT |
| PLG-EVENT | `DomainEvent` enum and `HookPoint` subscription mechanism | 3 | PLG-REG |
| PLG-WIRE | Wire event dispatching into `Project` mutation methods | 5 | PLG-EVENT, DOM-ADD |
| PLG-CONFIG | Plugin configuration (per-plugin JSON in project file) | 3 | PLG-REG, SER-JSON |
| PLG-ACTLOG | Built-in `ActivityLogPlugin` (append JSON lines to `.rustwerk/activity.log`) | 3 | PLG-WIRE |

---

## Phase 8 — AI Agent Integration

**Goal:** structured JSON I/O for AI agents — WBS
ingestion, effort estimation hooks, batch commands.

| ID | Task | Complexity | Depends on |
|----|------|-----------|------------|
| AI-SCHEMA | WBS JSON import schema (`WbsImport` struct) | 3 | FILE-SCHEMA |
| AI-INGEST | CLI `wbs ingest` (create tasks + deps from JSON file) | 5 | AI-SCHEMA, DOM-ADD, DEP-ADD |
| AI-EXPORT | CLI `wbs export` (serialize project as WBS JSON) | 3 | AI-SCHEMA, SER-JSON |
| AI-ESTIMATE | Effort estimation hooks (two-phase: `estimate request` → `estimate apply`) | 5 | DOM-LOG, CLI-JSON |
| AI-ERRORS | Structured JSON error output (stable error codes) | 3 | CLI-JSON |
| AI-BATCH | Batch command execution (JSON command array, atomic rollback) | 8 | CLI-TASK |

---

## Phase Dependency Graph

```
Phase 1 (File Format + Init)
    │
    └──► Phase 2 (Task CRUD CLI)
             │
             ├──► Phase 3 (DAG)
             │        │
             │        ├──► Phase 4 (Querying + Viz)
             │        │        │
             │        │        └──► Phase 6 (Reporting)
             │        │
             │        └──► Phase 8 (AI Agent)
             │
             ├──► Phase 5 (Git Ops)
             │
             └──► Phase 7 (Plugin Architecture)
```

---

## Summary

| Phase | Tasks | Complexity | Deliverable |
|-------|------:|-----------:|-------------|
| 1 — File Format + Init | 8 | 23 | Project file schema, `init`, `show` |
| 2 — Task CRUD CLI | 17 | 45 | Full task management via CLI + developer registry |
| 3 — Dependency Graph | 7 | 23 | DAG, cycle detection, critical path |
| 4 — Querying + Viz | 11 | 49 | Filters, tree/Gantt, Unicode bars, TUI |
| 5 — Git Operations | 5 | 24 | Dual-context, atomic commit+push |
| 6 — Reporting | 8 | 31 | Dashboards, PM reports, HTML export |
| 7 — Plugin Architecture | 6 | 20 | Plugin system + activity log |
| 8 — AI Agent Integration | 6 | 27 | WBS ingestion, estimation, batch |
| **Total** | **68** | **242** | |

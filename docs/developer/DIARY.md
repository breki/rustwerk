# Development Diary

This diary tracks functional changes to the RustWerk codebase in
reverse chronological order.

---

### 2026-04-03

- Add WBS import/export schema for AI agents (v0.12.0)

    New `ai::wbs_schema` module with `WbsTaskEntry` struct for
    bulk task creation. `parse_wbs`/`serialize_wbs` for JSON I/O.
    `import_into_project` creates tasks then adds dependencies
    (two-pass, idempotent — skips existing IDs). `export_from_project`
    serializes current tasks back to WBS format. Rejects cycles
    during import. 8 new tests including round-trip and cycle
    detection.

- Add code coverage enforcement and red team findings log

    `cargo xtask coverage` runs `cargo-llvm-cov` and enforces a 90%
    line coverage threshold. `cargo xtask validate` now includes
    coverage as the final step (clippy → tests → coverage). Added
    22 CLI integration tests (`crates/rustwerk/tests/cli_integration.rs`)
    and 18 in-process batch command tests. Coverage went from 68% to
    94.9%. Added `docs/developer/redteam-log.md` with all 12 historical
    findings backfilled. Updated `/commit` to maintain the log and warn
    when 10+ findings are open.

- Add atomic batch command execution (v0.11.0)

    `rustwerk batch [--file path]` executes a JSON array of commands
    atomically — loads project once, runs all commands in-memory, saves
    only if all succeed. On any failure, nothing is persisted and the
    error is reported as JSON with the failing command index. Reads from
    file or stdin. Supports all 10 command types (`task.add`,
    `task.status`, `task.depend`, `effort.log`, etc.). Designed for AI
    agent integration — agents can pipe structured JSON to execute
    complex multi-step operations in a single atomic call.

- Add project status summary to `show` command (v0.10.0)

    `Project::summary()` returns `ProjectSummary` with task counts
    by status, % complete, total estimated/actual effort hours, and
    total complexity. `show` command now displays a full project
    dashboard. Updated `/next-task` to use direct binary, `/commit`
    to always run red team on code changes. 3 new tests.

- Add effort logging and estimation (v0.9.0)

    `effort log ID AMOUNT --dev NAME` logs effort on IN_PROGRESS tasks.
    `effort estimate ID AMOUNT` sets estimated effort. `Effort::to_hours()`
    converts all units to hours (1D=8H, 1W=40H, 1M=160H).
    `Task::total_actual_effort_hours()` sums logged entries. 5 new tests.

### 2026-04-02

- Add assignee management and `/next-task` command (v0.8.0)

    `task assign ID --to NAME` and `task unassign ID` CLI commands
    with `assign`/`unassign` domain methods on `Project`. Added
    `/next-task` Claude Code skill that lists available WBS tasks,
    lets the user pick one, marks it in-progress, plans if needed,
    implements with TDD, and commits. 6 new tests.

- Add `--force` flag to `task status` (v0.7.0)

    `task status ID STATUS --force` bypasses transition validation,
    allowing corrections like DONE→TODO. `set_status` domain method
    now takes a `force: bool` parameter.

- Add task remove and update commands (v0.6.0)

    `task remove` deletes a task, guarded by dependency check — cannot
    remove a task that others depend on. `task update` changes title
    and/or description (use `--desc ""` to clear). Domain methods
    `remove_task` and `update_task` on `Project`. 9 new tests.

- Add topological sort, critical path analysis, and `*` marker (v0.5.0)

    `topological_sort()` via Kahn's algorithm returns tasks in
    dependency order. `critical_path()` finds the longest chain by
    complexity weight using DP on the topological order.
    `critical_path_set()` returns the set for O(1) membership checks.
    `task list` now marks critical-path tasks with `*`. 7 new tests.

- Add dependency management and available task filtering (v0.4.0)

    `task depend` and `task undepend` CLI commands manage task
    dependencies. `add_dependency` validates both task IDs exist,
    rejects self-dependencies and cycles via DFS. `task list
    --available` shows only tasks whose dependencies are all done.
    `available_tasks()` query on `Project` aggregate. All WBS
    dependencies imported into dogfooding project file. 15 new
    tests for dependency CRUD, cycle detection, and availability
    filtering.

- Add task management CLI commands (v0.3.0)

    `task add` creates tasks with optional mnemonic ID, description,
    complexity, and effort estimate. Auto-generates sequential IDs
    (T1, T2...) when no ID is provided. `task status` sets task status
    with transition validation. `task list` displays all tasks with
    status and complexity. Domain methods `add_task`, `add_task_auto`,
    and `set_status` on `Project` aggregate. Enables dogfooding —
    rustwerk can now track its own development tasks.

- Implement Phase 1: core domain, persistence, CLI init/show (v0.2.0)

    Added DDD domain model: `Project` aggregate, `Task` with `Status`
    enum, `Effort` with time-unit parsing ("2.5H", "1D", "0.5W",
    "1M"), `DomainError` via `thiserror`. JSON persistence layer with
    file-based `ProjectStore` saving to `.rustwerk/project.json`. CLI
    `init` creates a new project file, `show` displays project summary.
    44 unit tests covering domain types, serialization round-trips, and
    file store operations.

- Initial project scaffold (v0.1.0)

    Set up workspace with `rustwerk` library/binary crate and `xtask`
    build tooling. CLI skeleton using `clap` with `serde`/`serde_json`
    for structured I/O. Workspace-level `#[deny(warnings)]` and
    clippy pedantic lints enabled.

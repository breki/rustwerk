# Development Diary

This diary tracks functional changes to the RustWerk codebase in
reverse chronological order.

---

### 2026-04-03

- Add blocked-by-deps auto-detection (v0.25.0)

    New `Project::dep_blocked_tasks()` method returns tasks
    that are TODO but have at least one incomplete dependency.
    Complements `available_tasks()` (all deps done) and
    `active_tasks()` (in-progress).

- Add `dev add` and `dev remove` commands (v0.24.0)

    New `rustwerk dev add` registers a developer with name,
    optional email and role. `rustwerk dev remove` unregisters
    a developer (blocked if any task is assigned to them).

- Add `report effort` command (v0.23.0)

    New `rustwerk report effort` command shows effort
    breakdown per developer with hours and percentage.

- Add `dev list` command (v0.22.0)

    New `rustwerk dev list` command shows all registered
    developers with name, email, and role.

- Add `report complete` command (v0.21.0)

    New `rustwerk report complete` command shows a PM-friendly
    completion summary: status breakdown, visual progress bar,
    estimated vs actual effort with burn rate, complexity
    totals, and remaining critical path with task IDs.

- Upgrade Gantt time axis to box-drawing chars (v0.20.0)

    Axis line now uses `┬` for tick marks and `─` for the
    horizontal rule, replacing plain `|` and spaces.

- Add `--remaining` flag to `gantt` command (v0.19.0)

    `rustwerk gantt --remaining` filters out done tasks,
    showing only the remaining work. Filtering happens after
    scheduling so bar positions reflect the full timeline.

- Red Gantt bars for critical path tasks (v0.18.0)

    Critical path tasks now render the entire line (marker,
    ID, and bar) in red, overriding the status-based color.
    Extracted `bar_style()` function for testability with 4
    new unit tests. Red chosen over bold/underline because
    those are not visible enough on most terminal themes.

- Fix Gantt chart alignment and bar overlap (v0.17.1)

    Three bugs fixed: (1) `TaskId::Display` didn't forward
    format specifiers, causing ID column padding to be ignored
    and bars to start at wrong columns. (2) Bar caps were added
    outside the scaled width, causing consecutive bars to overlap
    by 1 column. (3) Header tick marks were misaligned with bar
    positions. Added 4 visual integration tests for Gantt layout.

- Upgrade Gantt bars to Unicode blocks with caps (v0.17.0)

    Gantt bars now use Unicode block characters instead of ASCII:
    `█` (full block) for done/blocked, `▓` (dark shade) for
    in-progress filled portion, `░` (light shade) for remaining/todo.
    Bar brackets `[]` replaced with half-block caps `▐` `▌` for a
    polished look. New `left_cap()` and `right_cap()` methods on
    `GanttRow`. 7 new tests for character selection.

- Add terminal-width-aware Gantt scaling (v0.16.0)

    `rustwerk gantt` now detects terminal width via the `terminal_size`
    crate and scales bars proportionally when the chart would overflow.
    Scale factor capped at 1.0 (never stretches beyond 1:1). Tick
    interval widens at small scales. Minimum bar width of 1 character
    ensures no task disappears.

- Add Developer domain type and project registry (v0.15.0)

    New `Developer` struct with name, optional email, role, and
    specialties. `DeveloperId` newtype (lowercase ASCII alphanumeric).
    `Project` gains `developers` map with `add_developer` and
    `remove_developer` methods. Removal blocked if any task is
    assigned to the developer. JSON serialization round-trips.
    15 new tests (8 for DeveloperId/Developer, 7 for Project
    integration).

- Add ANSI colors to Gantt chart (v0.14.0)

    `rustwerk gantt` now renders with ANSI colors: green for done,
    yellow/bold for in-progress, red for blocked, dim for todo, cyan
    for critical-path markers. Auto-detects terminal via
    `std::io::IsTerminal`; respects `NO_COLOR` env var. Scale header
    rendered in dim. No external dependencies.

- Fix `--available` to show TODO only, add `--active` flag (v0.13.1)

    `task list --available` now shows only TODO tasks whose deps are
    all done (previously included IN_PROGRESS). New `task list --active`
    shows only IN_PROGRESS tasks. `active_tasks()` query on `Project`.
    3 new tests.

- Add ASCII Gantt chart command (v0.13.0)

    `rustwerk gantt` renders a dependency-aware Gantt chart. Tasks
    positioned by topological sort — start column = max(end of deps).
    Bar width = complexity score. Fill shows status: `#` done, `#.`
    in-progress, `.` todo, `!` blocked. Critical path tasks marked
    with `*`. Scale header with column markers every 5 units.
    `gantt_schedule()` on `Project` returns `Vec<GanttRow>`. 6 new
    tests.

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

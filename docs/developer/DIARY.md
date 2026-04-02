# Development Diary

This diary tracks functional changes to the RustWerk codebase in
reverse chronological order.

---

### 2026-04-02

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

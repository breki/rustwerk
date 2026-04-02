# Development Diary

This diary tracks functional changes to the RustWerk codebase in
reverse chronological order.

---

### 2026-04-02

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

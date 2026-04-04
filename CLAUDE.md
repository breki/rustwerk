# RustWerk

Git-native, AI-agent-friendly project orchestration CLI.

- **Stack**: Rust, single portable binary
- **Target platforms**: Windows, Linux, macOS
- **Interface**: CLI with structured JSON I/O
- **Design**: procedural logic first, AI only for
  reasoning tasks

## Build Commands

```bash
cargo xtask validate             # clippy + tests + coverage
cargo xtask test [filter]        # tests only
cargo xtask clippy               # lint only
cargo xtask coverage             # coverage only (≥90%)
cargo xtask fmt                  # format code
```

Never use raw `cargo test` or `cargo clippy` — always
go through `xtask`.

The working directory is already set to the project root.
Never use `cd` to the project root or `git -C <dir>`.

## Coding Standards

- Rust edition 2021
- `#[deny(warnings)]` via workspace lints
- clippy pedantic where practical
- Error handling: `thiserror` for library errors,
  `anyhow` for CLI errors
- Prefer `&str` over `String` in function signatures
- All public items must have doc comments
- Wrap markdown at 80 characters per line

## Commits

Use `/commit`. No "Co-Authored-By", no emoji.

## End-User Manual

The file `docs/manual.md` is the end-user manual. Update
it when a `feat` or `fix` commit changes CLI behavior.
Skip updates for internal refactors or non-user-facing
work.

## Skills

Project-specific skills available as slash commands:

| Skill | Purpose |
|-------|---------|
| `/commit` | Commit with versioning, diary, and code review |
| `/next-task` | Pick and implement the next WBS task |
| `/todo` | Process the next pending TODO item |
| `/rustwerk` | RustWerk CLI reference for project management |
| `/tui-expert` | TUI rendering and alignment guidelines |
| `/simplify` | Review changed code for quality |

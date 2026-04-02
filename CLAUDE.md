# RustWerk

Git-native, AI-agent-friendly project orchestration CLI.

## Quick Reference

- **Stack**: Rust, single portable binary
- **Target platforms**: Windows, Linux, macOS
- **Interface**: CLI with structured JSON I/O
- **Design**: procedural logic first, AI only for
  reasoning tasks

## CLI Commands

```bash
# Build (never use raw cargo test/clippy)
cargo xtask validate             # clippy + tests
cargo xtask test [filter]        # tests only
cargo xtask clippy               # lint only
cargo xtask fmt                  # format code
```

**Important:** The working directory is already set to
the project root. Never use `cd` to the project root
or `git -C <project-dir>` — run all commands directly.

**Commits:** Use `/commit` command. No "Co-Authored-By",
no emoji.

## Formatting

- Wrap markdown at 80 characters per line.

## Coding Standards

- Rust edition 2021
- `#[deny(warnings)]` via workspace lints
- clippy pedantic where practical
- Error handling: use `thiserror` for library errors,
  `anyhow` for CLI errors (add when needed)
- Prefer `&str` over `String` in function signatures
- All public items must have doc comments

## Semantic Versioning

Version lives in `crates/rustwerk/Cargo.toml` (single source
of truth). The `/commit` command handles version bumps
automatically based on commit type:

| Commit Type | Version Bump |
|-------------|-------------|
| `feat` | **minor** (0.x.0) |
| `fix`, `perf` | **patch** (0.1.x) |
| `docs`, `test`, `refactor`, `chore`, `style` | no bump |

## Development Diary

Track significant changes in `docs/developer/DIARY.md`. Add
entries for functional or infrastructure changes only. See
[docs/developer/diary-guidelines.md](docs/developer/diary-guidelines.md)
for format details.

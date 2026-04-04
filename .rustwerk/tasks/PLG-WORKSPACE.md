# PLG-WORKSPACE: Wire plugin crates into workspace

## Why

The plugin architecture requires two new crates
(`rustwerk-plugin-api` and `rustwerk-jira-plugin`) to
live in the workspace. The main binary crate needs new
dependencies (`libloading`, `rustwerk-plugin-api`).
This task sets up the scaffolding so the other PLG
tasks can build on it.

## What

1. Add `crates/rustwerk-plugin-api` and
   `crates/rustwerk-jira-plugin` to workspace members
   in the root `Cargo.toml`.
2. Add dependencies to `crates/rustwerk/Cargo.toml`:
   - `libloading = { version = "0.8", optional = true }`
   - `rustwerk-plugin-api = { path = "../rustwerk-plugin-api" }`
3. Add feature flag:
   ```toml
   [features]
   default = ["plugins"]
   plugins = ["dep:libloading"]
   ```
4. Handle `unsafe_code = "forbid"` at workspace level:
   - `rustwerk-plugin-api`: no override needed (no
     unsafe code)
   - `rustwerk-jira-plugin`: crate-level
     `unsafe_code = "allow"` (cdylib exports require
     `extern "C"` with raw pointers)

## How

- Edit `Cargo.toml` (workspace root)
- Edit `crates/rustwerk/Cargo.toml`
- Create minimal `crates/rustwerk-plugin-api/Cargo.toml`
  with serde + serde_json deps
- Create minimal `crates/rustwerk-jira-plugin/Cargo.toml`
  with `crate-type = ["cdylib"]` and lint override

## Acceptance criteria

- [ ] `cargo check` succeeds for the entire workspace
- [ ] `cargo xtask clippy` passes (no new warnings)
- [ ] Both new crates are empty but structurally valid
- [ ] `libloading` is optional and gated behind the
      `plugins` feature
- [ ] Building without `--features plugins` still works

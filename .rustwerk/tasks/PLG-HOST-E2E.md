# PLG-HOST-E2E: Load-discover-invoke integration test

## Why

`bin/rustwerk/plugin_host.rs` sits at 48.5% coverage
and is coverage-exempt — the unsafe FFI paths need a
real cdylib to exercise. `jira_client.rs` sits at
81.7% and is exempt for the same reason. A single
integration test that actually builds the jira plugin,
loads it, and drives `plugin list` + `plugin push
--dry-run` against it closes both gaps without needing
a live Jira.

## What

### Test harness

A Rust integration test (`crates/rustwerk/tests/
plugin_e2e.rs`) that:

1. Invokes `cargo build -p rustwerk-jira-plugin`
   programmatically (via `cargo metadata` to find the
   target dir, then `std::process::Command`).
2. Copies the resulting cdylib into a scratch temp
   dir laid out like `<tmp>/.rustwerk/plugins/`.
3. Spawns `cargo run -p rustwerk -- plugin list` with
   cwd set to `<tmp>` and asserts the jira plugin
   appears in the output.
4. Spawns `... plugin push jira --dry-run
   --project-key TEST --tasks TASK-1` and asserts the
   dry-run prints the ADF payload for TASK-1 without
   making network calls.

### Dry-run discipline

`plugin push --dry-run` must guarantee zero outbound
HTTP. Today that's enforced at the plugin level; this
test pins it by asserting via a loopback socket that
no connection attempt happens on any interface
(implementation: bind port 0 and set
`JIRA_URL=https://127.0.0.1:<port>`; fail if
`accept()` returns anything).

### Build fixture management

Building the cdylib inside a test is slow (~seconds
on a warm target dir). Gate the test behind a
`#[ignore]` attribute **only** if it exceeds ~5s on
CI; otherwise keep it in the default `cargo test`
run.

## How

- `crates/rustwerk/tests/plugin_e2e.rs`: the test file.
- Reuse `rustwerk-plugin-api::API_VERSION` in the
  assertions so an accidental API version bump trips
  the test.
- Temp dirs via `tempfile::TempDir`; no global state.

## Acceptance criteria

- [ ] Test builds the jira cdylib, loads it via
      `plugin list`, and asserts the discovered name
      is `jira` with `push_tasks` capability
- [ ] Test runs `plugin push jira --dry-run` and
      asserts the emitted payload matches the ADF
      shape PLG-MAP produces
- [ ] Loopback socket proves no HTTP is attempted in
      `--dry-run`
- [ ] Coverage exemptions on `plugin_host.rs` and
      `jira_client.rs` can be tightened (not
      necessarily removed — transport error paths
      still need a fake)
- [ ] `cargo xtask validate` passes

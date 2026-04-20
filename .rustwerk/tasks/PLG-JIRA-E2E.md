# PLG-JIRA-E2E: Live-Jira smoke test

## Why

Both PLG-JIRA and PLG-MAP have "test with actual Jira
Cloud free instance (manual verification)" in their
acceptance criteria. The HTTP client fake catches
logic bugs but not wiring bugs — TLS config, header
casing, gateway-fallback cloud-id extraction, real ADF
validation — and nobody remembers to run the manual
check before a release.

An opt-in integration test, gated on env vars, makes
the check reproducible without forcing CI to have Jira
credentials.

## What

### Opt-in gating

A `#[test]` in `crates/rustwerk-jira-plugin/tests/`
that returns early with `eprintln!("skipping:
RUSTWERK_JIRA_TEST_* env vars not set")` unless all of
these are present:

- `RUSTWERK_JIRA_TEST_URL`
- `RUSTWERK_JIRA_TEST_TOKEN`
- `RUSTWERK_JIRA_TEST_USERNAME`
- `RUSTWERK_JIRA_TEST_PROJECT`

When set, the test builds a `JiraClient` (real ureq,
not the fake), creates an issue with a UUID-suffixed
summary, asserts the response shape, and deletes the
issue via `DELETE /rest/api/3/issue/{key}` in a
teardown guard that runs even on panic.

### Teardown guard

A small RAII struct that holds the issue key and hits
DELETE on drop, using
`std::thread::panicking()`-aware formatting so a
failed create doesn't leak state.

### Docs

README or manual snippet showing how to set the env
vars and run:

```bash
RUSTWERK_JIRA_TEST_URL=https://foo.atlassian.net \
RUSTWERK_JIRA_TEST_TOKEN=... \
RUSTWERK_JIRA_TEST_USERNAME=you@example.com \
RUSTWERK_JIRA_TEST_PROJECT=RUST \
cargo xtask test -- --ignored jira_live
```

Marked `#[ignore]` so `cargo test` never runs it by
default.

## How

- `crates/rustwerk-jira-plugin/tests/jira_live.rs`:
  single `#[test] #[ignore]` function as described.
- Shared teardown-guard lives in the same file (it's
  specific to this test).
- No changes to src/ code.

## Acceptance criteria

- [ ] Test skips cleanly (no error, no Jira call)
      when env vars are absent
- [ ] When env vars are set, test creates + deletes
      an issue and leaves no residue
- [ ] Teardown runs on panic (asserted via a
      deliberate-failure variant of the test)
- [ ] Test is `#[ignore]`d by default; never runs in
      `cargo xtask validate`
- [ ] Manual/README snippet shows how to run it

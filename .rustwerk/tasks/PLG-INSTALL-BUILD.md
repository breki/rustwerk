# PLG-INSTALL-BUILD: Build-from-source variants of `rustwerk plugin install`

## Why

PLG-INSTALL deliberately shipped the smallest useful
install flow: accept a path to a built cdylib. This
leaves a two-step workflow:

```
cargo build -p rustwerk-jira-plugin
rustwerk plugin install target/debug/rustwerk_jira_plugin.dll
```

The step-one cargo invocation is boilerplate, and the
artifact path is platform-specific
(`rustwerk_jira_plugin.dll` vs `.so` vs `.dylib`,
debug vs release). Folding the build into the install
command collapses it to one step and removes a class
of path-guessing mistakes.

## What

### Two new `SOURCE` shapes

Today `SOURCE` is a path to a built cdylib. Extend
the positional argument so it also accepts:

- **A Cargo project directory.** E.g.
  `rustwerk plugin install crates/rustwerk-jira-plugin/`
  — detect via `Cargo.toml` present, cdylib
  crate-type declared, build it, copy the artifact
  in, hand off to the existing install pipeline.

And add a new flag for the package-name variant:

- **`--from <package-name>`.** Look up the package in
  `cargo metadata` on the current workspace, assert
  the crate is a cdylib, build it, copy, install.
  Disjoint from passing a positional `SOURCE` (one or
  the other, not both — clap `conflicts_with`).

### Release profile flag

Add `--release` so users can install an optimised
build:

```
rustwerk plugin install --from rustwerk-jira-plugin --release
```

Default is debug to match the current single-step
dogfood flow.

### Build invocation

Run `cargo build --message-format=json
[--package <NAME>] [--release]` as a child process.
Parse the emitted JSON line stream for
`reason: "compiler-artifact"` records whose
`target.kind` contains `"cdylib"` and whose
`package_id` matches the requested package. The
`filenames` array on that record is the canonical
platform-correct artifact path — no guessing.

Propagate the child's stdout/stderr to the user so
compile errors stay visible. Exit with a targeted
error when:

- `cargo` is not on `PATH`;
- the named package doesn't exist in the workspace;
- the package exists but has no `cdylib` crate-type;
- the build completes but no cdylib artifact appears
  in the JSON stream (shouldn't happen if the
  crate-type check passed, but guard anyway).

### Handoff to existing pipeline

Once the cdylib path is known, the flow is identical
to PLG-INSTALL's path-to-cdylib case: reuse
`validate_cdylib_extension`, `install_from_path`,
`production_verify`. No duplication of copy / verify
/ rollback logic.

### Not in scope

- Installing from a crates.io package (`--from
  <name>@<version>`).
- Installing from a git URL.
- Cross-compilation (`--target <triple>`).
- Running `cargo clean` first.

## How

- `crates/rustwerk/src/bin/rustwerk/commands/plugin.rs`
  (or a new sibling — see AQ-087 for the module-split
  plan; doing this task at the same time as the split
  is a reasonable natural grouping):
  - Add `fn build_and_install(source_kind, scope,
    force, release)`.
  - `enum BuildSource { PackageName(String),
    ProjectDir(PathBuf) }`.
  - `fn run_cargo_build(spec: &BuildSource, release:
    bool) -> Result<PathBuf>` — runs the subprocess,
    parses the JSON stream, returns the cdylib path.
- `crates/rustwerk/src/bin/rustwerk/main.rs`: extend
  `PluginAction::Install` with `--from
  <package-name>` (`conflicts_with = "source"`) and
  `--release` flags.
- Manual + `llms.txt`: update to describe all three
  source shapes and the `--release` flag.

## Acceptance criteria

- [ ] `rustwerk plugin install crates/rustwerk-jira-plugin/`
      builds the crate and installs the resulting
      cdylib
- [ ] `rustwerk plugin install --from
      rustwerk-jira-plugin` does the same via
      workspace package name
- [ ] `--release` installs from `target/release/`
- [ ] `--from <not-in-workspace>` errors with a
      targeted "no such workspace member" message
- [ ] Passing a package without a `cdylib`
      crate-type errors with a targeted message
      (no wasted `cargo build`)
- [ ] Build failures propagate the child process's
      stderr to the user without re-wrapping
- [ ] After a successful build, the install flow
      reuses `install_from_path` (same copy / verify /
      rollback behavior as PLG-INSTALL)
- [ ] Unit tests for the JSON-stream parser (happy
      path, no matching artifact, multiple artifacts
      where only one is a cdylib)
- [ ] Manual and `llms.txt` updated
- [ ] `cargo xtask validate` passes

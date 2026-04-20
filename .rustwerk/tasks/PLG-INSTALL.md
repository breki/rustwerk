# PLG-INSTALL: `rustwerk plugin install` subcommand

## Why

Right now installing a plugin means: `cargo build -p
rustwerk-jira-plugin`, then hand-copy the resulting
`.dll` / `.so` / `.dylib` into `.rustwerk/plugins/`
with the correct filename. Fine for dogfooding, rough
for anyone else, and easy to botch on Windows (where
`target/debug/rustwerk_jira_plugin.dll` is what you
want but `target/debug/rustwerk-jira-plugin.dll` is
what you type).

## What

### CLI shape

```
rustwerk plugin install <SOURCE> [--scope <project|user>]
                                 [--force]
```

`SOURCE` is a path to an already-built cdylib
(`./target/debug/rustwerk_jira_plugin.dll`).

`--scope project` (default) installs into
`./.rustwerk/plugins/`; `--scope user` installs into
`$HOME/.rustwerk/plugins/` (or `%USERPROFILE%\...`).

`--force` overwrites an existing plugin of the same
filename.

### Deliberately out of scope

- **Building from source.** `--from <package-name>`
  and "source is a Cargo project directory" are
  deferred to a follow-up task (`PLG-INSTALL-BUILD`,
  c=3) because they require a `cargo build`
  subprocess + JSON artifact-path parsing and would
  push this task past its c=2 rating. Today's
  workflow stays two-step:
  `cargo build -p <plugin-crate> && rustwerk plugin install <path>`.
- **Uninstall** (`rustwerk plugin uninstall <NAME>`):
  trivial once install lands; separate task.

### Safety

- Reject sources whose basename doesn't end in the
  host OS's dynamic-library extension.
- After copying, load the plugin via the same path
  `plugin list` uses, call `rustwerk_plugin_info`, and
  print the resolved name/version/capabilities.
- If post-copy verification fails, delete the copy
  and return a clear error — never leave a half-
  installed plugin behind.

## How

- `crates/rustwerk/src/bin/rustwerk/commands/plugin.rs`:
  add `cmd_plugin_install`, a pure
  `install_from_path(source, dest_dir, force, verify)`
  factored so the verifier can be faked in tests, and
  a pure `resolve_scope_dir(scope, project_root, home)`.
- `crates/rustwerk/src/bin/rustwerk/plugin_host.rs`:
  promote `DYLIB_EXT`, `load_plugin`, and `home_dir`
  to `pub(crate)` so `commands::plugin` can reuse
  them without duplicating the platform logic.
- `crates/rustwerk/src/bin/rustwerk/main.rs`: add an
  `Install { source, scope, force }` variant to
  `PluginAction` plus a clap `ValueEnum` `InstallScope`.

## Acceptance criteria

- [ ] `rustwerk plugin install ./target/debug/rustwerk_jira_plugin.dll`
      copies the file into `.rustwerk/plugins/` and
      prints the discovered plugin info
- [ ] Wrong-extension source returns a clear error
      without touching the destination directory
- [ ] Failed post-copy verification removes the
      partially-installed file (unit-tested via a
      verifier fake)
- [ ] `--scope user` writes to the user directory
      (`$HOME/.rustwerk/plugins/` or
      `%USERPROFILE%\.rustwerk\plugins\`)
- [ ] `--force` replaces an existing install;
      without `--force`, conflict is a clear error
- [ ] Manual and `llms.txt` updated
- [ ] `cargo xtask validate` passes

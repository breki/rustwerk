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

Where `SOURCE` is one of:

- a path to an already-built cdylib
  (`./target/debug/rustwerk_jira_plugin.dll`),
- a path to a Cargo project dir containing a cdylib
  crate (build it first, then copy),
- `--from <cargo-package-name>` to build a workspace
  member by package name.

`--scope project` (default) installs into
`./.rustwerk/plugins/`; `--scope user` installs into
`$HOME/.rustwerk/plugins/` (or `%USERPROFILE%\...`).

`--force` overwrites an existing plugin of the same
name.

### Safety

- Reject sources whose basename doesn't look like a
  dynamic library (wrong extension for the host OS).
- After copying, load the plugin via the same path
  `plugin list` uses, call `rustwerk_plugin_info`, and
  print the resolved name/version/capabilities — the
  same thing `plugin list` would print. If that fails,
  delete the copy and return a clear error.

### Uninstall

Out of scope for this task; a follow-up
`plugin uninstall <NAME>` is trivial once this lands
and should be a separate small task.

## How

- `crates/rustwerk/src/bin/rustwerk/commands/plugin.rs`:
  add an `Install { source, scope, force }` variant to
  the existing `plugin` subcommand enum.
- Reuse `plugin_host::discover_plugins` for the
  post-copy verification step.
- Platform-specific extension resolution lives in a
  small helper
  (`#[cfg(windows)] "dll"`, `#[cfg(unix)] "so"` /
  `"dylib"`).

## Acceptance criteria

- [ ] `rustwerk plugin install ./target/debug/rustwerk_jira_plugin.dll`
      copies the file into `.rustwerk/plugins/` and
      prints the discovered plugin info
- [ ] Wrong-extension source returns a clear error
- [ ] Failed post-copy verification removes the
      partially-installed file
- [ ] `--scope user` writes to the user directory
- [ ] `--force` replaces an existing install;
      without `--force`, conflict is a clear error
- [ ] Manual and `llms.txt` updated to reflect the
      new subcommand
- [ ] `cargo xtask validate` passes

# PLG-CLI: CLI plugin list and push subcommands

## Why

Users need CLI commands to interact with plugins:
discover what's installed and trigger operations like
pushing tasks to Jira.

## What

### New subcommand: `rustwerk plugin`

Add `Plugin` variant to the `Commands` enum in
`main.rs` with two actions:

#### `rustwerk plugin list`

- Calls `discover_plugins()`
- Prints each plugin's name, version, description,
  and capabilities
- Empty list if no plugins found (not an error)

#### `rustwerk plugin push <name> [OPTIONS]`

- `<name>`: plugin name (e.g. "jira")
- `--tasks <IDS>`: comma-separated task IDs to push
  (all tasks if omitted)
- `--project-key <KEY>`: external project key (e.g.
  Jira project "PROJ")
- `--dry-run`: show what would be pushed without
  calling the plugin

### Config assembly

The host assembles a JSON config object from multiple
sources and passes it to the plugin:

```json
{
  "jira_url": "<from JIRA_URL env>",
  "jira_token": "<from JIRA_TOKEN env>",
  "username": "<from git config user.email>",
  "project_key": "<from --project-key arg>"
}
```

The host is generic — it does not know what each
plugin needs. It collects all available config and
the plugin picks what it uses.

Optional: `.rustwerk/plugin-config.json` for
persistent per-plugin settings.

### Task conversion

Convert selected `Task` objects to `TaskDto` using
the pattern from `ai/wbs_schema.rs::export_from_project`.

### Output

Print per-task results from `PluginResult`:
- Success: task ID + external key (e.g. "PLG-API ->
  PROJ-42")
- Failure: task ID + error message

## How

- New file: `src/bin/rustwerk/commands/plugin.rs`
- Modify `main.rs`: add `Plugin` command variant
- Modify `commands/mod.rs`: export plugin module
- Feature-gated behind `#[cfg(feature = "plugins")]`

## Acceptance criteria

- [ ] `rustwerk plugin list` works with 0 plugins
- [ ] `rustwerk plugin list` shows discovered plugins
- [ ] `rustwerk plugin push jira --project-key X`
      converts tasks and calls the plugin
- [ ] `--tasks` filter works correctly
- [ ] `--dry-run` shows tasks without calling plugin
- [ ] Helpful error if named plugin not found
- [ ] `cargo xtask clippy` passes

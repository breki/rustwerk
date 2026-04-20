# PLG-API-STATE: Per-task plugin-state round-trip in the plugin API

## Why

PLG-JIRA-UPDATE needs to know whether a task has
already been pushed so it can `PUT` instead of `POST`.
Without a place to persist that knowledge, every push
creates a duplicate issue.

Rustwerk's domain layer must stay agnostic about any
particular plugin. This task introduces an opaque,
plugin-namespaced per-task state bag that is stored in
`project.json` and round-tripped through the plugin API
— the host never interprets it; plugins own the shape.

## What

### TaskDto input

Add an optional per-task state field that the host
populates from `project.json` before calling
`rustwerk_plugin_push_tasks`:

```rust
pub struct TaskDto {
    // ... existing fields ...
    /// Opaque state previously returned by THIS
    /// plugin for THIS task. `None` on first push.
    pub plugin_state: Option<serde_json::Value>,
}
```

The host reads `task.plugin_state[<plugin-name>]` from
`project.json` and passes it in as `plugin_state`.

### Push response output

Each `PushResultEntry` grows an optional updated state
blob the host writes back:

```rust
pub struct PushResultEntry {
    pub task_id: String,
    pub outcome: PushOutcome,
    /// Updated opaque state. `None` means "leave the
    /// stored state unchanged". Explicit `Null` clears
    /// the entry.
    pub plugin_state_update: Option<serde_json::Value>,
}
```

### Host wiring

- `file_store` persists `plugin_state: { "<name>":
  <opaque JSON> }` per task in `project.json`.
- `plugin_host` merges `plugin_state_update` into the
  task's `plugin_state[<plugin-name>]` after a
  successful push, writes atomically.
- API version bumped: `API_VERSION = 2`. Host rejects
  v1 plugins with a clear "plugin needs rebuild against
  API v2" error.

## How

- `crates/rustwerk-plugin-api/src/lib.rs`: add fields,
  bump `API_VERSION`.
- `crates/rustwerk/src/domain/task.rs`: store opaque
  `plugin_state: BTreeMap<String, Value>` per task
  (default empty).
- `crates/rustwerk/src/persistence/file_store.rs`:
  serialize/deserialize the new field; ensure atomic
  write semantics still hold.
- `crates/rustwerk/src/bin/rustwerk/plugin_host.rs`:
  slice per-plugin state into/out of each `TaskDto`
  and merge the update back.
- Existing plugins without a response update are
  unaffected (`plugin_state_update: None`).

## Acceptance criteria

- [ ] `API_VERSION` bumped to 2; host rejects v1
      plugins with actionable error
- [ ] `TaskDto.plugin_state` round-trips through
      project.json
- [ ] Per-plugin namespacing: plugin "jira" cannot
      read or clobber plugin "github"'s state
- [ ] Atomic write preserved (no partial
      `plugin_state` on crash)
- [ ] Task rename / delete correctly moves / drops
      associated `plugin_state` entries
- [ ] Unit tests for serialization round-trip,
      namespacing isolation, and the merge semantics
      of `None` vs explicit `Null`
- [ ] `cargo xtask validate` passes

# PLG-JIRA-PARENT: Parent / epic linking for Jira push

## Why

A Jira WBS is hierarchical: Epics group stories and
tasks, parent tasks own sub-tasks, the "Epic Link"
field wires a story to its epic. Rustwerk tracks
tasks and dependencies but has no explicit
parent-child relation today — the WBS tree view
renders it by inferring hierarchy from ID prefixes
(`PLG-JIRA` → `PLG-JIRA-STATE`), which is display-
only.

To auto-create a Jira WBS, rustwerk needs:

1. a first-class `parent: Option<TaskId>` on `Task`,
2. a two-phase push that resolves parent keys from
   `plugin_state.jira.key` of the parent, and
3. payload emission of `parent.key` (for modern
   Jira, which uses `parent` for *all* hierarchy
   including the epic link since 2022) on children.

The PLG-API-STATE + PLG-JIRA-STATE machinery already
stores the necessary state blob per task; nothing
currently consumes it *across* tasks within a single
push. This task wires that consumer up.

## What

### Domain: explicit parent edge

```rust
pub struct Task {
    // ... existing fields ...
    pub parent: Option<TaskId>,
}
```

Constraints (validated at `project.json` load):

- A task cannot be its own parent.
- The parent must exist in the project.
- Parent edges form a forest (no cycles) — distinct
  from the DAG of `dependencies`.

Not added: a generalized tree query API. A minimal
`parents_first_push_order(&[TaskId])` topological
sort on the parent edge is sufficient for this task.

### Push ordering

`cmd_plugin_push` today builds a flat list of tasks
to send. After this task, it:

1. Topologically sorts the selected tasks so every
   parent precedes its children.
2. Invokes the plugin once per *level* — not once
   per task — so parents' state is persisted before
   children are processed.
3. Between levels, the host re-reads `project.json`
   so each child's `TaskDto.plugin_state` carries
   the state the parent just wrote.

Rationale for level-by-level instead of one mega-
batch: a single plugin invocation is atomic on the
plugin side but the *state writes* currently happen
after the call returns. Splitting by level lets the
host round-trip state through `project.json` before
the next level sees its parents.

### DTO additions

`TaskDto` gains an optional resolved-parent field
populated by the host:

```rust
pub struct TaskDto {
    // ...
    /// Parent task's plugin state (for the plugin
    /// currently being invoked), if the parent has
    /// been pushed before this call. `None` for
    /// root tasks and for tasks whose parent has
    /// never been pushed.
    pub parent_plugin_state: Option<serde_json::Value>,
}
```

Deliberately opaque: the plugin extracts
`parent_plugin_state.get("key")` exactly the same
way it reads its own `plugin_state.key`. The host
never peeks inside.

### Jira mapping

`build_issue_payload` emits:

```json
"fields": {
  "parent": { "key": "<parent_jira_key>" },
  ...
}
```

…when `parent_plugin_state.key` is present AND valid
per `IssueKey::parse`. If the parent is present but
the key is invalid or missing, emit a warning into
the per-task message (same pattern as RT-118's
parse-warning) and skip the parent field — creating
an orphan issue is better than failing the push.

### Modern-vs-legacy Jira

Modern Jira (post-2022) uses `parent.key` for
everything including epic linking. Legacy sites used
a custom field (`customfield_10014`) for the epic
link. Default to modern behavior; add
`epic_link_custom_field: Option<String>` to
`JiraConfig` for legacy sites. Keep the scope: if
the field is configured, the plugin sets both
`parent.key` (for sub-tasks) *and* the custom field
(for epic link). If unset, only `parent.key`.

### Not in scope

- Changing a task's parent after initial push.
- Moving issues between epics Jira-side.
- Multi-level hierarchies beyond
  Epic→Story/Task→Sub-task (Jira itself doesn't
  allow deeper).
- Verification that `parent.issue_type` is
  Epic-or-higher than `child.issue_type` — the
  plugin emits whatever rustwerk says; Jira rejects
  invalid combinations.

## How

- `crates/rustwerk/src/domain/task.rs`: add
  `parent: Option<TaskId>` field. Validate at load:
  no self-parent, no cycle, parent exists. Add
  `parents_first_push_order` helper that layers
  tasks by parent depth.
- `crates/rustwerk-plugin-api/src/lib.rs`: add
  `parent_plugin_state: Option<Value>` to
  `TaskDto`. No API-version bump *if*
  PLG-JIRA-ISSUETYPE already bumped to v3 —
  otherwise bump here.
- `crates/rustwerk/src/bin/rustwerk/commands/plugin.rs`:
  split `cmd_plugin_push`'s single plugin
  invocation into per-level invocations, with a
  `project.json` re-read between levels.
- `crates/rustwerk/src/bin/rustwerk/commands/task.rs`:
  `task add --parent <ID>`, `task update --parent <ID>`,
  `task unparent <ID>`.
- `crates/rustwerk-jira-plugin/src/mapping.rs`: emit
  `fields.parent.key` (plus optional legacy
  custom-field) from `parent_plugin_state`.

## Acceptance criteria

- [ ] `task add --parent PROJ-EPIC` records the
      parent edge; cycles are rejected at load time
- [ ] WBS display (`task list`, `rustwerk tree`)
      shows parent-child hierarchy from the
      explicit edge, not ID-prefix inference
- [ ] First `plugin push jira` on a project with
      an Epic parent + two Task children creates
      three Jira issues in the right order and the
      children's `fields.parent.key` is the epic's
      Jira key
- [ ] Second push updates (per PLG-JIRA-UPDATE)
      and re-emits the parent field idempotently
- [ ] If an epic hasn't been pushed yet and a
      child task is pushed alone, the child's
      task message contains a warning but the push
      succeeds (orphan, not a hard failure)
- [ ] Legacy `epic_link_custom_field` config
      correctly writes both `parent.key` and the
      custom field
- [ ] Unit tests for: level ordering, missing-parent
      warning path, legacy-field dual emit, cycle
      rejection at load
- [ ] `cargo xtask validate` passes

# PLG-JIRA-ISSUETYPE: Per-task Jira issue type

## Why

`mapping::build_issue_payload` currently hardcodes
`"issuetype": { "name": "Task" }`. That makes the
plugin useless for any real WBS that distinguishes
Epics, user stories, tasks, and sub-tasks — which is
the common Jira workflow and the scenario the plugin
needs to support before anyone can migrate a real
project.

PLG-JIRA-ISSUETYPE is deliberately scoped *narrowly*
to issue-type selection. Parent/epic linking — which
the issue type implies — lands in PLG-JIRA-PARENT.
Keeping them separate means PLG-JIRA-ISSUETYPE can
ship and be useful (flat projects with mixed Tasks
and Stories) while the harder hierarchy piece is
still cooking.

## What

### Domain change

Add an optional enum field to `Task`:

```rust
pub enum IssueType {
    Epic,
    Story,
    Task,
    SubTask,
}

pub struct Task {
    // ... existing fields ...
    pub issue_type: Option<IssueType>,
}
```

`None` → the plugin falls back to the config-level
default (`jira_default_issue_type`, see below) or
`"Task"` if no default is configured.

### Plugin config additions

`JiraConfig` gains two optional fields, both
defaulted:

```json
{
  "default_issue_type": "Task",
  "issue_type_map": {
    "epic":     "Epic",
    "story":    "Story",
    "task":     "Task",
    "sub-task": "Sub-task"
  }
}
```

The `issue_type_map` exists because some Jira sites
rename "Sub-task" to "Subtask" (no hyphen) or localize
names. The map is `IssueType` (rustwerk enum) →
string-as-Jira-sees-it. Omitted keys fall through to
the default set above.

### Mapping change

`build_issue_payload` takes `&JiraConfig` (was just
`&cfg.project_key`) and resolves per task:

1. `task.issue_type.map(|t| cfg.resolve_issue_type(t))`
2. else `cfg.default_issue_type.as_deref()`
3. else `"Task"`

The result lands in `fields.issuetype.name`. The
existing `"issuetype": {"name": "Task"}` literal is
deleted.

### CLI

`task add --type <epic|story|task|sub-task>` and
`task update --type …`. Rendered in `task list` via
a prefix marker (`E:`, `S:`, `T:`, `s:`) so the WBS
view shows type at a glance. Display in the domain
layer; no Jira-specific knowledge leaks out of the
plugin.

### TaskDto

Wire a `issue_type: Option<String>` field (stringly-
typed at the DTO boundary so future types don't
break API v3). The plugin maps it back to its
internal `IssueType` via the same map as the config.

### Not in scope

- Parent/epic linking (that's PLG-JIRA-PARENT).
- Custom issue types beyond the four listed —
  future work.
- Changing the issue type of an already-pushed
  issue — Jira's `PUT /issue/{key}` with a
  different `issuetype` is tricky (requires a
  workflow that allows it) and out of scope; first
  push wins.
- Discovering the issue-type IDs dynamically via
  `GET /issue/createmeta`. Mapping by name is
  sufficient for the MVP.

## How

- `crates/rustwerk/src/domain/task.rs`: add
  `issue_type: Option<IssueType>` to `Task`; add
  `IssueType` enum with `Serialize` /
  `Deserialize` using kebab-case tags.
- `crates/rustwerk-plugin-api/src/lib.rs`: add
  `issue_type: Option<String>` to `TaskDto`. Bump
  `API_VERSION` to 3. Host rejects v2 plugins with
  a "plugin needs rebuild against API v3" error.
- `crates/rustwerk/src/bin/rustwerk/plugin_host.rs`:
  `task_to_dto` populates `issue_type` from the
  domain `Task`.
- `crates/rustwerk/src/bin/rustwerk/commands/task.rs`:
  wire the `--type` flag on `task add` and
  `task update`.
- `crates/rustwerk-jira-plugin/src/config.rs`: add
  `default_issue_type` + `issue_type_map` fields
  (both `Option`-bearing, default-sane).
- `crates/rustwerk-jira-plugin/src/mapping.rs`:
  change `build_issue_payload` to take
  `&JiraConfig` and resolve the Jira issue-type
  name per task.

## Acceptance criteria

- [ ] `task add --type epic` and `task update --type story`
      round-trip through `project.json` and
      `task describe`
- [ ] Plugin payload's `fields.issuetype.name`
      reflects the per-task type when set, and
      falls through to
      `config.default_issue_type` or `"Task"`
      otherwise
- [ ] Pushing a task with
      `issue_type = Some(IssueType::Epic)` to a
      real (or mocked) Jira creates an Epic issue
      (verified against both an empty map — uses
      default `"Epic"` — and a remapped site where
      the map overrides the wire name)
- [ ] API version bumped to 3; host rejects v2
      plugins with a clear message
- [ ] Unit tests for mapping: each of the four
      types, default fallback, map override, no
      config default
- [ ] `cargo xtask validate` passes

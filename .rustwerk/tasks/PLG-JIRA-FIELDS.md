# PLG-JIRA-FIELDS: Richer Jira field mapping

## Why

PLG-MAP shipped the minimum viable payload: project +
summary + ADF description + `issuetype: "Task"`.
Rustwerk tracks status, assignee, complexity, and
tags; none of it reaches Jira. For the plugin to be
useful beyond demo-ware, those fields have to land on
the issue.

The reason this is one task, not four, is that status
/ assignee / priority / labels share the same
infrastructure concern: a per-plugin config that maps
rustwerk concepts to Jira IDs (workflow transitions,
accountIds, priority scheme).

## What

### Config additions

Added to the jira-plugin config JSON passed in by the
host:

```json
{
  "status_map": {
    "todo":        null,
    "in_progress": "11",
    "done":        "31"
  },
  "assignee_map": {
    "igor.brejc@wagalabs.com": "712020:abc-def-..."
  },
  "priority_map": { "1": "Highest", "3": "Medium", "5": "Low" },
  "labels_from_tags": true
}
```

All four fields are optional; omitting any one skips
that field entirely in the payload.

### Field mapping

| rustwerk source        | Jira target                         | Notes                                              |
|------------------------|-------------------------------------|----------------------------------------------------|
| `task.status`          | `transitions[].id` (after create)   | via `POST /issue/{key}/transitions` if mapped      |
| `task.assignee`        | `fields.assignee.accountId`         | looked up via `assignee_map`; silently skip if miss|
| `task.complexity`      | `fields.priority.name`              | mapped via `priority_map`; skip if miss            |
| `task.tags`            | `fields.labels`                     | only if `labels_from_tags: true`                   |

### Status is a post-create transition

Jira's REST API rejects `status` on create; it must
be applied via a second call, `POST
/rest/api/3/issue/{key}/transitions { "transition": { "id": "<id>" }}`.
PLG-JIRA-UPDATE's `PUT` path also needs to detect a
status change and emit a transition.

### Not in scope

- Workflow auto-discovery (`GET /issue/{key}/transitions`
  to learn IDs dynamically).
- Custom-field mapping.
- Components.

## How

- `crates/rustwerk-jira-plugin/src/config.rs`: extend
  `JiraConfig` with four optional fields; validate
  `assignee_map` keys are emails.
- `crates/rustwerk-jira-plugin/src/mapping.rs`: take
  a `&JiraConfig` (currently takes only `project_key`)
  and emit the additional `fields.*` entries only when
  mapped.
- `crates/rustwerk-jira-plugin/src/jira_client.rs`:
  add `transition(key, transition_id)` with gateway
  fallback.
- Update PLG-JIRA-STATE-style response to include
  transition result in `plugin_state_update` if a
  transition was attempted.

## Acceptance criteria

- [ ] Each mapped field appears only when its map is
      present AND has a matching entry; unknown keys
      are silently skipped with a warning in the push
      response
- [ ] Status transitions happen after create/update,
      not inline on the fields payload
- [ ] `labels_from_tags: false` (the default) omits
      `fields.labels` entirely
- [ ] Unit tests for each mapping branch (present,
      missing-entry, map-absent) and for the transition
      dispatch
- [ ] `cargo xtask validate` passes

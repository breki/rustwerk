# PLG-JIRA-STATE: Remember the Jira issue key per task

## Why

PLG-API-STATE gives plugins a place to stash per-task
state. This task teaches the jira plugin to use it:
record the returned Jira issue key on the first
successful create, so a later push can know the task
has already been mirrored. It's the minimum memory
required before PLG-JIRA-UPDATE becomes meaningful.

## What

### State shape

Plugin-owned, stored by the host under
`plugin_state.jira`:

```json
{
  "key":        "RUST-142",
  "self":       "https://.../rest/api/3/issue/10042",
  "last_pushed_at": "2026-04-22T09:14:07Z"
}
```

Fields are additive; future iterations may record
`last_hash`, `last_response_etag`, etc.

### Behavior

- On a successful `POST /rest/api/3/issue`, the plugin
  returns
  `PushResultEntry { plugin_state_update: Some(json!({...})), .. }`.
- On a failure, `plugin_state_update` stays `None` —
  the host leaves the previously stored state
  untouched.
- On input, the plugin reads `task.plugin_state` but
  **does not yet act on it** — this task only writes.
  Reading/acting lands in PLG-JIRA-UPDATE.

### Not in scope

- Using the stored state to decide create-vs-update
  (that's PLG-JIRA-UPDATE).
- Retrying a failed push based on state (future work).

## How

- `crates/rustwerk-jira-plugin/src/lib.rs`: after
  `JiraClient::create_issue` succeeds, build the state
  object from the response (`id`, `key`, `self`) and
  attach it to the per-task result.
- `crates/rustwerk-jira-plugin/src/jira_client.rs`:
  parse the existing issue-creation response into a
  typed `CreatedIssue { id, key, self_url }` so the
  state blob isn't constructed from raw JSON.

## Acceptance criteria

- [ ] First push of a fresh task stores
      `{ "key": "<KEY>", "self": "<URL>", "last_pushed_at": "<ISO8601>" }`
      in `project.json` under `plugin_state.jira`
- [ ] Unit tests cover: successful create emits
      `plugin_state_update`; failed create emits
      `None`; `last_pushed_at` uses UTC ISO-8601
- [ ] `cargo xtask validate` passes

# PLG-JIRA-UPDATE: Idempotent push — update existing issues

## Why

Today `rustwerk plugin push jira` is create-only: run
it twice and every task produces two Jira issues. With
PLG-JIRA-STATE in place the plugin now knows which
tasks have been pushed; this task closes the loop so
the second push updates the existing issue rather than
creating a new one.

## What

### Dispatch logic

For each `TaskDto`:

| Incoming `plugin_state.jira.key` | Jira response      | Action                 |
|----------------------------------|--------------------|------------------------|
| `None`                           | n/a                | `POST /issue` (create) |
| `Some(key)`                      | 200 on GET /issue/{key} | `PUT /issue/{key}` (update) |
| `Some(key)`                      | 404 on GET /issue/{key} | `POST /issue` (recreate); overwrite stored key |

The GET/404 probe covers the "issue deleted in Jira"
case and is cheap relative to a wasted `PUT`.

### Update payload

`PUT /rest/api/3/issue/{key}` sends only changed
fields. First pass can send everything PLG-MAP
currently emits (`summary`, ADF `description`); a
`last_hash` field on the stored state can short-circuit
no-op pushes later (not in scope here).

### State updates on update

- 200/204 from `PUT`: refresh `last_pushed_at`, keep
  key/self.
- 404 recreate path: overwrite the state blob with the
  new key/self (same shape as PLG-JIRA-STATE's initial
  write).
- Any non-2xx: leave state untouched (`None` update).

### Not in scope

- Change detection / diffing (`last_hash`).
- Conflict handling if Jira-side edits diverge from
  rustwerk.
- Deleting Jira issues when a rustwerk task is removed.

## How

- `crates/rustwerk-jira-plugin/src/jira_client.rs`:
  add `get_issue(key)` (with gateway fallback reused
  from create) and `update_issue(key, payload)`.
- `crates/rustwerk-jira-plugin/src/lib.rs`: branch on
  `task.plugin_state` inside `push_one`; thread the
  three outcomes through `plugin_state_update`.
- Unit tests use the `HttpClient` fake that
  PLG-JIRA already established.

## Acceptance criteria

- [ ] Second push of an already-pushed task sends
      `PUT /rest/api/3/issue/{key}`, not `POST`
- [ ] Second push of a task whose Jira issue was
      deleted in Jira recreates and overwrites state
- [ ] Failed `PUT` leaves the stored key/self
      unchanged
- [ ] Gateway fallback (cloud-id discovery) applies to
      GET and PUT paths, not only POST
- [ ] Unit tests for all three branches (create,
      update, recreate-on-404) and for state-update
      semantics on failure
- [ ] `cargo xtask validate` passes

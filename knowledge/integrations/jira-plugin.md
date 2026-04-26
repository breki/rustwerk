+++
title = "Jira plugin"
date = 2026-04-21
description = "Push rustwerk tasks into Jira Cloud as issues, with idempotent create/update, parent/epic linking, and status transitions."

[taxonomies]
tags = ["plugin", "jira"]

[extra]
note_type = "integration"
links = [
  { relation = "implements", target = "architecture/crate-jira-plugin" },
  { relation = "relates-to", target = "integrations/jira-field-mapping" },
  { relation = "relates-to", target = "integrations/jira-parent-epic" },
  { relation = "relates-to", target = "architecture/plugin-host" },
]
+++

`rustwerk plugin push jira` pushes a rustwerk project's
tasks into a Jira Cloud project as issues. The plugin
ships as a `cdylib` that the host loads at runtime — see
[Plugin host](@/architecture/plugin-host.md) and
[crate: rustwerk-jira-plugin](@/architecture/crate-jira-plugin.md).

The feature is *push-only* and *one-way*: rustwerk is
the source of truth, Jira is a projection. Nothing is
read back from Jira into the rustwerk project file.

## What a push actually does

For each task selected by the user the plugin:

1. Builds a Jira issue payload from the `TaskDto` (see
   [Jira field mapping](@/integrations/jira-field-mapping.md)).
2. Decides whether the task has been pushed before by
   reading `plugin_state.jira.key` on the task.
3. Either **creates** a new issue, or **probes** the
   stored key and then **updates** (or, under narrow
   conditions, **recreates**) it.
4. Fires an optional workflow transition so the Jira
   status lines up with the rustwerk status.
5. Hands back a per-task result plus a `plugin_state`
   patch the host persists under the task.

The whole batch is driven level-by-level by parent
depth so that epic / parent links always resolve
against issue keys created earlier in the same run —
see [Jira parent/epic linking](@/integrations/jira-parent-epic.md).

## The idempotency ladder

This is the part most worth internalizing. `push.rs`
dispatches on the incoming state:

| `plugin_state.jira.key` | Probe result (GET /issue/{key}) | Action |
|---|---|---|
| absent                      | —                              | `POST /issue` (create) |
| present, valid              | 2xx                            | `PUT  /issue/{key}` (update) |
| present, valid              | 404 on direct **and** gateway  | `POST /issue` (recreate, overwrite state) |
| present, valid              | 401 direct + 404 gateway       | **fail the task** (ambiguous — see below) |
| present, valid              | other non-2xx                  | fail the task with the response body |
| present, malformed          | —                              | fail the task |

Two non-obvious edges worth flagging:

- **Ambiguous missing.** If the direct URL is 401 but
  the gateway URL is 404, the plugin refuses to
  recreate (RT-122). That combination is not proof the
  issue is gone — it might be alive but unreadable by
  the current token. Silently recreating would
  duplicate a live issue.
- **Poisoned state key.** If `plugin_state.jira.key` is
  present but does not match Jira's issue-key grammar,
  the task fails loudly (RT-121) rather than being
  treated as "never pushed" — a corrupted project file
  must not be able to coerce a duplicate issue.

## The `plugin_state.jira` blob

After a successful push the plugin returns a small JSON
object which the host persists under the task:

```json
{
  "key": "PROJ-42",
  "self": "https://acme.atlassian.net/rest/api/3/issue/10042",
  "last_pushed_at": "2026-04-21T09:17:05Z"
}
```

Contract notes:

- `key` is the idempotency anchor for the update path
  and the parent-link source for children in the next
  level.
- On **create**, the blob is built from scratch from
  the Jira response.
- On **update**, Jira's `PUT` returns 204 (no body), so
  the blob is carried over verbatim — only
  `last_pushed_at` is refreshed. This preserves any
  additive fields a future plugin version may have
  written (RT-123).
- On a failed update the blob is **not** rewritten —
  `plugin_state_update: None` — so a transient
  failure does not drop the idempotency anchor.
- The timestamp is RFC-3339 UTC with seconds precision
  (single format authority in `format_last_pushed_at`)
  so stored state stays diff-stable across hosts.

## Configuration

The plugin receives a JSON config from the host
(assembled from project + user config). Fields:

| Field | Purpose |
|---|---|
| `jira_url` | Base URL of the Jira site. Must be `https://…​.atlassian.net`. |
| `jira_token` | Scoped API token (Basic-auth password). |
| `username` | Basic-auth username (typically the user's email). |
| `project_key` | Target Jira project key, e.g. `PROJ`. |
| `default_issue_type` | Fallback when a task has no `issue_type`. Defaults to `"Task"`. |
| `issue_type_map` | Overrides kebab rustwerk names → Jira-visible issue-type strings. |
| `status_map` | rustwerk status wire names → Jira workflow **transition IDs** (not status names). |
| `assignee_map` | rustwerk assignee (email) → Jira `accountId`. |
| `priority_map` | Complexity score (stringified int) → Jira priority **name**. |
| `labels_from_tags` | Opt-in forwarding of rustwerk tags to `fields.labels`. |
| `epic_link_custom_field` | Legacy `customfield_<digits>` for sites that predate modern `parent.key` epic linking. |

The loader (`config.rs`) is strict:

- Required fields (`jira_url`, `jira_token`, `username`,
  `project_key`) must be present and non-empty.
- `jira_url` is validated to be `https` *and* end in
  `.atlassian.net` — a misconfigured or hostile URL
  cannot redirect the Basic-auth token to a third
  party.
- `assignee_map` keys must contain `@` (typos become
  visible at load time rather than silently producing
  "no assignee" on every push).
- `epic_link_custom_field` must match `customfield_\d+`.
- Map keys are canonicalized on load: issue-type keys
  normalize `subtask` → `sub-task`; status-map keys
  are lowercased/trimmed. Config written as
  `"In_Progress"` still resolves.

## Security posture

- Credentials are **never logged**; `warnings.rs`
  collects non-fatal redaction notes that the host
  surfaces to the user.
- Host allowlist (`*.atlassian.net`) blocks token
  exfiltration via a swapped URL.
- Issue keys stored in `plugin_state` are validated
  before being spliced into a request URL — poisoned
  state cannot construct arbitrary endpoints.

## CLI surface

```bash
rustwerk plugin push jira \
    [--project-key <KEY>] \
    [--tasks <ID,ID,...>] \
    [--dry-run]
```

- `--tasks` filters which tasks are pushed; omitted →
  every task in the project.
- `--dry-run` builds payloads and reports the planned
  actions without hitting Jira.
- Exit status reflects whether *every* task succeeded;
  partial failures return non-zero while still
  committing the state updates for the successes.
- The host reloads `project.json` between levels so
  each child sees the Jira keys stamped for its parent
  in the prior level.

## Failure modes

- **Auth failure (401/403).** The HTTP client retries
  once via the optional gateway URL before surfacing
  the error to the host.
- **Workflow mismatch.** If the target Jira project's
  workflow does not accept the transition the plugin
  wants to apply, the issue is created/updated
  successfully but the status is left at the workflow
  default; the plugin records a warning.
- **Partial push.** A failure mid-run leaves already-
  created issues in place. Per-task results are
  returned so the host can persist state for the
  successes and a re-run will skip them via the
  update path.
- **Malformed Jira response.** A 2xx with an
  unparseable body on create does **not** silently
  drop — the warning is folded into the message so a
  follow-up push cannot create a duplicate issue
  against lost state.

## Opt-in testing

A live end-to-end smoke test exists under the
`jira-live` feature flag (see commit `c92f8e7`). It is
opt-in and requires real Jira credentials — the CI
suite runs only the mocked paths.

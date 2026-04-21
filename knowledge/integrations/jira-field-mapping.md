+++
title = "Jira field mapping"
date = 2026-04-21
description = "How rustwerk task fields become Jira issue fields."

[taxonomies]
tags = ["jira", "plugin"]

[extra]
note_type = "integration"
links = [
  { relation = "part-of", target = "integrations/jira-plugin" },
]
+++

`mapping.rs` in the Jira plugin owns the translation
from `Task` → Jira issue JSON. The shape is
intentionally flat and declarative.

| rustwerk | Jira |
|---|---|
| `Task.title` | `fields.summary` |
| `Task.description` | `fields.description` (ADF) |
| `Task.status` | workflow transition (see `transition.rs`) |
| `Task.assignee` | `fields.assignee.accountId` (resolved) |
| `Task.metadata.priority` | `fields.priority.name` |
| `Task.metadata.labels` | `fields.labels` |
| `Task.parent_id` | `fields.parent` or epic link |
| project default | `fields.issuetype.name` |
| per-task override | `Task.metadata.issue_type` |

## Issue-type resolution

Issue type is picked in order:

1. A `issue_type` entry on the task's `metadata`.
2. An entry in the config-level issue-type map keyed
   by task ID prefix (e.g. `BUG- → Bug`).
3. The project-wide default.

## Status transitions

Jira does not accept a status directly — it requires a
transition ID. `transition.rs` fetches the target
project's workflow at startup, caches a
`status name → transition id` map, and applies the
right transition after issue creation.

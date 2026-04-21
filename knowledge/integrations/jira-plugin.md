+++
title = "Jira plugin"
date = 2026-04-21
description = "Push rustwerk tasks into Jira Cloud as issues, linked by parent/epic."

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

`rustwerk plugin push jira` pushes a project's tasks
into a Jira Cloud project as issues. The plugin is a
`cdylib` that the host loads at runtime — see
[Plugin host](@/architecture/plugin-host.md).

## Configuration

The plugin reads a JSON config (`site_url`, project
key, credentials, issue-type map, status transitions,
defaults). Credentials are never logged; the plugin's
`warnings.rs` collects non-fatal redaction notes that
the host surfaces to the user.

## Failure modes

- **Auth failure (401/403).** The client retries once
  via the optional gateway URL before surfacing the
  error to the host.
- **Workflow mismatch.** If the target Jira project's
  workflow lacks a transition the plugin needs, the
  issue is created but its status is left as the
  workflow default; the host logs a warning.
- **Partial push.** A failure mid-run leaves already-
  created issues in place; the plugin returns
  per-task results so a re-run can skip them.

## Opt-in testing

A live end-to-end smoke test exists under the
`jira-live` feature flag (see commit `c92f8e7`). It is
opt-in and requires real Jira credentials.

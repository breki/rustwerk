+++
title = "Jira parent/epic linking"
date = 2026-04-21
description = "Level-by-level orchestration so children find their parents."

[taxonomies]
tags = ["jira", "plugin"]

[extra]
note_type = "integration"
links = [
  { relation = "part-of", target = "integrations/jira-plugin" },
  { relation = "relates-to", target = "architecture/crate-jira-plugin" },
]
+++

Jira requires that a parent or epic issue exist before
a child can link to it. rustwerk honors this by pushing
the WBS one **level** at a time:

1. Sort tasks by parent depth (`PushLevels` in the
   domain layer).
2. Push all level-0 tasks (no parent) first.
3. For each subsequent level, map rustwerk parent IDs
   to the Jira issue keys that were returned for the
   previous level.
4. Stamp each child with `fields.parent` (or the
   custom epic-link field, depending on project type).

This is why the plugin's `push.rs` is a small
orchestrator rather than a flat "for each task" loop.

## Why not a second pass?

A single topological pass with cached keys is simpler
and keeps the JSON round-trips proportional to the
task count rather than doubling them.

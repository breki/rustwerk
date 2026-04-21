+++
title = "Rustbase template lineage"
date = 2026-04-21
description = "Where rustwerk's build setup came from, and how upstream changes flow in."

[taxonomies]
tags = ["template", "tooling"]

[extra]
note_type = "integration"
links = [
  { relation = "relates-to", target = "architecture/xtask" },
  { relation = "relates-to", target = "architecture/workspace" },
]
+++

Rustwerk predates the
[rustbase](https://github.com/breki/rustbase) template
and was retroactively linked to it — the link lives in
`.template-sync.toml` at the repo root.

## Sync direction

Changes only flow **upstream → rustwerk**, never the
other way. The `/template-sync` slash command:

1. Fetches upstream rustbase.
2. Categorizes the diff (build, lints, docs, web).
3. Lets the user approve or skip each category.
4. Applies only the approved changes, preserving
   rustwerk customizations.

## What is always skipped

Rustbase serves projects that have a web frontend
(React + Playwright). Rustwerk is CLI-only, so web/e2e
template changes are skipped by default. Anything
build-related (xtask, lints, coverage thresholds) is
usually accepted.

## Feedback loop

When a template-provided file feels suboptimal,
`/template-improve` writes a note into
`docs/developer/template-feedback.md`. That file is
the mechanism by which rustwerk's experience feeds
back into the template for future projects.

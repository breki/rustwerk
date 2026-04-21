+++
title = "Developer"
date = 2026-04-21
description = "Who does the work — the subject of assignments and effort."

[taxonomies]
tags = ["domain"]

[extra]
note_type = "concept"
links = [
  { relation = "relates-to", target = "concepts/task" },
  { relation = "relates-to", target = "concepts/effort" },
]
+++

A **Developer** is an identity that tasks are assigned
to and effort is logged against. They are registered
once with `rustwerk dev add` and referenced everywhere
by `DeveloperId`.

## Implicit defaults

When a CLI command needs a developer but the user did
not pass `--dev`, the binary falls back to the
`RUSTWERK_USER` environment variable. This is what
makes `rustwerk effort log TASK 2H` pleasant in
interactive use — the shell holds the context for
you.

Batch files and plugin invocations must pass developers
explicitly; they must not rely on `RUSTWERK_USER`.

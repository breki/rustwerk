+++
title = "Architecture"
sort_by = "title"
description = "Workspace, crates, modules, and build plumbing."
+++

RustWerk is a Cargo workspace with three production
crates plus an `xtask` build runner. The main binary
(`rustwerk`) is composed of a pure **domain** layer, a
thin **persistence** layer, and a **CLI** layer that
wires them to `clap`-driven subcommands. Optional
behavior (currently: push tasks to Jira) is supplied by
**dynamic library plugins** loaded at runtime.

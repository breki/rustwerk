+++
title = "Ship a single portable binary"
date = 2026-04-21
description = "No runtime deps, no daemon, no package manager."

[taxonomies]
tags = ["distribution", "ergonomics"]

[extra]
note_type = "decision"
links = [
  { relation = "relates-to", target = "decisions/git-native-json-store" },
  { relation = "relates-to", target = "architecture/crate-rustwerk" },
]
+++

**Decision.** `rustwerk` is one statically-linked
executable per target platform. No Python, no node, no
JVM, no system services.

## Why

- **Agent friction.** An agent that wants to use
  rustwerk in a sandbox should be able to copy one
  binary and go. Anything else — `pip install`, SDK
  configs, auth flows — raises the odds the agent
  gives up or shells out to the web UI.
- **Windows parity.** A dependency on POSIX shell or
  bash shims would make Windows second-class. Rust
  plus `clap` gets to parity without `cmd.exe`
  gymnastics.
- **Version pinning.** A project's `.rustwerk/` folder
  can name the binary version it was built against
  (`tool_version` field in the project file) so old
  projects still load correctly.

## Cost

Plugins break the "single binary" promise, which is
why they are opt-in (`--features plugins`) and scoped
narrowly to external integrations (Jira, ...).

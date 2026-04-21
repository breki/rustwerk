+++
title = "crate: rustwerk"
date = 2026-04-21
description = "The main binary plus its embedded domain library."

[taxonomies]
tags = ["crate", "cli", "domain"]

[extra]
note_type = "architecture"
links = [
  { relation = "part-of", target = "architecture/workspace" },
  { relation = "depends-on", target = "architecture/crate-plugin-api" },
  { relation = "relates-to", target = "architecture/domain-layer" },
  { relation = "relates-to", target = "architecture/cli-layer" },
  { relation = "relates-to", target = "architecture/persistence-layer" },
  { relation = "relates-to", target = "architecture/plugin-host" },
]
+++

`crates/rustwerk` ships both a library (`rustwerk::`)
and the `rustwerk` binary. The library has three
mutually-independent layers:

```
src/
├── domain/        # pure model + invariants, no I/O
├── persistence/   # JSON (de)serialization, file_store
├── ai/            # WBS JSON schema for agent import
└── bin/rustwerk/  # CLI: clap parser, commands, render
```

## Why library + binary

Putting the domain in `lib.rs` lets integration tests
and future embedders (e.g. a TUI, a daemon) depend on
the same types the CLI uses, without having to shell
out. The binary only owns CLI-specific code: argument
parsing, rendering, and the plugin host.

## Feature flags

- `plugins` (default) — pulls in `libloading` and
  enables the dynamic-library plugin system. See
  [plugin host](@/architecture/plugin-host.md).

Disabling `plugins` yields a smaller binary with no
`unsafe_code` at all, useful for embedded or
restricted-sandbox deployments.

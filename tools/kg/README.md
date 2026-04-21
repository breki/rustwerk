# RustWerk Knowledge Graph

A browsable map of rustwerk's architecture, domain
concepts, design decisions, and external integrations.

Authored as markdown notes under `knowledge/`, rendered
to a static website by [Zola](https://www.getzola.org/).

## Layout

```
knowledge/                   # authoring root (checked in)
  _index.md                  # KG home
  architecture/              # crates, modules, build plumbing
  concepts/                  # domain vocabulary (task, WBS, ...)
  decisions/                 # design decisions and their rationale
  integrations/              # external systems (Jira, rustbase, ...)
tools/kg/
  bin/                       # [generated] auto-downloaded zola binary
  site/                      # Zola site (templates, sass, config)
    content/                 # [generated] staged copy of knowledge/
    public/                  # [generated] built site
  scripts/
    kg-build.{sh,ps1}        # thin wrapper → `cargo xtask kg build`
    kg-serve.{sh,ps1}        # thin wrapper → `cargo xtask kg serve`
    kg-new.sh                # scaffold a new note
    kg-validate.sh           # check that link targets resolve
    kg-stats.sh              # note + link counts
```

The `bin/`, `content/`, and `public/` directories are
build artifacts and are git-ignored.

## Note format

Each note is a markdown file with TOML frontmatter that
Zola reads natively — there is no custom transform step.

```toml
+++
title = "Task"
date = 2026-04-21
description = "The atomic unit of work in a rustwerk project."

[taxonomies]
tags = ["domain", "core"]

[extra]
note_type = "concept"
links = [
  { relation = "part-of",   target = "concepts/wbs" },
  { relation = "relates-to", target = "concepts/dependencies" },
]
+++

Body content in markdown...
```

### Node types (`extra.note_type`)

| Type           | Purpose                                      |
|----------------|----------------------------------------------|
| `concept`      | A domain idea, model, or vocabulary term     |
| `architecture` | A crate, module, or structural component     |
| `decision`     | A design choice and its rationale            |
| `integration`  | An external system or boundary               |
| `reference`    | Pointer to external docs or source           |

### Edge types (`extra.links[].relation`)

| Relation          | Meaning                                |
|-------------------|----------------------------------------|
| `part-of`         | This note is a component of the target |
| `depends-on`      | This note requires the target          |
| `implements`      | This note realizes the target's idea   |
| `relates-to`      | General association                    |
| `derived-from`    | Source or inspiration                  |
| `contradicts`     | Tension between ideas                  |

The `target` is a path relative to `knowledge/`, without
the `.md` extension (e.g. `concepts/task`).

## Usage

Build and serve flow through **xtask**, matching the
rest of the project's build policy (CLAUDE.md: never
use raw cargo — always go through xtask).

```bash
# Build / serve — xtask owns the logic
cargo xtask kg build
cargo xtask kg serve                         # http://localhost:1111
cargo xtask kg serve -- --port 8080 --open   # forward args to zola serve

# Thin wrappers exist for discoverability / shell completion:
tools/kg/scripts/kg-build.sh                 # or .ps1 on Windows
tools/kg/scripts/kg-serve.sh                 # or .ps1 on Windows

# Authoring helpers (pure shell — no zola involved)
tools/kg/scripts/kg-new.sh concepts "Critical Path" concept wbs,scheduling
tools/kg/scripts/kg-validate.sh              # check outgoing link targets
tools/kg/scripts/kg-stats.sh                 # note + link counts
```

## Zola

Xtask resolves zola in this order:

1. `zola` on `PATH` (whatever version is installed).
2. The vendored copy at `tools/kg/bin/zola[.exe]`.
3. Download the pinned release into that location.

No manual install is required — the first build
auto-fetches zola from GitHub Releases. The pinned
version lives in `xtask/src/kg.rs` (`ZOLA_VERSION`).

## Why no custom binary?

Authoring notes in Zola-native frontmatter lets Zola do
100% of the rendering work — no intermediate
transformation step, no extra Rust binary to build and
maintain. The typed edge semantics (`extra.links` with
`relation` + `target`) are interpreted entirely by the
Tera templates in `site/templates/`.

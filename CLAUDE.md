**IMPORTANT: The working directory is already set to the
project root. NEVER use `cd` to the project root or
`git -C <dir>` — blanket permission rules cannot be
set for commands starting with `cd` or `git -C`, so
they require manual approval every time.**

# RustWerk

Git-native, AI-agent-friendly project orchestration CLI.

- **Stack**: Rust, single portable binary
- **Target platforms**: Windows, Linux, macOS
- **Interface**: CLI with structured JSON I/O
- **Design**: procedural logic first, AI only for
  reasoning tasks

## Working Style

Before each step of execution, print a short one-line
description of what you are about to do and why, so the
human developer can follow along. "Step" here means a
distinct action — running a command, editing a file,
searching the codebase, calling a subagent — not every
individual tool call inside a single logical operation.

- State the intent *before* the action, not after.
- One sentence is almost always enough; avoid paragraphs.
- Skip the narration for trivial lookups (a single
  `Read`, a single `Grep`) where the tool call itself
  is self-explanatory.
- If you change direction mid-task, say so explicitly
  ("That did not work — trying X instead because …")
  rather than silently pivoting.

The goal is that a human skimming the transcript can
always answer "what is Claude doing right now and why?"
without reading tool output.

## Build Commands

```bash
cargo xtask check                # fast compile check (no tests)
cargo xtask validate             # clippy + tests + coverage
cargo xtask test [filter]        # tests only
cargo xtask clippy               # lint only
cargo xtask coverage             # coverage only (≥90%)
cargo xtask fmt                  # format code
```

Never use raw `cargo test` or `cargo clippy` — always
go through `xtask`.

## Coding Standards

- Rust edition 2021
- `#[deny(warnings)]` via workspace lints
- clippy pedantic where practical
- Error handling: `thiserror` for library errors,
  `anyhow` for CLI errors
- Prefer `&str` over `String` in function signatures
- All public items must have doc comments
- Wrap markdown at 80 characters per line

## Commits

Use `/commit`. No "Co-Authored-By", no emoji.

## End-User Manual

The file `docs/manual.md` is the end-user manual. Update
it when a `feat` or `fix` commit changes CLI behavior.
Skip updates for internal refactors or non-user-facing
work.

## LLM Agent Reference

The file `llms.txt` is the AI-agent-facing reference
following the llmstxt.org convention. It must reflect the
latest CLI state before any release. Update it alongside
`docs/manual.md` when CLI behavior changes.

## Skills

Project-specific skills available as slash commands:

| Skill | Purpose |
|-------|---------|
| `/check` | Fast compile check (no tests) |
| `/test` | Run tests with agent-friendly output |
| `/validate` | Full quality pipeline (clippy + tests + coverage + dupes) |
| `/commit` | Commit with versioning, diary, and code review |
| `/next-task` | Pick and implement the next WBS task |
| `/todo` | Process the next pending TODO item |
| `/rustwerk` | RustWerk CLI reference for project management (update on any functional change) |
| `/tui-expert` | TUI rendering and alignment guidelines |
| `/simplify` | Review changed code for quality |
| `/template-improve` | Log feedback for the rustbase template |
| `/template-sync` | Sync upstream template changes into this project |

## Template Sync

This project tracks its template origin in
`.template-sync.toml`. rustwerk predates the
[rustbase](https://github.com/breki/rustbase) template
and was retroactively linked to it. Use `/template-sync`
to pull improvements from upstream rustbase. The command
fetches upstream changes, categorizes them, and helps
you selectively apply relevant updates while preserving
rustwerk's customizations (notably: rustwerk is CLI-only
and has no frontend, so web/e2e template changes are
skipped by default).

## Knowledge Graph

The project has a browsable knowledge graph under
`knowledge/` (markdown notes with TOML frontmatter),
rendered to a static site by Zola. Use it to get
oriented and to record non-obvious architecture and
design information.

```bash
cargo xtask kg build                 # render site to tools/kg/site/public/
cargo xtask kg serve                 # live-reload dev server
tools/kg/scripts/kg-new.sh <section> "<Title>" [type] [tags]
tools/kg/scripts/kg-validate.sh      # check outgoing link targets
tools/kg/scripts/kg-stats.sh         # note + link counts
```

Xtask auto-downloads zola into `tools/kg/bin/` on
first run — no manual install needed.

Sections: `architecture/`, `concepts/`, `decisions/`,
`integrations/`. Schema and conventions are documented
in [tools/kg/README.md](tools/kg/README.md).

When a commit changes rustwerk architecture or makes a
non-obvious design decision, add or update the relevant
KG note alongside the code change. Do not document
code-derivable facts (file paths, obvious conventions)
— only the *why*, the cross-cutting maps, and the
integration contracts.

## Template Feedback

When you notice anything in template-provided files that
is suboptimal, incorrect, outdated, or could be
improved, log it in `docs/developer/template-feedback.md`
via `/template-improve`. This feedback is used to
improve the rustbase template for future projects.

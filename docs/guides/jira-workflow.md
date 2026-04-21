# RustWerk + Jira Workflow

A task-oriented guide for running a project in RustWerk
while keeping a Jira project synchronized for
stakeholders. You already know you want to use both —
this document shows how to wire them together and what
to do when they drift.

For the full `plugin push` reference, see
[manual.md#plugins](../manual.md#plugins). For the
field-mapping contract, see
[knowledge/integrations/jira-field-mapping.md](../../knowledge/integrations/jira-field-mapping.md).

## Contents

- [When this fits](#when-this-fits)
- [Mental model](#mental-model)
- [One-time setup](#one-time-setup)
- [Daily workflow](#daily-workflow)
- [Concept mapping](#concept-mapping)
- [Escape hatches: when Jira gets edited directly](#escape-hatches-when-jira-gets-edited-directly)
- [AI agent notes](#ai-agent-notes)

## When this fits

This guide assumes a single, recognisable shape:

- You are the developer (or team) executing the work.
- You want rustwerk's scheduling, critical-path, and
  effort analytics during development.
- You have a Jira project that stakeholders (PMs,
  management, other teams) already watch, and you
  need it to stay roughly current.

If your Jira project is the **authoritative** task
system — with a PM entering and reordering work there
directly, and you are expected to sync back into
rustwerk — this guide does **not** fit. RustWerk's
plugin is push-only (`rustwerk → Jira`); there is no
pull. Use a different tool, or treat rustwerk as a
scratch pad and skip the push.

## Mental model

**RustWerk is the source of truth. Jira is the mirror.**

| Responsibility | Home |
|---|---|
| Task creation, IDs, titles, descriptions | RustWerk |
| Dependencies, effort estimates, complexity | RustWerk |
| Status transitions (`todo` → `in-progress` → `done`) | RustWerk |
| Assignment, parent/epic structure | RustWerk |
| Effort logs (actuals) | RustWerk |
| Critical-path & Gantt analysis | RustWerk |
| Stakeholder visibility, dashboards, reports | Jira |
| Cross-team comments, attachments, ad-hoc discussion | Jira |

Think of `plugin push jira` as a deploy step that
republishes the current rustwerk state to Jira. It is
one-way, idempotent, and safe to re-run.

## One-time setup

### 1. Build and install the plugin

```bash
cargo build -p rustwerk-jira-plugin
rustwerk plugin install \
  target/debug/rustwerk_jira_plugin.dll   # Linux: .so, macOS: .dylib
```

Use `--scope user` if the plugin should be available to
every project on this machine; the default is
project-local under `.rustwerk/plugins/`.

Verify it loaded:

```bash
rustwerk plugin list
```

### 2. Configure credentials and project key

The plugin reads credentials from environment variables,
not from `project.json`:

```bash
export JIRA_URL=https://your-site.atlassian.net
export JIRA_TOKEN=<api-token>         # see Atlassian docs
export RUSTWERK_USER=alice             # used elsewhere too
```

`JIRA_TOKEN` is a Jira Cloud **API token**, not your
password. Provision it from your Atlassian account
settings — that is outside rustwerk's scope.

Pick the Jira project you will push into and remember
its key (e.g. `RUST`); you pass it to every
`plugin push` invocation.

### 3. Build the rustwerk-side config

Optional but usually worth doing: tell the plugin how
rustwerk concepts map to your Jira site's specifics.
Add a `plugins.jira` block to `.rustwerk/project.json`,
or pass overrides on the command line.

Minimum useful config for a non-trivial project:

```json
{
  "plugins": {
    "jira": {
      "default_issue_type": "Task",
      "status_map": {
        "in_progress": "11",
        "done": "31",
        "blocked": "41"
      },
      "assignee_map": {
        "alice@example.com": "557058:abcd-..."
      }
    }
  }
}
```

`status_map` values are Jira **transition IDs**, not
status names. Discover them once with:

```bash
curl -u you@example.com:$JIRA_TOKEN \
  $JIRA_URL/rest/api/3/issue/RUST-1/transitions | jq
```

`assignee_map` maps email addresses to Jira
`accountId`s; unmapped assignees are silently dropped
from the payload with a warning on the push output.

See [manual.md#plugins](../manual.md#plugins) for the
full list of config keys, including `issue_type_map`,
`priority_map`, `labels_from_tags`, and
`epic_link_custom_field`.

### 4. Dry-run before the first real push

```bash
rustwerk plugin push jira --project-key RUST --dry-run
```

This prints the resolved task list and the **names** of
the config keys that were found (never the token value).
It does not hit Jira. Use it to confirm the right env
vars are set and the right tasks will be sent.

## Daily workflow

The loop is: edit in rustwerk, push on a cadence.

```
┌──────────────────────────────────────────────────────┐
│  1. Edit rustwerk state (add / update / status /     │
│     effort).                                          │
│                                                       │
│  2. Commit. The change is in git.                    │
│                                                       │
│  3. Push to Jira at a cadence that suits your team   │
│     (end of day, before standup, on PR merge, ...).  │
└──────────────────────────────────────────────────────┘
```

Concretely:

```bash
# 1. During development — stay inside rustwerk
rustwerk task add "Implement login" --id AUTH-LOGIN \
  --complexity 5 --effort 8H --type story
rustwerk task depend AUTH-LOGIN AUTH-SCHEMA
rustwerk task status AUTH-LOGIN in-progress
rustwerk effort log AUTH-LOGIN 2H

# 2. Commit — the .rustwerk/project.json change goes
#    into the same PR as the code
git add .rustwerk/ src/
git commit -m "feat(auth): start login work"

# 3. Push when it is the right moment for Jira readers
rustwerk plugin push jira --project-key RUST
```

**Cadence.** There is no wrong answer, but two shapes
work well:

- **Per-PR-merge** — the push runs in CI on merge to
  `main`. Stakeholders see Jira update exactly when
  code lands. Requires `JIRA_TOKEN` in CI secrets.
- **End of day** — a developer runs the push manually
  before leaving. Lower infrastructure cost, visible
  delay for watchers.

Avoid pushing on every rustwerk command — Jira
rate-limits, and the push is cheap but not free.

### Why this direction

Because rustwerk state lives in `.rustwerk/project.json`
inside the repo, it travels with branches. When you
check out a feature branch, the tasks for that branch
travel with it. Jira cannot do this. If Jira were the
source of truth, every branch switch would require a
Jira sync — and merges would need Jira-aware conflict
resolution. Keeping rustwerk authoritative keeps the
VCS and the WBS in lockstep.

## Concept mapping

| RustWerk | Jira | Where configured |
|---|---|---|
| `Task.title` | `fields.summary` | always |
| `Task.description` | `fields.description` (ADF) | always |
| `Task.status` | workflow transition | `status_map` |
| `Task.assignee` | `fields.assignee.accountId` | `assignee_map` |
| `Task.parent_id` | `fields.parent.key` | always |
| `Task.parent_id` (legacy sites) | `fields.<customfield>` | `epic_link_custom_field` |
| `Task.type` (`epic`/`story`/...) | `fields.issuetype.name` | `issue_type_map` + `default_issue_type` |
| `Task.tags` | `fields.labels` | `labels_from_tags` (bool) |
| `Task.complexity` | `fields.priority.name` | `priority_map` |
| `Task.effort_estimate` | *(not pushed)* | — |
| `Task.effort_entries` (actuals) | *(not pushed)* | — |

Two consequences worth noting:

- **Effort data stays in rustwerk.** Logged effort and
  estimates are not written to Jira. If stakeholders
  need them, run `rustwerk report effort` and paste.
- **Complexity maps to priority.** Rustwerk's
  complexity is numeric (Fibonacci-ish); Jira priority
  is an enum. The `priority_map` lets you say
  `"8" → "Highest"`, `"5" → "High"`, etc. Without it,
  priority is not written.

## Escape hatches: when Jira gets edited directly

The one-way push is *defensive*, not *enforcing*.
Stakeholders can still change things in Jira — reassign
an issue, move it to a different status, rewrite the
summary. Here is what rustwerk does in each case and
what you should do about it.

### A status changed in Jira

**What rustwerk does on next push.** Nothing special
unless `status_map` is set. If it is, the plugin fires
the transition for the rustwerk status and overwrites
whatever Jira's current status was. Rustwerk wins.

**What to do.** Usually: nothing. The rustwerk status
is authoritative; the Jira edit was noise. If the
Jira-side change represents real information you want
to keep, update rustwerk explicitly and push:

```bash
rustwerk task status AUTH-LOGIN blocked
rustwerk plugin push jira --project-key RUST
```

### A summary or description changed in Jira

**What rustwerk does on next push.** Overwrites
`fields.summary` and `fields.description` with the
rustwerk values. Jira-side edits are lost.

**What to do.** Before pushing, decide whose text is
correct. If the Jira edit was right, copy the edit
back into rustwerk first:

```bash
rustwerk task update AUTH-LOGIN \
  --title "New title from Jira" \
  --desc "New description from Jira"
rustwerk plugin push jira --project-key RUST
```

### An assignee changed in Jira

**What rustwerk does on next push.** Overwrites
`fields.assignee` with whatever `assignee_map` resolves
rustwerk's assignee to. If rustwerk has no assignee and
`assignee_map` has no entry, the field is omitted and
Jira's current assignee is preserved (Jira's behavior
on a missing field in `PUT /issue` is "don't change").

**What to do.** If you want the Jira-side assignment to
stand, update rustwerk to match before the next push:

```bash
rustwerk task assign AUTH-LOGIN bob
```

If you want rustwerk-side to win, do nothing — the push
will restore it.

### An issue was deleted in Jira

**What rustwerk does on next push.** The stored
`plugin_state.jira.key` for that task resolves to 404.
The plugin creates a fresh issue and overwrites the
stored key. Comments, history, and attachments on the
old issue are gone forever — Jira does not undo a
delete.

**What to do.** Nothing; the push self-heals. Just
accept that the deleted issue's Jira history is lost.

### Two developers pushed against the same Jira project

The plugin is idempotent per rustwerk task, not
per-project-globally. If two developers each have their
own branch with divergent rustwerk state and both push,
the later push overwrites the earlier — per task. This
rarely matters because branches should merge before
they push from CI, but if you push from two
workstations against `main`, agree on cadence.

## AI agent notes

A Claude Code / Cursor session driving this workflow
should:

1. **Edit rustwerk, never Jira.** If the user asks "mark
   AUTH-LOGIN done in Jira," translate to `rustwerk task
   status AUTH-LOGIN done` followed by a push. Do not
   call the Jira REST API directly.
2. **Prefer `--json` for any command that will feed a
   later step.** The human-readable output is unstable.
3. **Dry-run before the first push in a new environment.**
   A single `--dry-run` confirms credentials are wired
   without hitting the Jira write path.
4. **Batch rustwerk edits.** Use `rustwerk batch
   --file <json>` for multi-task changes so the WBS
   moves atomically, then push once at the end.

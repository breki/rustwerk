+++
title = "Plugins are invoked implicitly by core commands, not a separate push verb"
date = 2026-04-26
description = "Implicit, synchronous, broad-scope plugin invocation gated by a one-time create predicate; `plugin push` becomes a reconcile escape hatch."

[taxonomies]
tags = ["plugin", "jira", "cli"]

[extra]
note_type = "decision"
links = [
  { relation = "relates-to", target = "integrations/jira-plugin" },
  { relation = "relates-to", target = "architecture/plugin-host" },
  { relation = "relates-to", target = "decisions/ffi-plugin-boundary" },
]
+++

**Status.** Accepted 2026-04-26, not yet implemented.
The current code ships the operator-facing
`plugin push` verb described in
[Jira plugin](@/integrations/jira-plugin.md); this note
records the target state and the design choices made
before work starts.

**Decision.** Plugin effects are side-effects of core
rustwerk domain commands, not of a separate operator-
facing verb. Each task lives in one of two states with
respect to a given plugin: **pre-push** (no Jira issue
exists yet — only the create gate matters) or
**post-push** (an issue exists — every mutation pushes
immediately). The transition between states is driven
by a one-time **create gate**: the first time a task
becomes `in_progress`, the plugin creates the issue and
the task moves to post-push for the rest of its life.

Invocation is synchronous by default with a
`--no-plugins` escape. On failure, the local mutation
commits and the failed push enters a retry queue.

## The two-state model

The state for plugin `P` is observable from a single
field on the task: `plugin_state.P.key`.

- **Pre-push** (`key` absent). The task is local-only
  with respect to plugin `P`. Mutations of any field —
  `task add`, title edits, effort updates, dependency
  changes, assignment, etc. — push **nothing**.
  The only thing the dispatcher cares about is whether
  this mutation transitioned the task to `in_progress`.
  If yes → fire create, task moves to post-push.
  If no → record locally and stop.
- **Post-push** (`key` present). The task exists in
  Jira. **Every** mutation pushes the full current
  snapshot synchronously. Title edits, status moves
  (including back to `todo`), reassignments, effort
  changes, tag edits — all fire one PUT each.

The dispatcher logic is a one-line check on every
mutation: branch on `plugin_state.P.key.is_some()`.

## The create gate

A Jira issue is created the first time a task's status
becomes `in_progress`, regardless of whether an
assignee is set. The triggering mutation is whichever
one crossed the threshold:

- `task status set <id> in_progress` on a not-yet-pushed
  task.
- `task add <id> --status in_progress` (born already
  in-progress).

Assignment is *not* part of the gate. A task may be
created in Jira with no assignee — the broad-scope
update rule means the moment somebody is later assigned
in rustwerk, that change pushes through.

Why `in_progress` and not `task add`:

- Assignment churns during planning; tasks get
  reshuffled or rewritten before anyone touches them.
- "Work started" is the moment a Jira issue earns its
  existence (time tracking, status boards, reviewer
  visibility).
- Deleting Jira issues is expensive or permission-
  gated. The conservative gate ensures issues are only
  created when someone is actually going to use them.

The gate is intentionally **asymmetric**: hard to
enter, impossible to exit. Once an issue exists, its
lifecycle is Jira's problem. A task moved back to
`todo` or unassigned still pushes the *update* — but
nothing is ever deleted on the Jira side. This is a
hard rule, not a default.

## What fires a plugin event (post-push)

Every domain command that mutates a task's mappable
fields fires a push:

- `task add` (only relevant when the gate also trips).
- `task assign`, `task unassign`.
- `task status set`.
- `task edit` (title, description, effort, complexity).
- `task tag add` / `task tag remove`.
- `task dep add` / `task dep remove`.

This is broader than the earlier "minimal scope"
proposal and is the right tradeoff because rustwerk's
mutating commands are coarse — there is no
keystroke-level editing in the CLI, so "every mutation
pushes" practically means "one push per command the
operator explicitly ran."

## Synchronous by default

Domain commands block on the plugin round-trip. The
alternative — queue-and-drain — would introduce a
second source of truth that could diverge from
`plugin_state` under partial failure. Synchronous
execution keeps the observable rule simple: "if the
command returned 0, Jira saw it (or it's in the retry
queue, which the command told you about)."

`--no-plugins` is the only way to suppress plugin
effects. There is no ambient environment-variable
toggle, because silent suppression would let drift
accumulate invisibly.

## Failure policy: local commit + retry queue

When a plugin fails:

1. The local domain mutation **commits** (project.json
   is updated, the git-native invariant holds).
2. The failure is appended to a per-plugin retry queue
   stored under `plugin_state.<plugin>.pending` on the
   affected task.
3. The domain command exits non-zero with a message
   naming the queued work so the operator is never
   surprised.

The queue is drained:

- Opportunistically — if any subsequent event fires for
  the same task, the new event flushes the queue first.
- Explicitly via `rustwerk plugin reconcile`.

Rejected alternatives:

- **Roll back the local mutation.** User-intuitive
  (atomicity) but couples local work to remote
  availability, breaking offline use.
- **Commit + warn only.** Simpler, but puts the onus
  on the operator to remember to reconcile. Implicit
  invocation only works if drift is also tracked
  implicitly.

## Parent/epic ancestors on a JIT create

Jira refuses `fields.parent.key` pointing at a non-
existent issue. When a JIT create fires on a leaf
whose ancestors are still in pre-push state, the host
walks up the parent chain and creates the missing
ancestors in the same invocation, level-by-level
(see [Jira parent/epic linking](@/integrations/jira-parent-epic.md)).

This is the same level orchestrator that exists today
— the only change is that the input set is "the
triggering task plus its pre-push ancestors" instead of
"every task in the project."

The walk-up *bypasses* the create gate for the
ancestors: a parent epic that has never been
`in_progress` still gets created so its child can link
to it. This is a conscious exception — the alternative
is an orphaned child issue with a dangling parent ref,
which is worse than an "early" parent issue.

## `plugin push` survives as `plugin reconcile`

The existing operator-facing verb is kept, rebranded
as a diagnostic / bulk-repair escape hatch:

- **Bootstrapping** an existing rustwerk project onto
  a newly-configured plugin.
- **Reconciling drift** from `--no-plugins` sessions.
- **Draining retry queues** explicitly.
- **CI / ops usage** where implicit invocation would
  not happen.

It is no longer the primary surface.

`reconcile` **respects the create gate** by default:
it pushes only tasks that are post-push (have an
existing `key`) plus tasks currently in `in_progress`.
A `--force-create-all` flag bypasses the gate for the
specific bootstrapping case where every task is
already mid-flight in some external system.

## Bootstrap from existing Jira state

A real-world rustwerk adoption frequently starts on a
project that already has issues in Jira (created
manually, in another tool, or by a previous workflow).
The bootstrap path leans on the same idempotency
ladder used everywhere else: if a task carries a valid
`plugin_state.jira.key`, the dispatcher treats it as
post-push and updates rather than creating, so no
duplicates are produced. The bootstrap problem reduces
to populating that key on each task.

Two distinct scenarios get two distinct gestures —
conflating them would be confusing:

- **Existing Jira issues, want to teach rustwerk
  about them.** Use `plugin link`. rustwerk learns
  the binding; nothing new is created in Jira.
- **Empty Jira, want every rustwerk task materialized
  now.** Use `plugin reconcile --force-create-all`.

Mixed projects get the natural sequence: link the
already-existing issues first, then either let the JIT
gate handle the rest organically, or run
`reconcile --force-create-all` to materialize the
remainder.

### `rustwerk plugin link <task-id> <jira-key>`

Single-task binding. The command:

1. Validates `<jira-key>` against the issue-key
   grammar (same check used by the existing
   `IssueKey::parse`).
2. **Issues a GET against the configured Jira project
   to confirm the issue exists.** This is the
   non-obvious safety check.
3. On success, writes
   `plugin_state.jira = { key, self, last_pushed_at: <now> }`
   to the task.
4. Refuses if the key 404s.

Why the GET matters: the standard
`ProbeOutcome::MissingConfirmed` path on the next push
would treat a typo'd key as "issue deleted" and trigger
the recreate-on-404 branch — silently creating a
*new* Jira issue under a fresh key while leaving the
pre-existing one untouched. Validating at link time
closes that hole; once the binding is written, the
normal probe semantics are correct because the key was
known-good at write time.

### `rustwerk plugin link --bulk <file>`

Batched form. Input is a CSV or JSON file mapping
rustwerk task IDs to Jira keys. The command:

1. Validates **every** key against Jira before writing
   **any** binding.
2. Aborts on the first failure with a per-row error
   list — partial bootstraps do not leave half the
   project linked to nonexistent issues.
3. Writes all bindings atomically once validation
   passes.

This is the realistic operator flow for "we already
have 80 tasks in Jira": export from Jira, hand-build
or fuzzy-script the rustwerk-task-ID ↔ Jira-key
mapping, feed it in.

### Auto-discovery is deliberately out of scope

Matching tasks to existing Jira issues by title or by
a custom Jira field that holds the rustwerk task ID is
**not** part of the design. Title-matching is fragile
(duplicates, drift, localization); custom-field
matching requires the operator to have set the field
up in Jira historically, which the manual-creation
scenario implies they did not. Link-by-explicit-mapping
is the only path with predictable failure modes.
A future helper that *generates* the mapping file by
fuzzy-matching titles is fine; it must produce the
same explicit mapping format `link --bulk` consumes
rather than acting directly on `plugin_state`.

## Consequences

- The plugin host crate grows a small dispatcher that
  each domain command calls after a successful local
  mutation. The existing `push_all` FFI entry point is
  unchanged — it already accepts an arbitrary task
  slice.
- `plugin_state.<plugin>` gains a `pending` array
  alongside the existing `key` / `self` /
  `last_pushed_at` anchor fields. Host treats it
  opaquely.
- Idempotency guarantees become more load-bearing, not
  less. Implicit invocation means re-runs happen
  naturally (retry drain, reconcile) — the
  create/probe/update ladder must keep working.
- Every domain command's tests gain a "plugin is
  installed and configured" case. Mocking the FFI
  boundary is cheap; the cost is real but bounded.
- The "stranded edits" failure mode that a narrower
  event scope would have introduced is eliminated by
  construction: any post-push mutation pushes.
- A new operator-facing verb, `plugin link` (with a
  `--bulk` form), is added specifically for the
  existing-Jira-state bootstrap. It is the only
  command in the design that *writes* `plugin_state`
  without performing a push first; the GET-to-validate
  step is what keeps it safe.

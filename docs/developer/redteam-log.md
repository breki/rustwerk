# Red Team Findings — Open

Open findings from red team reviews, newest first.
Fixed findings are moved to
[redteam-resolved.md](redteam-resolved.md).

**Next ID:** RT-037

**Threshold:** when 10+ findings are open, a full-codebase
red team review is required before continuing feature work.

---

### RT-024 — Cyclic graph in hand-edited JSON causes panic

- **Date:** 2026-04-03
- **Category:** Correctness
- **Commit context:** v0.13.0 Gantt chart
- **Description:** `topological_sort` silently returns
  fewer tasks when cycles exist in hand-edited JSON
  (runtime `add_dependency` prevents cycles but there's
  no validation on load). `critical_path` then panics
  accessing `dist[other_id]` for tasks not in the
  topological order.
- **Impact:** Hard crash on `rustwerk gantt` or
  `rustwerk task list` with corrupted project file.
- **Suggested fix:** Validate graph on load, or check
  `order.len() == tasks.len()` after topological sort.

### RT-014 — Batch `--file` reads any path (path traversal)

- **Date:** 2026-04-03
- **Category:** Security
- **Commit context:** v0.11.0 batch command / coverage
- **Description:** `--file` argument is passed directly to
  `fs::read_to_string` with no path validation. Any
  readable file on the system can be read. If the file
  isn't valid JSON, serde's error message may leak a
  fragment of the file content to stderr.
- **Impact:** Low for a CLI tool invoked by the user
  themselves. Higher risk if rustwerk is ever invoked by
  an orchestration layer with untrusted input.
- **Suggested fix:** Acceptable for current use case.
  Restrict path if rustwerk is ever used non-interactively.

### RT-013 — Batch rollback is implicit, no explicit snapshot

- **Date:** 2026-04-03
- **Category:** Code Quality
- **Commit context:** Coverage infrastructure
- **Description:** Batch "atomicity" relies on not calling
  `save_project` on error — there is no snapshot of the
  original project state that gets restored. If a future
  refactor moves the save earlier (e.g. for checkpointing),
  the atomicity guarantee silently breaks.
- **Impact:** Design debt — not a current bug but fragile
  for future changes.
- **Suggested fix:** Clone the project before the batch
  loop, restore the clone on error.

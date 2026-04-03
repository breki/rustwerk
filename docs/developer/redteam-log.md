# Red Team Findings — Open

Open findings from red team reviews, newest first.
Fixed findings are moved to
[redteam-resolved.md](redteam-resolved.md).

**Next ID:** RT-023

**Threshold:** when 10+ findings are open, a full-codebase
red team review is required before continuing feature work.

---

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

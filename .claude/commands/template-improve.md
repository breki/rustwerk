---
description: Log feedback for the rustbase template
allowed-tools: Read, Edit, AskUserQuestion
---

Log an observation about the rustbase template in
`docs/developer/template-feedback.md`.

## Instructions

1. If the user provided a specific observation, use it.
   Otherwise, use `AskUserQuestion` to ask what they
   noticed.

2. Read `docs/developer/template-feedback.md` to see
   the format and existing entries.

3. Add the new entry under today's date heading
   (create the heading if it doesn't exist). Use
   reverse chronological order (newest date first,
   newest entry first within a date).

4. Format each entry as:
   ```
   - **Short title.** Description of the issue,
     why it matters, and suggested fix for the
     template.
   ```

5. Do NOT commit -- just edit the file and let the
   user review. It will be included in the next
   `/commit`.

# Development Diary Guidelines

The development diary at `docs/developer/DIARY.md` tracks significant
changes to the codebase. This provides a human-readable history of
what was changed and why.

## When to Add Entries

**Add an entry for:**
- Functional changes: features, bug fixes, performance improvements
- Infrastructure changes: build tools, CI/CD, major dependency
  updates

**Do NOT add entries for:**
- Documentation-only updates
- Formatting or comment changes
- Refactoring without behavior changes
- Minor dependency bumps

## Entry Format

Entries are in **reverse chronological order** (newest first).

**Merge entries for the same day** into a single date heading.

```markdown
### YYYY-MM-DD

- Brief title for first change (vX.Y.Z)

    Description of what was changed and why.

- Brief title for second change (vX.Y.Z)

    Description of the second change.
```

The version in parentheses is the app version **after** the version
bump for that commit (from `Cargo.toml`). Attach the version to each
entry, not the date heading, since multiple version bumps can happen
on the same day. Only include the version tag for entries that bumped
the version (`feat`, `fix`, `perf`).

## Style Guidelines

- **Use backticks** for technical terms: function names, file names,
  config fields, error codes, commands
- Keep descriptions concise but informative
- Focus on the "why" not just the "what"

## Example Entry

```markdown
### 2026-04-10

- Add task dependency graph (v0.2.0)

    Implemented `DependencyGraph` to model task relationships as a
    DAG. CLI commands `add-dep` and `remove-dep` update the graph.
    Cycle detection prevents invalid dependency chains.

- Fix status transition for blocked tasks (v0.1.1)

    Corrected `update_status()` to check upstream dependencies
    before allowing BLOCKED → IN_PROGRESS transition.
```

## Tips

- Review recent entries to match the existing style
- Group related changes under a single bullet point
- Include relevant file/function names for context

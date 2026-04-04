# PLG-MAP: Task-to-Jira issue mapping (ADF format)

## Why

Jira Cloud REST API v3 requires Atlassian Document
Format (ADF) for rich text fields like description.
Plain text or markdown won't render correctly. This
task handles the conversion from TaskDto to valid
Jira issue creation payloads.

## What

### Mapping module (`mapping.rs`)

Converts `TaskDto` to Jira `POST /rest/api/3/issue`
payload:

```json
{
  "fields": {
    "project": { "key": "<project_key>" },
    "summary": "<task.title>",
    "description": {
      "type": "doc",
      "version": 1,
      "content": [
        {
          "type": "paragraph",
          "content": [
            { "type": "text", "text": "<task.description>" }
          ]
        }
      ]
    },
    "issuetype": { "name": "Task" }
  }
}
```

### ADF construction

- Wrap description text in a single paragraph node
- Handle None/empty description (omit the field or
  send empty doc)
- Multi-line descriptions: one paragraph per line,
  or a single paragraph with the full text

### Field mapping

| TaskDto field | Jira field | Notes |
|---------------|------------|-------|
| title | summary | Direct |
| description | description | ADF format |
| — | project.key | From config |
| — | issuetype.name | Default "Task" |

### Not in scope (initial version)

- Status mapping (would need Jira workflow
  transition IDs)
- Assignee mapping (would need Jira account IDs)
- Priority mapping
- Labels/components from tags
- Updating existing issues (push-only, create new)

## How

- File: `crates/rustwerk-jira-plugin/src/mapping.rs`
- Pure functions, no side effects
- All Jira JSON built with `serde_json::json!` macro

## Acceptance criteria

- [ ] Produces valid Jira issue creation JSON
- [ ] ADF description renders correctly in Jira
- [ ] Handles empty/None description gracefully
- [ ] Unit tests for all mapping cases
- [ ] Test with actual Jira Cloud free instance
      (manual verification)
- [ ] `cargo xtask clippy` passes

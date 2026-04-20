//! Map rustwerk [`TaskDto`]s to Jira issue-creation
//! payloads.
//!
//! The Jira Cloud REST API v3 requires rich-text fields
//! like `description` to be expressed in Atlassian
//! Document Format (ADF): a versioned tree of `doc`,
//! `paragraph`, and `text` nodes. This module converts a
//! [`TaskDto`] into the minimal issue-creation payload
//! shape — `project`, `summary`, an ADF `description`,
//! and an `issuetype` of `"Task"`.

use rustwerk_plugin_api::TaskDto;
use serde_json::{json, Value};

/// Build the JSON payload for `POST /rest/api/3/issue`
/// for a single task.
pub(crate) fn build_issue_payload(task: &TaskDto, project_key: &str) -> Value {
    json!({
        "fields": {
            "project": { "key": project_key },
            "summary": summary_for(task),
            "description": adf_doc(description_text(task)),
            "issuetype": { "name": "Task" },
        }
    })
}

/// Produce the Jira `summary` field. Prefers the task's
/// title; falls back to the ID when the title is empty.
fn summary_for(task: &TaskDto) -> String {
    if task.title.trim().is_empty() {
        format!("[{}] (untitled)", task.id)
    } else {
        format!("[{}] {}", task.id, task.title)
    }
}

/// Source text for the Jira `description` field. Falls
/// back to the task title when the description is empty
/// so Jira never receives a blank document.
fn description_text(task: &TaskDto) -> &str {
    if task.description.is_empty() {
        task.title.as_str()
    } else {
        task.description.as_str()
    }
}

/// Build an Atlassian Document Format (ADF) `doc` node
/// from plain text, producing one paragraph per line.
///
/// Line endings are normalized: `\r\n` and bare `\r` are
/// folded into `\n` before splitting, so Windows-authored
/// descriptions don't smuggle trailing carriage returns
/// into ADF `text` nodes.
///
/// Empty input still produces a valid ADF `doc` with a
/// single empty paragraph — `"".split('\n')` yields one
/// empty element, and Jira rejects a `doc` whose
/// `content` array is empty.
fn adf_doc(text: &str) -> Value {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let paragraphs: Vec<Value> = normalized.split('\n').map(adf_paragraph).collect();
    json!({
        "type": "doc",
        "version": 1,
        "content": paragraphs
    })
}

/// Build a single ADF `paragraph` node. Empty lines (and
/// lines that become empty after stripping disallowed
/// control characters) render as a paragraph with no
/// `content` field, which ADF treats as a blank line.
///
/// ADF `text` nodes reject ASCII control characters other
/// than `\t`; `\n` is already handled by the caller when
/// it splits into lines. Stripping here keeps a stray
/// form-feed or ANSI escape from turning a successful
/// push into an opaque HTTP 400 from the Jira validator.
fn adf_paragraph(line: &str) -> Value {
    let sanitized: String = line
        .chars()
        .filter(|c| *c == '\t' || !c.is_control())
        .collect();
    if sanitized.is_empty() {
        json!({ "type": "paragraph" })
    } else {
        json!({
            "type": "paragraph",
            "content": [ { "type": "text", "text": sanitized } ]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustwerk_plugin_api::TaskStatusDto;

    fn task() -> TaskDto {
        TaskDto {
            id: "PLG-JIRA".into(),
            title: "Jira plugin".into(),
            description: "Push tasks to Jira".into(),
            status: TaskStatusDto::InProgress,
            dependencies: vec![],
            effort_estimate: None,
            complexity: None,
            assignee: None,
            tags: vec![],
            plugin_state: None,
        }
    }

    #[test]
    fn payload_contains_project_key() {
        let v = build_issue_payload(&task(), "PROJ");
        assert_eq!(v["fields"]["project"]["key"], "PROJ");
    }

    #[test]
    fn payload_summary_includes_id_and_title() {
        let v = build_issue_payload(&task(), "PROJ");
        assert_eq!(v["fields"]["summary"], "[PLG-JIRA] Jira plugin");
    }

    #[test]
    fn payload_description_is_adf_doc() {
        let v = build_issue_payload(&task(), "PROJ");
        let desc = &v["fields"]["description"];
        assert_eq!(desc["type"], "doc");
        assert_eq!(desc["version"], 1);
        assert!(desc["content"].is_array());
    }

    #[test]
    fn payload_description_wraps_text_in_paragraph() {
        let v = build_issue_payload(&task(), "PROJ");
        let content = &v["fields"]["description"]["content"];
        assert_eq!(content[0]["type"], "paragraph");
        assert_eq!(content[0]["content"][0]["type"], "text");
        assert_eq!(content[0]["content"][0]["text"], "Push tasks to Jira");
    }

    #[test]
    fn payload_description_one_paragraph_per_line() {
        let mut t = task();
        t.description = "first line\nsecond line\nthird".into();
        let v = build_issue_payload(&t, "PROJ");
        let content = &v["fields"]["description"]["content"];
        assert_eq!(content.as_array().unwrap().len(), 3);
        assert_eq!(content[0]["content"][0]["text"], "first line");
        assert_eq!(content[1]["content"][0]["text"], "second line");
        assert_eq!(content[2]["content"][0]["text"], "third");
    }

    #[test]
    fn payload_description_blank_line_is_empty_paragraph() {
        let mut t = task();
        t.description = "first\n\nthird".into();
        let v = build_issue_payload(&t, "PROJ");
        let content = &v["fields"]["description"]["content"];
        assert_eq!(content.as_array().unwrap().len(), 3);
        assert_eq!(content[1]["type"], "paragraph");
        assert!(content[1].get("content").is_none());
    }

    #[test]
    fn payload_description_normalizes_crlf() {
        let mut t = task();
        t.description = "line one\r\nline two".into();
        let v = build_issue_payload(&t, "PROJ");
        let content = &v["fields"]["description"]["content"];
        assert_eq!(content.as_array().unwrap().len(), 2);
        assert_eq!(content[0]["content"][0]["text"], "line one");
        assert_eq!(content[1]["content"][0]["text"], "line two");
    }

    #[test]
    fn payload_description_normalizes_bare_cr() {
        let mut t = task();
        t.description = "line one\rline two".into();
        let v = build_issue_payload(&t, "PROJ");
        let content = &v["fields"]["description"]["content"];
        assert_eq!(content.as_array().unwrap().len(), 2);
        assert_eq!(content[0]["content"][0]["text"], "line one");
        assert_eq!(content[1]["content"][0]["text"], "line two");
    }

    #[test]
    fn payload_description_strips_control_chars() {
        let mut t = task();
        t.description = "hello\x0cworld\x1b[31m!".into();
        let v = build_issue_payload(&t, "PROJ");
        let content = &v["fields"]["description"]["content"];
        assert_eq!(content[0]["content"][0]["text"], "helloworld[31m!");
    }

    #[test]
    fn payload_description_preserves_tabs() {
        let mut t = task();
        t.description = "col1\tcol2".into();
        let v = build_issue_payload(&t, "PROJ");
        let content = &v["fields"]["description"]["content"];
        assert_eq!(content[0]["content"][0]["text"], "col1\tcol2");
    }

    #[test]
    fn payload_description_falls_back_to_title_when_description_empty() {
        let mut t = task();
        t.description = String::new();
        let v = build_issue_payload(&t, "PROJ");
        let content = &v["fields"]["description"]["content"];
        assert_eq!(content[0]["content"][0]["text"], "Jira plugin");
    }

    #[test]
    fn payload_marks_untitled_summary_when_title_blank() {
        let mut t = task();
        t.title = "   ".into();
        let v = build_issue_payload(&t, "PROJ");
        assert_eq!(v["fields"]["summary"], "[PLG-JIRA] (untitled)");
    }

    #[test]
    fn payload_issue_type_is_task() {
        let v = build_issue_payload(&task(), "PROJ");
        assert_eq!(v["fields"]["issuetype"]["name"], "Task");
    }

    #[test]
    fn adf_doc_is_valid_even_when_input_empty() {
        let v = adf_doc("");
        assert_eq!(v["type"], "doc");
        assert_eq!(v["content"].as_array().unwrap().len(), 1);
        assert_eq!(v["content"][0]["type"], "paragraph");
    }
}

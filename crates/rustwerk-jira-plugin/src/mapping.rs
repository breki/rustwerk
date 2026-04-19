//! Map rustwerk [`TaskDto`]s to Jira issue-creation
//! payloads.
//!
//! This is the minimal shape needed to prove the plugin
//! architecture end-to-end: `project`, `summary`, a
//! plain-text `description`, and an `issuetype` of
//! `"Task"`. PLG-MAP upgrades the description to
//! Atlassian Document Format (ADF) and may extend this
//! payload.

use rustwerk_plugin_api::TaskDto;
use serde_json::{json, Value};

/// Build the JSON payload for `POST /rest/api/3/issue`
/// for a single task.
pub(crate) fn build_issue_payload(task: &TaskDto, project_key: &str) -> Value {
    let description = if task.description.is_empty() {
        task.title.clone()
    } else {
        task.description.clone()
    };
    json!({
        "fields": {
            "project": { "key": project_key },
            "summary": summary_for(task),
            "description": description,
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
    fn payload_uses_description_when_present() {
        let v = build_issue_payload(&task(), "PROJ");
        assert_eq!(v["fields"]["description"], "Push tasks to Jira");
    }

    #[test]
    fn payload_falls_back_to_title_when_description_empty() {
        let mut t = task();
        t.description = String::new();
        let v = build_issue_payload(&t, "PROJ");
        assert_eq!(v["fields"]["description"], "Jira plugin");
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
}

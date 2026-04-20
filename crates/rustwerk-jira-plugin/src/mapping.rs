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

use crate::config::JiraConfig;
use crate::jira_client::IssueKey;
use crate::push::STATE_KEY_FIELD;
use crate::warnings::MappingWarning;

/// Outbound Jira issue payload plus any advisory
/// warnings emitted while building it. Warnings are
/// non-fatal: the payload still goes out, just without
/// the unmapped / rejected fields.
#[derive(Debug, Clone)]
pub(crate) struct IssuePayload {
    /// JSON body ready for `POST`/`PUT /issue`.
    pub body: Value,
    /// Advisory warnings produced by the mapping step.
    /// Additional warnings (e.g. from the transition
    /// call) may be appended downstream before the final
    /// message is rendered.
    pub warnings: Vec<MappingWarning>,
}

/// Build the JSON payload for `POST /rest/api/3/issue`
/// for a single task.
///
/// Takes the whole [`JiraConfig`] rather than individual
/// fields so per-task issue-type resolution, per-task
/// assignee/priority mapping, and any future policy can
/// be looked up here without fanning the call sites out
/// again.
pub(crate) fn build_issue_payload(task: &TaskDto, cfg: &JiraConfig) -> IssuePayload {
    let issue_type_name = cfg.resolve_issue_type_name(task.issue_type.as_deref());
    let mut fields = serde_json::Map::new();
    fields.insert("project".into(), json!({ "key": cfg.project_key }));
    fields.insert("summary".into(), Value::String(summary_for(task)));
    fields.insert("description".into(), adf_doc(description_text(task)));
    fields.insert("issuetype".into(), json!({ "name": issue_type_name }));

    let mut warnings = Vec::new();
    apply_assignee(&mut fields, &mut warnings, task, cfg);
    apply_priority(&mut fields, &mut warnings, task, cfg);
    apply_labels(&mut fields, &mut warnings, task, cfg);
    apply_parent(&mut fields, &mut warnings, task, cfg);

    IssuePayload {
        body: Value::Object(
            std::iter::once(("fields".to_string(), Value::Object(fields))).collect(),
        ),
        warnings,
    }
}

/// Emit `fields.assignee.accountId` when `assignee_map`
/// holds an entry for the task's assignee. An absent map
/// is silent; a populated map with no matching key
/// produces a warning so the operator can fix the config.
fn apply_assignee(
    fields: &mut serde_json::Map<String, Value>,
    warnings: &mut Vec<MappingWarning>,
    task: &TaskDto,
    cfg: &JiraConfig,
) {
    let Some(email) = task.assignee.as_deref() else {
        return;
    };
    if cfg.assignee_map.is_empty() {
        return;
    }
    match cfg.account_id_for_assignee(email) {
        Some(account_id) => {
            fields.insert("assignee".into(), json!({ "accountId": account_id }));
        }
        None => warnings.push(MappingWarning::UnmappedAssignee(email.to_owned())),
    }
}

/// Emit `fields.priority.name` when `priority_map` holds
/// an entry for the task's complexity. Same silent-vs-warn
/// semantics as assignee.
fn apply_priority(
    fields: &mut serde_json::Map<String, Value>,
    warnings: &mut Vec<MappingWarning>,
    task: &TaskDto,
    cfg: &JiraConfig,
) {
    let Some(complexity) = task.complexity else {
        return;
    };
    if cfg.priority_map.is_empty() {
        return;
    }
    match cfg.priority_name_for_complexity(complexity) {
        Some(name) => {
            fields.insert("priority".into(), json!({ "name": name }));
        }
        None => warnings.push(MappingWarning::UnmappedPriority(complexity)),
    }
}

/// Emit `fields.labels` only when the plugin is
/// explicitly configured to forward tags. Default
/// `labels_from_tags: false` → silent skip.
fn apply_labels(
    fields: &mut serde_json::Map<String, Value>,
    warnings: &mut Vec<MappingWarning>,
    task: &TaskDto,
    cfg: &JiraConfig,
) {
    if !cfg.labels_from_tags || task.tags.is_empty() {
        return;
    }
    // Jira labels reject whitespace and control characters.
    // Forwarding a bad tag would return HTTP 400 and fail
    // the entire task, which also aborts transition +
    // state-anchor writes (RT-X2). Drop rejected tags
    // with a warning so the push still succeeds.
    let mut labels: Vec<Value> = Vec::with_capacity(task.tags.len());
    for tag in &task.tags {
        if is_valid_jira_label(tag) {
            labels.push(Value::String(tag.clone()));
        } else {
            warnings.push(MappingWarning::RejectedLabel(tag.clone()));
        }
    }
    if !labels.is_empty() {
        fields.insert("labels".into(), Value::Array(labels));
    }
}

/// Jira accepts labels made of non-whitespace,
/// non-control characters only. Empty strings are also
/// rejected. Kept as a free function so tests can assert
/// on the predicate directly.
fn is_valid_jira_label(s: &str) -> bool {
    !s.is_empty() && !s.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// PLG-JIRA-PARENT: emit `fields.parent.key` when the
/// host attached `parent_plugin_state` and it carries a
/// valid Jira issue key. When the legacy
/// `epic_link_custom_field` is configured, emit that
/// field with the same key alongside `parent.key` so
/// legacy sites that drive hierarchy through a custom
/// field stay linked.
///
/// Missing `parent_plugin_state` is silent (root task,
/// or parent not yet pushed). Present-but-invalid key
/// (hand-edited `project.json`, or schema drift) emits
/// a typed warning and the parent field is omitted —
/// creating an orphan issue is better than failing the
/// task.
fn apply_parent(
    fields: &mut serde_json::Map<String, Value>,
    warnings: &mut Vec<MappingWarning>,
    task: &TaskDto,
    cfg: &JiraConfig,
) {
    let Some(state) = task.parent_plugin_state.as_ref() else {
        return;
    };
    let Some(raw_key) = state.get(STATE_KEY_FIELD).and_then(Value::as_str) else {
        return;
    };
    let Some(parent_key) = IssueKey::parse(raw_key) else {
        warnings.push(MappingWarning::InvalidParentKey(raw_key.to_owned()));
        return;
    };
    fields.insert(
        "parent".into(),
        json!({ "key": parent_key.as_str() }),
    );
    if let Some(custom_field) = cfg.epic_link_custom_field.as_deref() {
        fields.insert(
            custom_field.to_owned(),
            Value::String(parent_key.as_str().to_owned()),
        );
    }
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
    use std::collections::HashMap;

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
            issue_type: None,
            plugin_state: None,
            parent_plugin_state: None,
        }
    }

    fn cfg(key: &str) -> JiraConfig {
        JiraConfig {
            jira_url: "https://x.atlassian.net".into(),
            jira_token: "t".into(),
            username: "u".into(),
            project_key: key.into(),
            default_issue_type: None,
            issue_type_map: HashMap::new(),
            status_map: HashMap::new(),
            assignee_map: HashMap::new(),
            priority_map: HashMap::new(),
            labels_from_tags: false,
            epic_link_custom_field: None,
        }
    }

    #[test]
    fn payload_contains_project_key() {
        let v = build_issue_payload(&task(), &cfg("PROJ")).body;
        assert_eq!(v["fields"]["project"]["key"], "PROJ");
    }

    #[test]
    fn payload_summary_includes_id_and_title() {
        let v = build_issue_payload(&task(), &cfg("PROJ")).body;
        assert_eq!(v["fields"]["summary"], "[PLG-JIRA] Jira plugin");
    }

    #[test]
    fn payload_description_is_adf_doc() {
        let v = build_issue_payload(&task(), &cfg("PROJ")).body;
        let desc = &v["fields"]["description"];
        assert_eq!(desc["type"], "doc");
        assert_eq!(desc["version"], 1);
        assert!(desc["content"].is_array());
    }

    #[test]
    fn payload_description_wraps_text_in_paragraph() {
        let v = build_issue_payload(&task(), &cfg("PROJ")).body;
        let content = &v["fields"]["description"]["content"];
        assert_eq!(content[0]["type"], "paragraph");
        assert_eq!(content[0]["content"][0]["type"], "text");
        assert_eq!(content[0]["content"][0]["text"], "Push tasks to Jira");
    }

    #[test]
    fn payload_description_one_paragraph_per_line() {
        let mut t = task();
        t.description = "first line\nsecond line\nthird".into();
        let v = build_issue_payload(&t, &cfg("PROJ")).body;
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
        let v = build_issue_payload(&t, &cfg("PROJ")).body;
        let content = &v["fields"]["description"]["content"];
        assert_eq!(content.as_array().unwrap().len(), 3);
        assert_eq!(content[1]["type"], "paragraph");
        assert!(content[1].get("content").is_none());
    }

    #[test]
    fn payload_description_normalizes_crlf() {
        let mut t = task();
        t.description = "line one\r\nline two".into();
        let v = build_issue_payload(&t, &cfg("PROJ")).body;
        let content = &v["fields"]["description"]["content"];
        assert_eq!(content.as_array().unwrap().len(), 2);
        assert_eq!(content[0]["content"][0]["text"], "line one");
        assert_eq!(content[1]["content"][0]["text"], "line two");
    }

    #[test]
    fn payload_description_normalizes_bare_cr() {
        let mut t = task();
        t.description = "line one\rline two".into();
        let v = build_issue_payload(&t, &cfg("PROJ")).body;
        let content = &v["fields"]["description"]["content"];
        assert_eq!(content.as_array().unwrap().len(), 2);
        assert_eq!(content[0]["content"][0]["text"], "line one");
        assert_eq!(content[1]["content"][0]["text"], "line two");
    }

    #[test]
    fn payload_description_strips_control_chars() {
        let mut t = task();
        t.description = "hello\x0cworld\x1b[31m!".into();
        let v = build_issue_payload(&t, &cfg("PROJ")).body;
        let content = &v["fields"]["description"]["content"];
        assert_eq!(content[0]["content"][0]["text"], "helloworld[31m!");
    }

    #[test]
    fn payload_description_preserves_tabs() {
        let mut t = task();
        t.description = "col1\tcol2".into();
        let v = build_issue_payload(&t, &cfg("PROJ")).body;
        let content = &v["fields"]["description"]["content"];
        assert_eq!(content[0]["content"][0]["text"], "col1\tcol2");
    }

    #[test]
    fn payload_description_falls_back_to_title_when_description_empty() {
        let mut t = task();
        t.description = String::new();
        let v = build_issue_payload(&t, &cfg("PROJ")).body;
        let content = &v["fields"]["description"]["content"];
        assert_eq!(content[0]["content"][0]["text"], "Jira plugin");
    }

    #[test]
    fn payload_marks_untitled_summary_when_title_blank() {
        let mut t = task();
        t.title = "   ".into();
        let v = build_issue_payload(&t, &cfg("PROJ")).body;
        assert_eq!(v["fields"]["summary"], "[PLG-JIRA] (untitled)");
    }

    #[test]
    fn payload_issue_type_defaults_to_task_when_unset() {
        let v = build_issue_payload(&task(), &cfg("PROJ")).body;
        assert_eq!(v["fields"]["issuetype"]["name"], "Task");
    }

    #[test]
    fn payload_issue_type_uses_per_task_value_over_default() {
        let mut t = task();
        t.issue_type = Some("epic".into());
        let v = build_issue_payload(&t, &cfg("PROJ")).body;
        assert_eq!(v["fields"]["issuetype"]["name"], "Epic");
    }

    #[test]
    fn payload_issue_type_uses_config_default_when_task_unset() {
        let mut c = cfg("PROJ");
        c.default_issue_type = Some("Story".into());
        let v = build_issue_payload(&task(), &c).body;
        assert_eq!(v["fields"]["issuetype"]["name"], "Story");
    }

    #[test]
    fn payload_issue_type_respects_map_override() {
        let mut c = cfg("PROJ");
        c.issue_type_map
            .insert("sub-task".into(), "Subtask".into());
        let mut t = task();
        t.issue_type = Some("sub-task".into());
        let v = build_issue_payload(&t, &c).body;
        assert_eq!(v["fields"]["issuetype"]["name"], "Subtask");
    }

    // --- PLG-JIRA-FIELDS: assignee / priority / labels ---

    #[test]
    fn assignee_omitted_when_map_empty_even_if_task_has_assignee() {
        let mut t = task();
        t.assignee = Some("alice@example.com".into());
        let out = build_issue_payload(&t, &cfg("PROJ"));
        assert!(out.body["fields"].get("assignee").is_none());
        assert!(out.warnings.is_empty());
    }

    #[test]
    fn assignee_omitted_when_task_has_no_assignee() {
        let mut c = cfg("PROJ");
        c.assignee_map.insert("alice@example.com".into(), "A".into());
        let out = build_issue_payload(&task(), &c);
        assert!(out.body["fields"].get("assignee").is_none());
        assert!(out.warnings.is_empty());
    }

    #[test]
    fn assignee_emitted_with_account_id_on_hit() {
        let mut c = cfg("PROJ");
        c.assignee_map
            .insert("alice@example.com".into(), "712020:abc".into());
        let mut t = task();
        t.assignee = Some("alice@example.com".into());
        let out = build_issue_payload(&t, &c);
        assert_eq!(
            out.body["fields"]["assignee"]["accountId"],
            "712020:abc"
        );
        assert!(out.warnings.is_empty());
    }

    #[test]
    fn assignee_miss_warns_and_omits_field() {
        let mut c = cfg("PROJ");
        c.assignee_map.insert("alice@example.com".into(), "A".into());
        let mut t = task();
        t.assignee = Some("bob@example.com".into());
        let out = build_issue_payload(&t, &c);
        assert!(out.body["fields"].get("assignee").is_none());
        assert_eq!(out.warnings.len(), 1);
        assert_eq!(
            out.warnings[0],
            MappingWarning::UnmappedAssignee("bob@example.com".into())
        );
    }

    #[test]
    fn priority_emitted_with_name_on_hit() {
        let mut c = cfg("PROJ");
        c.priority_map.insert("1".into(), "Highest".into());
        let mut t = task();
        t.complexity = Some(1);
        let out = build_issue_payload(&t, &c);
        assert_eq!(out.body["fields"]["priority"]["name"], "Highest");
        assert!(out.warnings.is_empty());
    }

    #[test]
    fn priority_omitted_when_map_empty() {
        let mut t = task();
        t.complexity = Some(3);
        let out = build_issue_payload(&t, &cfg("PROJ"));
        assert!(out.body["fields"].get("priority").is_none());
        assert!(out.warnings.is_empty());
    }

    #[test]
    fn priority_miss_warns_and_omits_field() {
        let mut c = cfg("PROJ");
        c.priority_map.insert("1".into(), "Highest".into());
        let mut t = task();
        t.complexity = Some(9);
        let out = build_issue_payload(&t, &c);
        assert!(out.body["fields"].get("priority").is_none());
        assert_eq!(out.warnings, vec![MappingWarning::UnmappedPriority(9)]);
    }

    #[test]
    fn priority_omitted_when_task_has_no_complexity() {
        let mut c = cfg("PROJ");
        c.priority_map.insert("3".into(), "Medium".into());
        let out = build_issue_payload(&task(), &c);
        assert!(out.body["fields"].get("priority").is_none());
        assert!(out.warnings.is_empty());
    }

    #[test]
    fn labels_omitted_by_default_even_with_tags() {
        let mut t = task();
        t.tags = vec!["alpha".into(), "beta".into()];
        let out = build_issue_payload(&t, &cfg("PROJ"));
        assert!(out.body["fields"].get("labels").is_none());
    }

    #[test]
    fn labels_emitted_when_configured_and_tags_present() {
        let mut c = cfg("PROJ");
        c.labels_from_tags = true;
        let mut t = task();
        t.tags = vec!["alpha".into(), "beta".into()];
        let out = build_issue_payload(&t, &c);
        let labels = out.body["fields"]["labels"].as_array().unwrap();
        assert_eq!(labels.len(), 2);
        assert_eq!(labels[0], "alpha");
        assert_eq!(labels[1], "beta");
    }

    #[test]
    fn labels_omitted_when_configured_but_tags_empty() {
        let mut c = cfg("PROJ");
        c.labels_from_tags = true;
        let out = build_issue_payload(&task(), &c);
        assert!(out.body["fields"].get("labels").is_none());
    }

    #[test]
    fn labels_with_whitespace_are_dropped_with_warning() {
        // RT-X2: a tag containing a space must not be
        // forwarded verbatim — Jira 400s the whole push
        // and the idempotency anchor is lost. Drop the
        // bad tag, keep the rest, surface a warning.
        let mut c = cfg("PROJ");
        c.labels_from_tags = true;
        let mut t = task();
        t.tags = vec!["tech debt".into(), "alpha".into()];
        let out = build_issue_payload(&t, &c);
        let labels = out.body["fields"]["labels"].as_array().unwrap();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0], "alpha");
        assert_eq!(
            out.warnings,
            vec![MappingWarning::RejectedLabel("tech debt".into())]
        );
    }

    #[test]
    fn labels_field_omitted_when_every_tag_rejected() {
        let mut c = cfg("PROJ");
        c.labels_from_tags = true;
        let mut t = task();
        t.tags = vec!["has space".into(), "".into()];
        let out = build_issue_payload(&t, &c);
        assert!(out.body["fields"].get("labels").is_none());
        assert_eq!(out.warnings.len(), 2);
    }

    #[test]
    fn is_valid_jira_label_accepts_typical_labels() {
        assert!(is_valid_jira_label("alpha"));
        assert!(is_valid_jira_label("tech-debt"));
        assert!(is_valid_jira_label("needs_review"));
    }

    #[test]
    fn is_valid_jira_label_rejects_whitespace_control_and_empty() {
        assert!(!is_valid_jira_label(""));
        assert!(!is_valid_jira_label("has space"));
        assert!(!is_valid_jira_label("has\ttab"));
        assert!(!is_valid_jira_label("has\nnewline"));
        assert!(!is_valid_jira_label("bell\x07"));
    }

    // --- PLG-JIRA-PARENT: parent field emission ---

    #[test]
    fn parent_field_emitted_when_parent_state_has_valid_key() {
        let mut t = task();
        t.parent_plugin_state = Some(json!({ "key": "PROJ-7" }));
        let out = build_issue_payload(&t, &cfg("PROJ"));
        assert_eq!(out.body["fields"]["parent"]["key"], "PROJ-7");
        assert!(out.warnings.is_empty());
    }

    #[test]
    fn parent_field_absent_when_parent_state_missing() {
        let out = build_issue_payload(&task(), &cfg("PROJ"));
        assert!(out.body["fields"].get("parent").is_none());
        assert!(out.warnings.is_empty());
    }

    #[test]
    fn parent_field_absent_when_key_missing_from_state() {
        // Parent was pushed but the state blob has no key
        // (e.g. a 204 no-body response); no warning — the
        // host simply has no parent key available yet.
        let mut t = task();
        t.parent_plugin_state = Some(json!({ "other": "field" }));
        let out = build_issue_payload(&t, &cfg("PROJ"));
        assert!(out.body["fields"].get("parent").is_none());
        assert!(out.warnings.is_empty());
    }

    #[test]
    fn invalid_parent_key_warns_and_omits_parent_field() {
        let mut t = task();
        t.parent_plugin_state = Some(json!({ "key": "../../admin" }));
        let out = build_issue_payload(&t, &cfg("PROJ"));
        assert!(out.body["fields"].get("parent").is_none());
        assert_eq!(
            out.warnings,
            vec![MappingWarning::InvalidParentKey("../../admin".into())]
        );
    }

    #[test]
    fn legacy_epic_link_custom_field_emitted_alongside_parent_key() {
        let mut c = cfg("PROJ");
        c.epic_link_custom_field = Some("customfield_10014".into());
        let mut t = task();
        t.parent_plugin_state = Some(json!({ "key": "PROJ-9" }));
        let out = build_issue_payload(&t, &c);
        assert_eq!(out.body["fields"]["parent"]["key"], "PROJ-9");
        assert_eq!(out.body["fields"]["customfield_10014"], "PROJ-9");
    }

    #[test]
    fn legacy_custom_field_omitted_when_no_parent_link() {
        let mut c = cfg("PROJ");
        c.epic_link_custom_field = Some("customfield_10014".into());
        // No parent state → neither field emitted.
        let out = build_issue_payload(&task(), &c);
        assert!(out.body["fields"].get("parent").is_none());
        assert!(out.body["fields"].get("customfield_10014").is_none());
    }

    #[test]
    fn mapping_warning_display_is_stable_for_each_variant() {
        assert_eq!(
            MappingWarning::UnmappedAssignee("a@b".into()).to_string(),
            "assignee 'a@b' has no entry in assignee_map; skipped"
        );
        assert_eq!(
            MappingWarning::UnmappedPriority(5).to_string(),
            "complexity 5 has no entry in priority_map; skipped"
        );
        assert!(MappingWarning::RejectedLabel("x y".into())
            .to_string()
            .contains("'x y'"));
        assert!(MappingWarning::TransitionHttp {
            transition_id: "11".into(),
            status: 400,
            body: "nope".into(),
        }
        .to_string()
        .contains("transition to 11"));
        assert!(MappingWarning::TransitionTransport {
            transition_id: "11".into(),
            message: "reset".into(),
        }
        .to_string()
        .contains("reset"));
    }

    #[test]
    fn adf_doc_is_valid_even_when_input_empty() {
        let v = adf_doc("");
        assert_eq!(v["type"], "doc");
        assert_eq!(v["content"].as_array().unwrap().len(), 1);
        assert_eq!(v["content"][0]["type"], "paragraph");
    }
}

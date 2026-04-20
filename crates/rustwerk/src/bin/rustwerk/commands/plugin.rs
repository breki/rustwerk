//! `rustwerk plugin list` / `rustwerk plugin push`
//! subcommand implementation.
//!
//! This module is the only place outside `plugin_host.rs`
//! that knows how to invoke a dynamic plugin — everything
//! else in the binary operates on plain domain types. The
//! commands are feature-gated behind `plugins` in the
//! parent `commands` module so a `--no-default-features`
//! build stays dynamic-loader-free.

use std::env;
use std::io::{self, Write};

use anyhow::{anyhow, bail, Context, Result};
use rustwerk_plugin_api::{PluginResult, TaskDto, TaskPushResult, TaskStatusDto};
use serde::Serialize;
use serde_json::{json, Map, Value};

use rustwerk::domain::project::Project;
use rustwerk::domain::task::{Status, Task, TaskId};

use crate::git;
use crate::load_project;
use crate::plugin_host::{self, LoadedPlugin};
use crate::render::RenderText;

// ---------------------------------------------------------------
// `plugin list`
// ---------------------------------------------------------------

/// One row of the `plugin list` output.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct PluginListItem {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) description: String,
    pub(crate) capabilities: Vec<String>,
    pub(crate) path: String,
}

/// Result of `plugin list`.
#[derive(Debug, Serialize)]
pub(crate) struct PluginListOutput {
    pub(crate) plugins: Vec<PluginListItem>,
}

impl RenderText for PluginListOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        if self.plugins.is_empty() {
            return writeln!(w, "No plugins installed.");
        }
        for p in &self.plugins {
            writeln!(w, "{} (v{}) — {}", p.name, p.version, p.description)?;
            writeln!(w, "  capabilities: {}", p.capabilities.join(", "))?;
            writeln!(w, "  path: {}", p.path)?;
        }
        Ok(())
    }
}

/// Enumerate installed plugins. Does not require a
/// rustwerk project — when invoked outside one, we scan
/// the user-scoped `~/.rustwerk/plugins/` only and
/// return whatever is installed globally.
pub(crate) fn cmd_plugin_list() -> Result<PluginListOutput> {
    let root = load_project().map(|(r, _)| r).or_else(|_| {
        env::current_dir().context("failed to get current directory")
    })?;
    let loaded = plugin_host::discover_plugins(&root);
    Ok(PluginListOutput {
        plugins: loaded.iter().map(to_list_item).collect(),
    })
}

fn to_list_item(p: &LoadedPlugin) -> PluginListItem {
    let info = p.info();
    PluginListItem {
        name: info.name.clone(),
        version: info.version.clone(),
        description: info.description.clone(),
        capabilities: info.capabilities.clone(),
        path: p.source_path().display().to_string(),
    }
}

// ---------------------------------------------------------------
// `plugin push`
// ---------------------------------------------------------------

/// Output of `plugin push` — success / failure is
/// determined by the inner `PluginResult::success` flag
/// for non-dry-run calls, or always-success for
/// `--dry-run`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "mode")]
pub(crate) enum PluginPushOutput {
    /// The plugin was actually invoked.
    Executed {
        plugin: String,
        result: PluginResult,
    },
    /// `--dry-run`: no plugin call was made; the payload
    /// summary is echoed back.
    DryRun {
        plugin: String,
        tasks: Vec<String>,
        config_keys: Vec<String>,
    },
}

impl PluginPushOutput {
    /// `true` when the output represents a successful
    /// run. Dry runs are always treated as successful;
    /// executed runs inherit the plugin's aggregate
    /// `success` flag. Used by the caller to decide the
    /// process exit code.
    pub(crate) fn is_success(&self) -> bool {
        match self {
            Self::Executed { result, .. } => result.success,
            Self::DryRun { .. } => true,
        }
    }
}

impl RenderText for PluginPushOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        match self {
            Self::Executed { plugin, result } => {
                render_push_text(plugin, result, w)
            }
            Self::DryRun {
                plugin,
                tasks,
                config_keys,
            } => render_dry_run_text(plugin, tasks, config_keys, w),
        }
    }
}

/// Dispatch for `plugin push`. Always returns `Ok` so
/// `render::emit` runs and per-task detail reaches the
/// user; the caller inspects [`PluginPushOutput::is_success`]
/// to decide the process exit code.
pub(crate) fn cmd_plugin_push(
    name: &str,
    project_key: Option<&str>,
    tasks_filter: Option<&str>,
    dry_run: bool,
) -> Result<PluginPushOutput> {
    let (root, project) = load_project()?;
    let selected = filter_tasks(&project, tasks_filter)?;
    let dtos: Vec<TaskDto> =
        selected.iter().map(|(id, t)| task_to_dto(id, t)).collect();

    let config = assemble_config(
        env::var("JIRA_URL").ok().as_deref(),
        env::var("JIRA_TOKEN").ok().as_deref(),
        git::user_email().as_deref(),
        project_key,
    );

    if dry_run {
        return Ok(PluginPushOutput::DryRun {
            plugin: name.to_string(),
            tasks: dtos.iter().map(|t| t.id.clone()).collect(),
            config_keys: config_key_names(&config),
        });
    }

    let plugins = plugin_host::discover_plugins(&root);
    let loaded = plugins
        .iter()
        .find(|p| p.info().name == name)
        .ok_or_else(|| {
            let available: Vec<String> =
                plugins.iter().map(|p| p.info().name.clone()).collect();
            anyhow!(
                "unknown plugin: {name} (available: {})",
                available.join(", ")
            )
        })?;

    let config_json = serde_json::to_string(&config)
        .context("failed to serialize plugin config")?;
    let tasks_json = serde_json::to_string(&dtos)
        .context("failed to serialize tasks payload")?;
    let result = loaded
        .push_tasks(&config_json, &tasks_json)
        .with_context(|| format!("plugin '{name}' push failed"))?;

    Ok(PluginPushOutput::Executed {
        plugin: name.to_string(),
        result,
    })
}

// ---------------------------------------------------------------
// Pure helpers (directly unit-testable)
// ---------------------------------------------------------------

/// Convert a domain `Task` to the FFI-portable
/// [`TaskDto`].
fn task_to_dto(id: &TaskId, task: &Task) -> TaskDto {
    TaskDto {
        id: id.as_str().to_string(),
        title: task.title.clone(),
        description: task.description.clone().unwrap_or_default(),
        status: status_to_dto(task.status),
        dependencies: task
            .dependencies
            .iter()
            .map(|d| d.as_str().to_string())
            .collect(),
        effort_estimate: task
            .effort_estimate
            .as_ref()
            .map(std::string::ToString::to_string),
        complexity: task.complexity,
        assignee: task.assignee.clone(),
        tags: task.tags.iter().map(|t| t.as_str().to_string()).collect(),
    }
}

fn status_to_dto(s: Status) -> TaskStatusDto {
    match s {
        Status::Todo => TaskStatusDto::Todo,
        Status::InProgress => TaskStatusDto::InProgress,
        Status::Blocked => TaskStatusDto::Blocked,
        Status::Done => TaskStatusDto::Done,
        Status::OnHold => TaskStatusDto::OnHold,
    }
}

/// Build the plugin config JSON from four optional
/// sources. Keys with `None` values are omitted entirely
/// so the plugin can distinguish "absent" from "empty".
fn assemble_config(
    jira_url: Option<&str>,
    jira_token: Option<&str>,
    username: Option<&str>,
    project_key: Option<&str>,
) -> Value {
    let mut map = Map::new();
    if let Some(v) = jira_url.filter(|s| !s.is_empty()) {
        map.insert("jira_url".into(), json!(v));
    }
    if let Some(v) = jira_token.filter(|s| !s.is_empty()) {
        map.insert("jira_token".into(), json!(v));
    }
    if let Some(v) = username.filter(|s| !s.is_empty()) {
        map.insert("username".into(), json!(v));
    }
    if let Some(v) = project_key.filter(|s| !s.is_empty()) {
        map.insert("project_key".into(), json!(v));
    }
    Value::Object(map)
}

/// Return the keys present in an assembled config
/// object. Used by `--dry-run` reports so the operator
/// can see which sources resolved without exposing the
/// token value.
fn config_key_names(config: &Value) -> Vec<String> {
    config
        .as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default()
}

/// Parse the optional `--tasks ID,ID,...` flag into a
/// list of task IDs, stripping whitespace and rejecting
/// empty entries ("`,A,`" is user-visible nonsense).
fn parse_tasks_filter(raw: &str) -> Result<Vec<String>> {
    let ids: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if ids.is_empty() {
        bail!("--tasks requires at least one task ID");
    }
    Ok(ids)
}

/// Resolve `--tasks` (or "all tasks" when `None`) into a
/// list of `(TaskId, &Task)` tuples. The `TaskId` is
/// cloned (cheap — a single `String`) so the result does
/// not depend on a temporary lookup key outliving its
/// scope. Errors name the first missing ID.
fn filter_tasks<'a>(
    project: &'a Project,
    tasks_filter: Option<&str>,
) -> Result<Vec<(TaskId, &'a Task)>> {
    let Some(raw) = tasks_filter else {
        return Ok(project
            .tasks
            .iter()
            .map(|(k, v)| (k.clone(), v))
            .collect());
    };
    let wanted = parse_tasks_filter(raw)?;
    let mut resolved = Vec::with_capacity(wanted.len());
    for id in &wanted {
        let task_id = TaskId::new(id)
            .with_context(|| format!("invalid task ID: {id}"))?;
        let (stored_key, task) = project
            .tasks
            .get_key_value(&task_id)
            .ok_or_else(|| anyhow!("unknown task ID: {id}"))?;
        resolved.push((stored_key.clone(), task));
    }
    Ok(resolved)
}

// ---------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------

fn render_push_text(
    plugin: &str,
    result: &PluginResult,
    w: &mut dyn Write,
) -> io::Result<()> {
    let marker = if result.success { "[ok]" } else { "[fail]" };
    writeln!(w, "{marker} {plugin}: {}", result.message)?;
    if let Some(task_results) = &result.task_results {
        for r in task_results {
            render_task_result(r, w)?;
        }
    }
    Ok(())
}

fn render_task_result(r: &TaskPushResult, w: &mut dyn Write) -> io::Result<()> {
    if r.success {
        match &r.external_key {
            Some(k) => writeln!(w, "  [ok]   {} -> {k}", r.task_id),
            None => writeln!(w, "  [ok]   {}: {}", r.task_id, r.message),
        }
    } else {
        writeln!(w, "  [fail] {}: {}", r.task_id, r.message)
    }
}

fn render_dry_run_text(
    plugin: &str,
    tasks: &[String],
    config_keys: &[String],
    w: &mut dyn Write,
) -> io::Result<()> {
    writeln!(w, "dry run: would push {} task(s) to '{plugin}'", tasks.len())?;
    writeln!(w, "  config keys present: [{}]", config_keys.join(", "))?;
    for id in tasks {
        writeln!(w, "  - {id}")?;
    }
    Ok(())
}

// ---------------------------------------------------------------
// Tests
// ---------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rustwerk::domain::task::{Effort, EffortUnit, Tag};

    fn task_fixture() -> Task {
        Task {
            title: "Title".into(),
            description: Some("Desc".into()),
            status: Status::InProgress,
            dependencies: vec![TaskId::new("PLG-API").unwrap()],
            effort_estimate: Some(Effort {
                value: 2.5,
                unit: EffortUnit::H,
            }),
            complexity: Some(5),
            assignee: Some("igor".into()),
            effort_entries: vec![],
            tags: vec![Tag::new("plugin").unwrap(), Tag::new("api").unwrap()],
        }
    }

    #[test]
    fn task_to_dto_maps_all_fields() {
        let id = TaskId::new("PLG-CLI").unwrap();
        let dto = task_to_dto(&id, &task_fixture());
        assert_eq!(dto.id, "PLG-CLI");
        assert_eq!(dto.title, "Title");
        assert_eq!(dto.description, "Desc");
        assert_eq!(dto.status, TaskStatusDto::InProgress);
        assert_eq!(dto.dependencies, vec!["PLG-API".to_string()]);
        assert_eq!(dto.effort_estimate.as_deref(), Some("2.5H"));
        assert_eq!(dto.complexity, Some(5));
        assert_eq!(dto.assignee.as_deref(), Some("igor"));
        assert_eq!(dto.tags, vec!["plugin".to_string(), "api".to_string()]);
    }

    #[test]
    fn task_to_dto_handles_missing_optional_fields() {
        let id = TaskId::new("T1").unwrap();
        let mut t = task_fixture();
        t.description = None;
        t.effort_estimate = None;
        t.complexity = None;
        t.assignee = None;
        t.tags.clear();
        t.dependencies.clear();
        let dto = task_to_dto(&id, &t);
        assert_eq!(dto.description, "");
        assert!(dto.dependencies.is_empty());
        assert!(dto.tags.is_empty());
        assert_eq!(dto.effort_estimate, None);
        assert_eq!(dto.complexity, None);
        assert_eq!(dto.assignee, None);
    }

    #[test]
    fn status_to_dto_covers_all_variants() {
        assert_eq!(status_to_dto(Status::Todo), TaskStatusDto::Todo);
        assert_eq!(
            status_to_dto(Status::InProgress),
            TaskStatusDto::InProgress
        );
        assert_eq!(status_to_dto(Status::Blocked), TaskStatusDto::Blocked);
        assert_eq!(status_to_dto(Status::Done), TaskStatusDto::Done);
        assert_eq!(status_to_dto(Status::OnHold), TaskStatusDto::OnHold);
    }

    #[test]
    fn assemble_config_omits_absent_keys() {
        let cfg = assemble_config(Some("https://x.atlassian.net"), None, None, None);
        let keys = config_key_names(&cfg);
        assert_eq!(keys, vec!["jira_url".to_string()]);
    }

    #[test]
    fn assemble_config_omits_empty_strings() {
        let cfg = assemble_config(Some(""), Some(""), Some(""), Some(""));
        assert!(cfg.as_object().unwrap().is_empty());
    }

    #[test]
    fn assemble_config_includes_all_present_sources() {
        let cfg = assemble_config(
            Some("https://x.atlassian.net"),
            Some("tok"),
            Some("u@example.com"),
            Some("PROJ"),
        );
        let obj = cfg.as_object().unwrap();
        assert_eq!(obj["jira_url"], "https://x.atlassian.net");
        assert_eq!(obj["jira_token"], "tok");
        assert_eq!(obj["username"], "u@example.com");
        assert_eq!(obj["project_key"], "PROJ");
    }

    #[test]
    fn config_key_names_returns_sorted_keys_when_fully_populated() {
        let cfg = assemble_config(
            Some("u"),
            Some("t"),
            Some("n"),
            Some("p"),
        );
        let mut keys = config_key_names(&cfg);
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "jira_token".to_string(),
                "jira_url".to_string(),
                "project_key".to_string(),
                "username".to_string(),
            ]
        );
    }

    #[test]
    fn parse_tasks_filter_splits_and_trims() {
        let out = parse_tasks_filter(" A , B ,C ").unwrap();
        assert_eq!(out, vec!["A", "B", "C"]);
    }

    #[test]
    fn parse_tasks_filter_drops_empty_entries() {
        let out = parse_tasks_filter("A,,B").unwrap();
        assert_eq!(out, vec!["A", "B"]);
    }

    #[test]
    fn parse_tasks_filter_errors_on_only_separators() {
        let err = parse_tasks_filter(" , , ").unwrap_err();
        assert!(format!("{err}").contains("at least one"));
    }

    fn build_project_with(task_ids: &[&str]) -> Project {
        let mut p = Project::new("test").unwrap();
        for id in task_ids {
            let tid = TaskId::new(id).unwrap();
            p.add_task(tid, Task::new("t").unwrap()).unwrap();
        }
        p
    }

    #[test]
    fn filter_tasks_returns_all_when_filter_absent() {
        let p = build_project_with(&["T-1", "T-2"]);
        let selected = filter_tasks(&p, None).unwrap();
        assert_eq!(selected.len(), 2);
    }

    #[test]
    fn filter_tasks_resolves_named_subset() {
        let p = build_project_with(&["T-1", "T-2", "T-3"]);
        let selected = filter_tasks(&p, Some("T-1, T-3")).unwrap();
        let ids: Vec<&str> =
            selected.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, vec!["T-1", "T-3"]);
    }

    #[test]
    fn filter_tasks_errors_on_unknown_id() {
        let p = build_project_with(&["T-1"]);
        let err = filter_tasks(&p, Some("T-1,T-404")).unwrap_err();
        assert!(format!("{err}").contains("T-404"));
    }

    #[test]
    fn render_push_text_shows_success() {
        let result = PluginResult {
            success: true,
            message: "2 task(s) pushed to Jira".into(),
            task_results: Some(vec![
                TaskPushResult {
                    task_id: "PLG-API".into(),
                    success: true,
                    message: "ok".into(),
                    external_key: Some("PROJ-1".into()),
                },
                TaskPushResult {
                    task_id: "PLG-HOST".into(),
                    success: true,
                    message: "created".into(),
                    external_key: Some("PROJ-2".into()),
                },
            ]),
        };
        let mut buf = Vec::new();
        render_push_text("jira", &result, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("[ok] jira: 2 task(s)"));
        assert!(out.contains("PLG-API -> PROJ-1"));
        assert!(out.contains("PLG-HOST -> PROJ-2"));
    }

    #[test]
    fn render_push_text_shows_failure() {
        let result = PluginResult {
            success: false,
            message: "1 of 2 task(s) failed".into(),
            task_results: Some(vec![
                TaskPushResult {
                    task_id: "A".into(),
                    success: true,
                    message: "".into(),
                    external_key: Some("PROJ-1".into()),
                },
                TaskPushResult {
                    task_id: "B".into(),
                    success: false,
                    message: "HTTP 500".into(),
                    external_key: None,
                },
            ]),
        };
        let mut buf = Vec::new();
        render_push_text("jira", &result, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("[fail] jira:"));
        assert!(out.contains("[fail] B: HTTP 500"));
    }

    #[test]
    fn dry_run_render_does_not_leak_token_value() {
        // Even if a caller somehow passed the token value
        // as a config *name*, the renderer must only emit
        // the key names — proven by never calling into
        // the config values at all.
        let tasks = vec!["T-1".to_string()];
        let keys = vec!["jira_url".into(), "jira_token".into()];
        let mut buf = Vec::new();
        render_dry_run_text("jira", &tasks, &keys, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("jira_token"));
        assert!(out.contains("T-1"));
        // Sanity: a real secret value would never be in
        // the rendered output because only keys are
        // passed through.
    }

    #[test]
    fn plugin_list_output_renders_empty_hint() {
        let out = PluginListOutput { plugins: vec![] };
        let mut buf = Vec::new();
        out.render_text(&mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("No plugins installed."));
    }

    #[test]
    fn is_success_true_for_dry_run() {
        let out = PluginPushOutput::DryRun {
            plugin: "x".into(),
            tasks: vec![],
            config_keys: vec![],
        };
        assert!(out.is_success());
    }

    #[test]
    fn is_success_reflects_executed_result_success_flag() {
        let ok = PluginPushOutput::Executed {
            plugin: "x".into(),
            result: PluginResult {
                success: true,
                message: "m".into(),
                task_results: None,
            },
        };
        assert!(ok.is_success());

        let bad = PluginPushOutput::Executed {
            plugin: "x".into(),
            result: PluginResult {
                success: false,
                message: "m".into(),
                task_results: None,
            },
        };
        assert!(!bad.is_success());
    }

    #[test]
    fn config_key_names_on_empty_config_is_empty() {
        let cfg = assemble_config(None, None, None, None);
        assert!(config_key_names(&cfg).is_empty());
    }

    #[test]
    fn plugin_push_output_render_dry_run_dispatches() {
        let out = PluginPushOutput::DryRun {
            plugin: "jira".into(),
            tasks: vec!["A".into()],
            config_keys: vec!["project_key".into()],
        };
        let mut buf = Vec::new();
        out.render_text(&mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("dry run"));
        assert!(s.contains("project_key"));
        assert!(s.contains("- A"));
    }

    #[test]
    fn plugin_push_output_render_executed_dispatches() {
        let out = PluginPushOutput::Executed {
            plugin: "jira".into(),
            result: PluginResult {
                success: true,
                message: "1 task(s) pushed to Jira".into(),
                task_results: Some(vec![TaskPushResult {
                    task_id: "A".into(),
                    success: true,
                    message: "ok".into(),
                    external_key: Some("PROJ-9".into()),
                }]),
            },
        };
        let mut buf = Vec::new();
        out.render_text(&mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("[ok] jira:"));
        assert!(s.contains("A -> PROJ-9"));
    }

    #[test]
    fn render_task_result_success_without_external_key_uses_message() {
        let r = TaskPushResult {
            task_id: "X".into(),
            success: true,
            message: "created (HTTP 201)".into(),
            external_key: None,
        };
        let mut buf = Vec::new();
        render_task_result(&r, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("[ok]") && s.contains("X: created"));
    }

    #[test]
    fn render_push_text_without_task_results_still_renders_header() {
        let result = PluginResult {
            success: true,
            message: "no-op".into(),
            task_results: None,
        };
        let mut buf = Vec::new();
        render_push_text("jira", &result, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("[ok] jira: no-op"));
    }

    #[test]
    fn plugin_list_output_renders_items() {
        let out = PluginListOutput {
            plugins: vec![PluginListItem {
                name: "jira".into(),
                version: "0.1.0".into(),
                description: "Push tasks to Jira".into(),
                capabilities: vec!["push_tasks".into()],
                path: "/path/to.dll".into(),
            }],
        };
        let mut buf = Vec::new();
        out.render_text(&mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("jira (v0.1.0) — Push tasks to Jira"));
        assert!(s.contains("capabilities: push_tasks"));
        assert!(s.contains("path: /path/to.dll"));
    }
}

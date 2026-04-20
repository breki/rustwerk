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
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

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
// `plugin install`
// ---------------------------------------------------------------

/// Where an `install` writes the plugin.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InstallScope {
    /// `./.rustwerk/plugins/` in the current project.
    Project,
    /// `$HOME/.rustwerk/plugins/` (or `%USERPROFILE%\…`).
    User,
}

impl fmt::Display for InstallScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Project => f.write_str("project"),
            Self::User => f.write_str("user"),
        }
    }
}

/// Result of `plugin install`. The destination path is
/// carried inside `installed.path` (filled in by the
/// verifier at the final copy location) — no separate
/// `destination` field so JSON consumers have a single
/// source of truth.
#[derive(Debug, Serialize)]
pub(crate) struct PluginInstallOutput {
    /// Discovered metadata for the freshly installed
    /// plugin. `installed.path` is the final destination
    /// on disk.
    pub(crate) installed: PluginListItem,
    /// Scope the install landed in.
    pub(crate) scope: InstallScope,
    /// `true` when an existing plugin was overwritten.
    pub(crate) replaced: bool,
}

impl RenderText for PluginInstallOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        let verb = if self.replaced {
            "Reinstalled"
        } else {
            "Installed"
        };
        writeln!(
            w,
            "{verb} {} (v{}) — {}",
            self.installed.name,
            self.installed.version,
            self.installed.description
        )?;
        writeln!(w, "  capabilities: {}", self.installed.capabilities.join(", "))?;
        writeln!(w, "  scope: {}", self.scope)?;
        writeln!(w, "  path: {}", self.installed.path)?;
        Ok(())
    }
}

/// Outcome of a successful [`install_from_path`] call.
/// Private — exists only to avoid a bare
/// `(PluginListItem, bool)` tuple return.
#[derive(Debug)]
struct InstallOutcome {
    info: PluginListItem,
    replaced: bool,
}

/// Dispatch for `plugin install`. Resolves the target
/// directory, delegates the copy + verify to
/// [`install_from_path`] with the production verifier,
/// then shapes the result into an output DTO.
///
/// `--scope project` requires a loaded rustwerk project
/// so `plugin install` never materialises a stray
/// `.rustwerk/plugins/` tree in whatever directory the
/// user happened to be in. `--scope user` is independent
/// of the working directory.
pub(crate) fn cmd_plugin_install(
    source: &Path,
    scope: InstallScope,
    force: bool,
) -> Result<PluginInstallOutput> {
    validate_cdylib_extension(source)?;

    let project_root = match scope {
        InstallScope::Project => Some(load_project().context(
            "plugin install --scope project requires a rustwerk project — run `rustwerk init` or pass --scope user",
        )?.0),
        InstallScope::User => None,
    };
    let home = plugin_host::home_dir();
    let dest_dir =
        resolve_scope_dir(scope, project_root.as_deref(), home.as_deref())?;
    fs::create_dir_all(&dest_dir).with_context(|| {
        format!("failed to create plugin directory {}", dest_dir.display())
    })?;

    let outcome = install_from_path(source, &dest_dir, force, production_verify)?;

    Ok(PluginInstallOutput {
        installed: outcome.info,
        scope,
        replaced: outcome.replaced,
    })
}

/// Production verifier: load the cdylib at `path` through
/// the same host entry point `plugin list` uses and shape
/// the metadata into a [`PluginListItem`] so the success
/// message matches what `plugin list` would later print.
fn production_verify(path: &Path) -> Result<PluginListItem> {
    let loaded = plugin_host::load_plugin(path)?;
    Ok(to_list_item(&loaded))
}

/// Reject a `source` whose extension doesn't match the
/// host OS's dynamic-library convention. Runs before any
/// filesystem mutation so a wrong-extension source never
/// touches the destination directory.
fn validate_cdylib_extension(source: &Path) -> Result<()> {
    let ext = source.extension().and_then(|e| e.to_str()).ok_or_else(|| {
        anyhow!(
            "source {} has no file extension (expected .{})",
            source.display(),
            plugin_host::DYLIB_EXT
        )
    })?;
    if !ext.eq_ignore_ascii_case(plugin_host::DYLIB_EXT) {
        bail!(
            "source {} has extension .{} but this platform expects .{}",
            source.display(),
            ext,
            plugin_host::DYLIB_EXT
        );
    }
    Ok(())
}

/// Resolve the on-disk target directory for `scope`.
/// `project_root` and `home` are accepted as arguments
/// (rather than read from the environment inside) so
/// tests exercise each missing-dependency path
/// deterministically. A `None` in the arm the scope
/// actually needs is a hard error with a clear message.
fn resolve_scope_dir(
    scope: InstallScope,
    project_root: Option<&Path>,
    home: Option<&Path>,
) -> Result<PathBuf> {
    match scope {
        InstallScope::Project => project_root
            .map(|r| r.join(".rustwerk").join("plugins"))
            .ok_or_else(|| {
                anyhow!(
                    "--scope project requires a rustwerk project; run `rustwerk init` or pass --scope user"
                )
            }),
        InstallScope::User => home
            .map(|h| h.join(".rustwerk").join("plugins"))
            .ok_or_else(|| {
                anyhow!(
                    "--scope user requires HOME (Unix) or USERPROFILE (Windows) to be set"
                )
            }),
    }
}

/// Copy `source` into `dest_dir` (preserving the source
/// filename), then verify the copy is a valid plugin.
/// On verification failure the copy is removed so a
/// failed install never leaves a partially-populated
/// `plugins/` directory behind.
///
/// `verify` is accepted as a generic closure so tests
/// can substitute a deterministic fake without paying
/// for dynamic dispatch in production. It is
/// [`production_verify`] at the real call site.
///
/// Safety gates run **before** the copy:
/// 1. Reject when `dest` is an existing symlink —
///    `fs::copy` would follow it and write plugin bytes
///    outside the plugins directory.
/// 2. Reject when `source` and `dest` resolve to the
///    same file — `fs::copy` of a file onto itself
///    truncates it and would then be deleted by the
///    verify-failure cleanup, silently destroying an
///    already-installed plugin.
fn install_from_path(
    source: &Path,
    dest_dir: &Path,
    force: bool,
    verify: impl Fn(&Path) -> Result<PluginListItem>,
) -> Result<InstallOutcome> {
    let filename = source.file_name().ok_or_else(|| {
        anyhow!("source path has no filename: {}", source.display())
    })?;
    let dest = dest_dir.join(filename);

    reject_symlink_dest(&dest)?;
    reject_self_copy(source, &dest)?;

    let replaced = dest.exists();
    if replaced && !force {
        bail!(
            "plugin already installed at {}; pass --force to overwrite",
            dest.display()
        );
    }

    fs::copy(source, &dest).with_context(|| {
        format!(
            "failed to copy {} to {}",
            source.display(),
            dest.display()
        )
    })?;

    match verify(&dest) {
        Ok(info) => Ok(InstallOutcome { info, replaced }),
        Err(e) => {
            // Drop the half-installed copy before
            // surfacing the error. Remove errors are
            // swallowed — the verification failure is
            // the interesting one to report.
            let _ = fs::remove_file(&dest);
            Err(e.context(format!(
                "post-copy verification failed; removed {}",
                dest.display()
            )))
        }
    }
}

/// Bail when `dest` already exists as a symlink. Passes
/// through the `NotFound` case (no pre-existing file —
/// fresh install) but surfaces other stat errors so a
/// weird permission failure isn't silently swallowed.
fn reject_symlink_dest(dest: &Path) -> Result<()> {
    match fs::symlink_metadata(dest) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                bail!(
                    "refusing to overwrite a symlink at {}; delete it first if intentional",
                    dest.display()
                );
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(anyhow::Error::from(e).context(format!(
            "failed to stat destination {}",
            dest.display()
        ))),
    }
}

/// Bail when `source` and `dest` resolve to the same
/// on-disk file. Only runs when `dest` already exists —
/// a fresh install can't collide with itself. Both
/// canonicalisation failures are tolerated (the real
/// copy call surfaces the underlying error).
fn reject_self_copy(source: &Path, dest: &Path) -> Result<()> {
    if !dest.exists() {
        return Ok(());
    }
    let Ok(canon_src) = source.canonicalize() else {
        return Ok(());
    };
    let Ok(canon_dst) = dest.canonicalize() else {
        return Ok(());
    };
    if canon_src == canon_dst {
        bail!(
            "source and destination resolve to the same file ({}); refusing to copy a file onto itself",
            canon_dst.display()
        );
    }
    Ok(())
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

    // -----------------------------------------
    // plugin install: pure helpers
    // -----------------------------------------

    #[test]
    fn validate_cdylib_extension_accepts_host_extension() {
        let name = format!("p.{}", plugin_host::DYLIB_EXT);
        assert!(validate_cdylib_extension(Path::new(&name)).is_ok());
    }

    #[test]
    fn validate_cdylib_extension_is_case_insensitive() {
        let name = format!("p.{}", plugin_host::DYLIB_EXT.to_uppercase());
        assert!(validate_cdylib_extension(Path::new(&name)).is_ok());
    }

    #[test]
    fn validate_cdylib_extension_rejects_missing_extension() {
        let err = validate_cdylib_extension(Path::new("plugin")).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("no file extension"), "got: {msg}");
    }

    #[test]
    fn validate_cdylib_extension_rejects_wrong_extension() {
        let err =
            validate_cdylib_extension(Path::new("plugin.txt")).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains(".txt"), "got: {msg}");
        assert!(
            msg.contains(plugin_host::DYLIB_EXT),
            "got: {msg}"
        );
    }

    #[test]
    fn resolve_scope_dir_project_uses_project_root() {
        let root = Path::new("/work/project");
        let home = Path::new("/home/u");
        let dir =
            resolve_scope_dir(InstallScope::Project, Some(root), Some(home))
                .unwrap();
        assert_eq!(dir, PathBuf::from("/work/project/.rustwerk/plugins"));
    }

    #[test]
    fn resolve_scope_dir_user_uses_home() {
        let root = Path::new("/work/project");
        let home = Path::new("/home/u");
        let dir =
            resolve_scope_dir(InstallScope::User, Some(root), Some(home))
                .unwrap();
        assert_eq!(dir, PathBuf::from("/home/u/.rustwerk/plugins"));
    }

    #[test]
    fn resolve_scope_dir_user_errors_without_home() {
        let root = Path::new("/work/project");
        let err =
            resolve_scope_dir(InstallScope::User, Some(root), None).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("HOME") || msg.contains("USERPROFILE"),
            "got: {msg}"
        );
    }

    #[test]
    fn resolve_scope_dir_project_errors_without_project_root() {
        let home = Path::new("/home/u");
        let err = resolve_scope_dir(InstallScope::Project, None, Some(home))
            .unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("rustwerk project"),
            "got: {msg}"
        );
    }

    // -----------------------------------------
    // plugin install: install_from_path
    // -----------------------------------------

    /// Temp-dir helper matching the pattern already used
    /// in `plugin_host` tests — no extra dev-dep needed.
    fn scratch(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rustwerk-plugin-install-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_fake_cdylib(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, b"fake-cdylib-bytes").unwrap();
        path
    }

    fn fake_info(name: &str) -> PluginListItem {
        PluginListItem {
            name: name.into(),
            version: "0.1.0".into(),
            description: "fake".into(),
            capabilities: vec!["push_tasks".into()],
            path: String::new(),
        }
    }

    #[test]
    fn install_from_path_copies_and_verifies() {
        let src_dir = scratch("copy-src");
        let dst_dir = scratch("copy-dst");
        let src = write_fake_cdylib(
            &src_dir,
            &format!("p.{}", plugin_host::DYLIB_EXT),
        );

        let verify =
            |_: &Path| -> Result<PluginListItem> { Ok(fake_info("p")) };
        let outcome =
            install_from_path(&src, &dst_dir, false, verify).unwrap();

        let expected_dest = dst_dir.join(src.file_name().unwrap());
        assert_eq!(outcome.info.name, "p");
        assert!(expected_dest.exists());
        assert!(!outcome.replaced);

        let _ = std::fs::remove_dir_all(&src_dir);
        let _ = std::fs::remove_dir_all(&dst_dir);
    }

    #[test]
    fn install_from_path_rejects_existing_without_force() {
        let src_dir = scratch("nf-src");
        let dst_dir = scratch("nf-dst");
        let name = format!("p.{}", plugin_host::DYLIB_EXT);
        let src = write_fake_cdylib(&src_dir, &name);
        // Pre-populate the destination.
        std::fs::write(dst_dir.join(&name), b"existing").unwrap();

        let verify = |_: &Path| -> Result<PluginListItem> {
            panic!("verify must not run when install is refused");
        };
        let err =
            install_from_path(&src, &dst_dir, false, verify).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("--force"), "got: {msg}");
        // The existing file must NOT have been clobbered.
        assert_eq!(
            std::fs::read(dst_dir.join(&name)).unwrap(),
            b"existing"
        );

        let _ = std::fs::remove_dir_all(&src_dir);
        let _ = std::fs::remove_dir_all(&dst_dir);
    }

    #[test]
    fn install_from_path_replaces_existing_with_force() {
        let src_dir = scratch("f-src");
        let dst_dir = scratch("f-dst");
        let name = format!("p.{}", plugin_host::DYLIB_EXT);
        let src = write_fake_cdylib(&src_dir, &name);
        std::fs::write(dst_dir.join(&name), b"stale").unwrap();

        let verify =
            |_: &Path| -> Result<PluginListItem> { Ok(fake_info("p")) };
        let outcome =
            install_from_path(&src, &dst_dir, true, verify).unwrap();

        assert!(outcome.replaced);
        let dest = dst_dir.join(&name);
        assert_eq!(std::fs::read(&dest).unwrap(), b"fake-cdylib-bytes");

        let _ = std::fs::remove_dir_all(&src_dir);
        let _ = std::fs::remove_dir_all(&dst_dir);
    }

    #[test]
    fn install_from_path_removes_copy_on_verify_failure() {
        let src_dir = scratch("rb-src");
        let dst_dir = scratch("rb-dst");
        let src = write_fake_cdylib(
            &src_dir,
            &format!("p.{}", plugin_host::DYLIB_EXT),
        );

        let verify = |_: &Path| -> Result<PluginListItem> {
            bail!("fabricated verification failure")
        };
        let err =
            install_from_path(&src, &dst_dir, false, verify).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("verification"), "got: {msg}");
        // Crucial: the half-installed copy is gone.
        assert!(
            !dst_dir.join(src.file_name().unwrap()).exists(),
            "dest should have been removed after verify failure"
        );

        let _ = std::fs::remove_dir_all(&src_dir);
        let _ = std::fs::remove_dir_all(&dst_dir);
    }

    #[test]
    fn install_from_path_errors_on_missing_source() {
        let dst_dir = scratch("ms-dst");
        let missing = std::env::temp_dir().join(format!(
            "rustwerk-nonexistent-{}.{}",
            std::process::id(),
            plugin_host::DYLIB_EXT
        ));

        let verify = |_: &Path| -> Result<PluginListItem> {
            panic!("verify must not run when copy fails");
        };
        let err =
            install_from_path(&missing, &dst_dir, false, verify).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("failed to copy"), "got: {msg}");

        let _ = std::fs::remove_dir_all(&dst_dir);
    }

    #[cfg(unix)]
    #[test]
    fn install_from_path_rejects_symlink_destination() {
        let src_dir = scratch("sym-src");
        let dst_dir = scratch("sym-dst");
        let name = format!("p.{}", plugin_host::DYLIB_EXT);
        let src = write_fake_cdylib(&src_dir, &name);
        // Create a symlink at the destination pointing
        // to an arbitrary file outside plugins/. If the
        // bug is present, fs::copy would write through
        // this link.
        let outside = scratch("sym-outside").join("victim.txt");
        std::fs::write(&outside, b"do not clobber").unwrap();
        std::os::unix::fs::symlink(&outside, dst_dir.join(&name)).unwrap();

        let verify = |_: &Path| -> Result<PluginListItem> {
            panic!("verify must not run when symlink is rejected");
        };
        let err =
            install_from_path(&src, &dst_dir, true, verify).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("symlink"), "got: {msg}");
        // Target of the symlink was not clobbered.
        assert_eq!(
            std::fs::read(&outside).unwrap(),
            b"do not clobber"
        );

        let _ = std::fs::remove_dir_all(&src_dir);
        let _ = std::fs::remove_dir_all(&dst_dir);
    }

    #[test]
    fn install_from_path_rejects_self_copy() {
        // Put a "plugin" in the destination directory
        // and then ask install_from_path to install THAT
        // same file. Without the self-copy guard,
        // fs::copy truncates it to zero bytes and the
        // verify-failure cleanup then deletes it,
        // silently destroying the user's plugin.
        let dst_dir = scratch("self-copy");
        let name = format!("p.{}", plugin_host::DYLIB_EXT);
        let existing = dst_dir.join(&name);
        std::fs::write(&existing, b"already-installed").unwrap();

        let verify = |_: &Path| -> Result<PluginListItem> {
            panic!("verify must not run when self-copy is rejected");
        };
        let err =
            install_from_path(&existing, &dst_dir, true, verify).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("same file"), "got: {msg}");
        // File contents must be intact.
        assert_eq!(std::fs::read(&existing).unwrap(), b"already-installed");

        let _ = std::fs::remove_dir_all(&dst_dir);
    }

    // -----------------------------------------
    // plugin install: rendering
    // -----------------------------------------

    #[test]
    fn plugin_install_output_renders_first_install() {
        let out = PluginInstallOutput {
            installed: PluginListItem {
                name: "jira".into(),
                version: "0.1.0".into(),
                description: "Push tasks to Jira".into(),
                capabilities: vec!["push_tasks".into()],
                path: "/p/.rustwerk/plugins/rustwerk_jira_plugin.dll".into(),
            },
            scope: InstallScope::Project,
            replaced: false,
        };
        let mut buf = Vec::new();
        out.render_text(&mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("Installed jira (v0.1.0) — Push tasks to Jira"));
        assert!(s.contains("scope: project"));
        assert!(s.contains("rustwerk_jira_plugin.dll"));
    }

    #[test]
    fn plugin_install_output_renders_reinstall() {
        let out = PluginInstallOutput {
            installed: PluginListItem {
                name: "jira".into(),
                version: "0.2.0".into(),
                description: "x".into(),
                capabilities: vec!["push_tasks".into()],
                path: "/tmp/x.dll".into(),
            },
            scope: InstallScope::User,
            replaced: true,
        };
        let mut buf = Vec::new();
        out.render_text(&mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("Reinstalled"));
        assert!(s.contains("scope: user"));
    }

    #[test]
    fn install_scope_display_formats_snake_case() {
        assert_eq!(format!("{}", InstallScope::Project), "project");
        assert_eq!(format!("{}", InstallScope::User), "user");
    }

    #[test]
    fn install_scope_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&InstallScope::Project).unwrap(),
            "\"project\""
        );
        assert_eq!(
            serde_json::to_string(&InstallScope::User).unwrap(),
            "\"user\""
        );
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

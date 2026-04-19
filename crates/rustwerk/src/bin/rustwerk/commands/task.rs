//! Task management commands. Public `cmd_*` functions
//! return owned DTOs implementing [`RenderText`]; the
//! top-level dispatch picks the output format.

use std::collections::HashSet;
use std::io::{self, Read, Write};

use anyhow::Result;
use serde::Serialize;

use rustwerk::domain::developer::DeveloperId;
use rustwerk::domain::task::{Effort, Status, Tag, Task, TaskId};
use rustwerk::persistence::file_store;

use crate::render::RenderText;
use crate::{load_project, parse_status, save_project};

/// Maximum size of a task description file served
/// through `task describe --json`. A description on
/// disk that exceeds this is refused rather than
/// loaded into memory.
const MAX_DESCRIBE_BYTES: u64 = 1024 * 1024;

fn parse_tags(tags_str: &str) -> Vec<&str> {
    if tags_str.is_empty() {
        Vec::new()
    } else {
        tags_str.split(',').map(str::trim).collect()
    }
}

/// Shared `(id, title)` DTO used by add/update.
#[derive(Serialize)]
pub(crate) struct TaskRef {
    pub(crate) id: TaskId,
    pub(crate) title: String,
}

/// `task add` output.
#[derive(Serialize)]
pub(crate) struct TaskAddOutput(pub(crate) TaskRef);

impl RenderText for TaskAddOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        writeln!(w, "Created task {}", self.0.id)
    }
}

pub(crate) fn cmd_task_add(
    title: &str,
    id: Option<&str>,
    desc: Option<&str>,
    complexity: Option<u32>,
    effort: Option<&str>,
    tags: Option<&str>,
) -> Result<TaskAddOutput> {
    let (root, mut project) = load_project()?;
    let mut task = Task::new(title)?;
    task.description = desc.map(String::from);
    if let Some(c) = complexity {
        task.set_complexity(c)?;
    }
    if let Some(e) = effort {
        task.effort_estimate = Some(Effort::parse(e)?);
    }
    if let Some(t) = tags {
        let tag_list = parse_tags(t);
        task.set_tags(&tag_list)?;
    }
    let task_id = if let Some(id_str) = id {
        let tid = TaskId::new(id_str)?;
        project.add_task(tid.clone(), task)?;
        tid
    } else {
        project.add_task_auto(task)
    };
    save_project(&root, &project)?;
    let title = project.tasks[&task_id].title.clone();
    Ok(TaskAddOutput(TaskRef { id: task_id, title }))
}

/// Assignment state DTO for both `assign` and
/// `unassign` (`assignee` is `null` for unassign).
#[derive(Serialize)]
pub(crate) struct TaskAssignOutput {
    pub(crate) id: TaskId,
    pub(crate) assignee: Option<DeveloperId>,
}

impl RenderText for TaskAssignOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        match &self.assignee {
            Some(dev) => writeln!(w, "{}: assigned to {dev}", self.id),
            None => writeln!(w, "{}: unassigned", self.id),
        }
    }
}

pub(crate) fn cmd_task_assign(id: &str, to: &str) -> Result<TaskAssignOutput> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)?;
    let dev_id = DeveloperId::new(to)?;
    project.assign(&task_id, &dev_id)?;
    save_project(&root, &project)?;
    Ok(TaskAssignOutput {
        id: task_id,
        assignee: Some(dev_id),
    })
}

pub(crate) fn cmd_task_unassign(id: &str) -> Result<TaskAssignOutput> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)?;
    project.unassign(&task_id)?;
    save_project(&root, &project)?;
    Ok(TaskAssignOutput {
        id: task_id,
        assignee: None,
    })
}

/// `task remove` output.
#[derive(Serialize)]
pub(crate) struct TaskRemoveOutput {
    pub(crate) id: TaskId,
    pub(crate) title: String,
    pub(crate) description_removed: bool,
}

impl RenderText for TaskRemoveOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        writeln!(w, "Removed task {}: {}", self.id, self.title)?;
        if self.description_removed {
            writeln!(w, "Removed description file")?;
        }
        Ok(())
    }
}

pub(crate) fn cmd_task_remove(id: &str) -> Result<TaskRemoveOutput> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)?;
    let task = project.remove_task(&task_id)?;
    save_project(&root, &project)?;
    let description_removed =
        file_store::remove_task_description(&root, &task_id)?;
    Ok(TaskRemoveOutput {
        id: task_id,
        title: task.title,
        description_removed,
    })
}

/// `task rename` output.
#[derive(Serialize)]
pub(crate) struct TaskRenameOutput {
    pub(crate) old_id: TaskId,
    pub(crate) new_id: TaskId,
}

impl RenderText for TaskRenameOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        writeln!(w, "{}: renamed to {}", self.old_id, self.new_id)
    }
}

pub(crate) fn cmd_task_rename(
    old_id: &str,
    new_id: &str,
) -> Result<TaskRenameOutput> {
    let (root, mut project) = load_project()?;
    let from = TaskId::new(old_id)?;
    let to = TaskId::new(new_id)?;
    if from != to {
        let new_path = file_store::task_description_path(&root, &to);
        if new_path.exists() {
            anyhow::bail!(
                "destination description file already \
                 exists: {}",
                new_path.display()
            );
        }
    }
    project.rename_task(&from, &to)?;
    save_project(&root, &project)?;
    file_store::rename_task_description(&root, &from, &to)?;
    Ok(TaskRenameOutput {
        old_id: from,
        new_id: to,
    })
}

/// `task update` output.
#[derive(Serialize)]
pub(crate) struct TaskUpdateOutput(pub(crate) TaskRef);

impl RenderText for TaskUpdateOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        writeln!(w, "Updated {}: {}", self.0.id, self.0.title)
    }
}

pub(crate) fn cmd_task_update(
    id: &str,
    title: Option<&str>,
    desc: Option<&str>,
    tags: Option<&str>,
) -> Result<TaskUpdateOutput> {
    if title.is_none() && desc.is_none() && tags.is_none() {
        anyhow::bail!(
            "task update requires at least one of \
             --title, --desc, or --tags"
        );
    }
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)?;
    let description = desc.map(|d| if d.is_empty() { None } else { Some(d) });
    project.update_task(&task_id, title, description)?;
    if let Some(t) = tags {
        let tag_list = parse_tags(t);
        project.set_task_tags(&task_id, &tag_list)?;
    }
    save_project(&root, &project)?;
    let title = project.tasks[&task_id].title.clone();
    Ok(TaskUpdateOutput(TaskRef { id: task_id, title }))
}

/// `task status` output.
#[derive(Serialize)]
pub(crate) struct TaskStatusOutput {
    pub(crate) id: TaskId,
    pub(crate) status: Status,
}

impl RenderText for TaskStatusOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        writeln!(w, "{}: {}", self.id, self.status)
    }
}

pub(crate) fn cmd_task_status(
    id: &str,
    status: &str,
    force: bool,
) -> Result<TaskStatusOutput> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)?;
    let new_status = parse_status(status)?;
    project.set_status(&task_id, new_status, force)?;
    save_project(&root, &project)?;
    Ok(TaskStatusOutput {
        id: task_id,
        status: new_status,
    })
}

/// One task in the `task list` output.
#[derive(Serialize)]
pub(crate) struct TaskListItem {
    pub(crate) id: TaskId,
    pub(crate) title: String,
    pub(crate) status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) complexity: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) assignee: Option<String>,
    pub(crate) critical: bool,
    pub(crate) tags: Vec<Tag>,
}

/// `task list` output.
#[derive(Serialize)]
pub(crate) struct TaskListOutput {
    pub(crate) tasks: Vec<TaskListItem>,
    #[serde(skip_serializing)]
    empty_project: bool,
    #[serde(skip_serializing)]
    has_filters: bool,
    #[serde(skip_serializing)]
    show_status: bool,
}

impl RenderText for TaskListOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        if self.empty_project {
            return writeln!(w, "No tasks.");
        }
        if self.tasks.is_empty() {
            if self.has_filters {
                return writeln!(w, "No matching tasks.");
            }
            return writeln!(w, "No tasks.");
        }
        let id_width = self
            .tasks
            .iter()
            .map(|t| t.id.as_str().len())
            .max()
            .unwrap_or(8)
            .max(8);
        for task in &self.tasks {
            let complexity = task
                .complexity
                .map_or(String::new(), |c| format!(" [{c}]"));
            let marker = if task.critical { "*" } else { " " };
            if self.show_status {
                writeln!(
                    w,
                    " {marker}{:<iw$} {:<14} {}{complexity}",
                    task.id.as_str(),
                    task.status,
                    task.title,
                    iw = id_width,
                )?;
            } else {
                writeln!(
                    w,
                    " {marker}{:<iw$} {}{complexity}",
                    task.id.as_str(),
                    task.title,
                    iw = id_width,
                )?;
            }
        }
        Ok(())
    }
}

struct TaskListFilters<'a> {
    available_only: bool,
    active_only: bool,
    status: Option<Status>,
    assignee: Option<&'a str>,
    chain: Option<TaskId>,
    tag: Option<Tag>,
}

impl TaskListFilters<'_> {
    fn any(&self) -> bool {
        self.available_only
            || self.active_only
            || self.status.is_some()
            || self.assignee.is_some()
            || self.chain.is_some()
            || self.tag.is_some()
    }
}

fn filter_task_ids<'a>(
    project: &'a rustwerk::domain::project::Project,
    f: &'a TaskListFilters<'_>,
) -> Result<HashSet<&'a TaskId>> {
    let mut ids: HashSet<&TaskId> = if f.available_only {
        project.available_tasks().into_iter().collect()
    } else if f.active_only {
        project.active_tasks().into_iter().collect()
    } else {
        project.tasks.keys().collect()
    };
    if let Some(s) = f.status {
        let by_status: HashSet<&TaskId> =
            project.tasks_by_status(s).into_iter().collect();
        ids = ids.intersection(&by_status).copied().collect();
    }
    if let Some(assignee) = f.assignee {
        let normalized = assignee.to_lowercase();
        let by_assignee: HashSet<&TaskId> =
            project.tasks_by_assignee(&normalized).into_iter().collect();
        ids = ids.intersection(&by_assignee).copied().collect();
    }
    if let Some(ref tid) = f.chain {
        let chain: HashSet<&TaskId> =
            project.dependency_chain(tid)?.into_iter().collect();
        ids = ids.intersection(&chain).copied().collect();
    }
    if let Some(ref tag) = f.tag {
        ids.retain(|id| {
            project.tasks.get(*id).is_some_and(|t| t.tags.contains(tag))
        });
    }
    Ok(ids)
}

// NOTE: task titles are copied into the DTO verbatim.
// A crafted project.json could contain ANSI escape
// sequences that affect terminal rendering. Sanitization
// should be added before this is used in untrusted
// environments.
pub(crate) fn cmd_task_list(
    available_only: bool,
    active_only: bool,
    status_filter: Option<&str>,
    assignee_filter: Option<&str>,
    chain_filter: Option<&str>,
    tag_filter: Option<&str>,
) -> Result<TaskListOutput> {
    let (_root, project) = load_project()?;
    let empty_project = project.tasks.is_empty();
    if empty_project {
        return Ok(TaskListOutput {
            tasks: Vec::new(),
            empty_project: true,
            has_filters: false,
            show_status: true,
        });
    }

    let crit = project.remaining_critical_path_set();
    let filters = TaskListFilters {
        available_only,
        active_only,
        status: status_filter.map(parse_status).transpose()?,
        assignee: assignee_filter,
        chain: chain_filter.map(TaskId::new).transpose()?,
        tag: tag_filter.map(Tag::new).transpose()?,
    };
    let show_status =
        !available_only && !active_only && filters.status.is_none();
    let ids = filter_task_ids(&project, &filters)?;

    let items: Vec<TaskListItem> = project
        .tasks
        .iter()
        .filter(|(id, _)| ids.contains(id))
        .map(|(id, task)| TaskListItem {
            id: id.clone(),
            title: task.title.clone(),
            status: task.status,
            complexity: task.complexity,
            assignee: task.assignee.clone(),
            critical: crit.contains(id),
            tags: task.tags.clone(),
        })
        .collect();

    Ok(TaskListOutput {
        tasks: items,
        empty_project: false,
        has_filters: filters.any(),
        show_status,
    })
}

/// `task depend` / `task undepend` output.
#[derive(Serialize)]
pub(crate) struct TaskDependOutput {
    pub(crate) from: TaskId,
    pub(crate) to: TaskId,
    #[serde(skip_serializing)]
    pub(crate) added: bool,
}

impl RenderText for TaskDependOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        if self.added {
            writeln!(w, "{} depends on {}", self.from, self.to)
        } else {
            writeln!(w, "Removed: {} depends on {}", self.from, self.to)
        }
    }
}

fn modify_dependency(from: &str, to: &str, add: bool) -> Result<TaskDependOutput> {
    let (root, mut project) = load_project()?;
    let from_id = TaskId::new(from)?;
    let to_id = TaskId::new(to)?;
    if add {
        project.add_dependency(&from_id, &to_id)?;
    } else {
        project.remove_dependency(&from_id, &to_id)?;
    }
    save_project(&root, &project)?;
    Ok(TaskDependOutput {
        from: from_id,
        to: to_id,
        added: add,
    })
}

pub(crate) fn cmd_depend(from: &str, to: &str) -> Result<TaskDependOutput> {
    modify_dependency(from, to, true)
}

pub(crate) fn cmd_undepend(from: &str, to: &str) -> Result<TaskDependOutput> {
    modify_dependency(from, to, false)
}

/// `task describe` output. `content` is `None` when no
/// description file exists; callers can disambiguate via
/// the explicit `exists` flag.
#[derive(Serialize)]
pub(crate) struct TaskDescribeOutput {
    pub(crate) id: TaskId,
    /// Path to the description file, relative to the
    /// project root.
    pub(crate) path: String,
    pub(crate) exists: bool,
    pub(crate) content: Option<String>,
}

impl RenderText for TaskDescribeOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        if let Some(content) = &self.content {
            write!(w, "{content}")
        } else {
            writeln!(w, "No description file for {}", self.id)?;
            writeln!(w, "Create one at: {}", self.path)
        }
    }
}

pub(crate) fn cmd_task_describe(id: &str) -> Result<TaskDescribeOutput> {
    use rustwerk::domain::error::DomainError;

    let (root, project) = load_project()?;
    let task_id = TaskId::new(id)?;
    if !project.tasks.contains_key(&task_id) {
        return Err(DomainError::TaskNotFound(task_id.to_string()).into());
    }
    let abs_path = file_store::task_description_path(&root, &task_id);
    let rel_path = abs_path
        .strip_prefix(&root)
        .unwrap_or(&abs_path)
        .display()
        .to_string();

    match std::fs::File::open(&abs_path) {
        Ok(file) => {
            let meta = file.metadata()?;
            if meta.len() > MAX_DESCRIBE_BYTES {
                anyhow::bail!(
                    "description file too large ({} bytes, max {}): {}",
                    meta.len(),
                    MAX_DESCRIBE_BYTES,
                    rel_path
                );
            }
            let mut buf = String::new();
            file.take(MAX_DESCRIBE_BYTES).read_to_string(&mut buf).map_err(
                |e| match e.kind() {
                    io::ErrorKind::InvalidData => anyhow::anyhow!(
                        "description file is not valid UTF-8: {rel_path}"
                    ),
                    _ => anyhow::Error::from(e),
                },
            )?;
            Ok(TaskDescribeOutput {
                id: task_id,
                path: rel_path,
                exists: true,
                content: Some(buf),
            })
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            Ok(TaskDescribeOutput {
                id: task_id,
                path: rel_path,
                exists: false,
                content: None,
            })
        }
        Err(e) => Err(e.into()),
    }
}

use std::collections::HashSet;

use anyhow::Result;

use rustwerk::domain::developer::DeveloperId;
use rustwerk::domain::task::{Effort, Tag, Task, TaskId};
use rustwerk::persistence::file_store;

use crate::{load_project, parse_status, save_project};

/// Parse a comma-separated tags string into a vec of
/// tag slices. An empty string returns an empty vec.
fn parse_tags(tags_str: &str) -> Vec<&str> {
    if tags_str.is_empty() {
        Vec::new()
    } else {
        tags_str.split(',').map(str::trim).collect()
    }
}

pub(crate) fn cmd_task_add(
    title: &str,
    id: Option<&str>,
    desc: Option<&str>,
    complexity: Option<u32>,
    effort: Option<&str>,
    tags: Option<&str>,
) -> Result<()> {
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
    println!("Created task {task_id}");
    Ok(())
}

pub(crate) fn cmd_task_assign(id: &str, to: &str) -> Result<()> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)?;
    let dev_id = DeveloperId::new(to)?;
    project.assign(&task_id, &dev_id)?;
    save_project(&root, &project)?;
    println!("{task_id}: assigned to {dev_id}");
    Ok(())
}

pub(crate) fn cmd_task_unassign(id: &str) -> Result<()> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)?;
    project.unassign(&task_id)?;
    save_project(&root, &project)?;
    println!("{task_id}: unassigned");
    Ok(())
}

pub(crate) fn cmd_task_remove(id: &str) -> Result<()> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)?;
    let task = project.remove_task(&task_id)?;
    save_project(&root, &project)?;
    let removed_desc = file_store::remove_task_description(&root, &task_id)?;
    println!("Removed task {task_id}: {}", task.title);
    if removed_desc {
        println!("Removed description file");
    }
    Ok(())
}

pub(crate) fn cmd_task_rename(old_id: &str, new_id: &str) -> Result<()> {
    let (root, mut project) = load_project()?;
    let from = TaskId::new(old_id)?;
    let to = TaskId::new(new_id)?;

    // Preflight: refuse if the destination description
    // file already exists, so we never reach a state
    // where project.json says the task is renamed but a
    // stale .md file would be overwritten.
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
    println!("{from}: renamed to {to}");
    Ok(())
}

pub(crate) fn cmd_task_update(
    id: &str,
    title: Option<&str>,
    desc: Option<&str>,
    tags: Option<&str>,
) -> Result<()> {
    if title.is_none() && desc.is_none() && tags.is_none() {
        anyhow::bail!(
            "task update requires at least one of \
             --title, --desc, or --tags"
        );
    }
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)?;
    // Empty string for desc means clear it.
    let description = desc.map(|d| if d.is_empty() { None } else { Some(d) });
    project.update_task(&task_id, title, description)?;
    if let Some(t) = tags {
        let tag_list = parse_tags(t);
        project.set_task_tags(&task_id, &tag_list)?;
    }
    save_project(&root, &project)?;
    let task = &project.tasks[&task_id];
    println!("Updated {task_id}: {}", task.title);
    Ok(())
}

pub(crate) fn cmd_task_status(
    id: &str,
    status: &str,
    force: bool,
) -> Result<()> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)?;
    let new_status = parse_status(status)?;
    project.set_status(&task_id, new_status, force)?;
    save_project(&root, &project)?;
    println!("{task_id}: {new_status}");
    Ok(())
}

// NOTE: task titles are printed verbatim. A crafted
// project.json could contain ANSI escape sequences that
// affect terminal rendering. Sanitization should be added
// before this is used in untrusted environments.
pub(crate) fn cmd_task_list(
    available_only: bool,
    active_only: bool,
    status_filter: Option<&str>,
    assignee_filter: Option<&str>,
    chain_filter: Option<&str>,
    tag_filter: Option<&str>,
) -> Result<()> {
    let (_root, project) = load_project()?;
    if project.tasks.is_empty() {
        println!("No tasks.");
        return Ok(());
    }

    let crit = project.remaining_critical_path_set();

    // Parse filters early so we fail fast.
    let status = status_filter.map(parse_status).transpose()?;
    let chain_tid = chain_filter.map(TaskId::new).transpose()?;
    let tag = tag_filter.map(Tag::new).transpose()?;

    // Build the base set of task IDs to display.
    let has_filters = available_only
        || active_only
        || status.is_some()
        || assignee_filter.is_some()
        || chain_tid.is_some()
        || tag.is_some();

    // Collect all task IDs, then narrow down.
    let all_ids: Vec<&TaskId> = project.tasks.keys().collect();

    // Start with base set.
    let mut ids: HashSet<&TaskId> = if available_only {
        project.available_tasks().into_iter().collect()
    } else if active_only {
        project.active_tasks().into_iter().collect()
    } else {
        all_ids.iter().copied().collect()
    };

    // Apply status filter.
    if let Some(s) = status {
        let by_status: HashSet<&TaskId> =
            project.tasks_by_status(s).into_iter().collect();
        ids = ids.intersection(&by_status).copied().collect();
    }

    // Apply assignee filter (lowercase to match DeveloperId
    // normalization).
    if let Some(assignee) = assignee_filter {
        let normalized = assignee.to_lowercase();
        let by_assignee: HashSet<&TaskId> =
            project.tasks_by_assignee(&normalized).into_iter().collect();
        ids = ids.intersection(&by_assignee).copied().collect();
    }

    // Apply chain filter.
    if let Some(ref tid) = chain_tid {
        let chain: HashSet<&TaskId> =
            project.dependency_chain(tid)?.into_iter().collect();
        ids = ids.intersection(&chain).copied().collect();
    }

    // Apply tag filter.
    if let Some(ref tag) = tag {
        ids.retain(|id| {
            project.tasks.get(*id).is_some_and(|t| t.tags.contains(tag))
        });
    }

    if ids.is_empty() {
        if has_filters {
            println!("No matching tasks.");
        } else {
            println!("No tasks.");
        }
        return Ok(());
    }

    // Compute column widths from the data.
    let id_width = project
        .tasks
        .keys()
        .map(|id| id.as_str().len())
        .max()
        .unwrap_or(8)
        .max(8);

    // Show status column only when not pre-filtered to a
    // specific subset (available/active/status).
    let show_status = !available_only && !active_only && status.is_none();

    for (id, task) in &project.tasks {
        if !ids.contains(id) {
            continue;
        }
        let complexity =
            task.complexity.map_or(String::new(), |c| format!(" [{c}]"));
        let marker = if crit.contains(id) { "*" } else { " " };
        if show_status {
            println!(
                " {marker}{:<iw$} {:<14} {}{complexity}",
                id.as_str(),
                task.status,
                task.title,
                iw = id_width,
            );
        } else {
            println!(
                " {marker}{:<iw$} {}{complexity}",
                id.as_str(),
                task.title,
                iw = id_width,
            );
        }
    }
    Ok(())
}

/// Shared logic for depend/undepend commands.
fn modify_dependency(from: &str, to: &str, add: bool) -> Result<()> {
    let (root, mut project) = load_project()?;
    let from_id = TaskId::new(from)?;
    let to_id = TaskId::new(to)?;
    if add {
        project.add_dependency(&from_id, &to_id)?;
        save_project(&root, &project)?;
        println!("{from_id} depends on {to_id}");
    } else {
        project.remove_dependency(&from_id, &to_id)?;
        save_project(&root, &project)?;
        println!("Removed: {from_id} depends on {to_id}");
    }
    Ok(())
}

pub(crate) fn cmd_task_describe(id: &str) -> Result<()> {
    use rustwerk::domain::error::DomainError;

    let (root, project) = load_project()?;
    let task_id = TaskId::new(id)?;

    if !project.tasks.contains_key(&task_id) {
        return Err(DomainError::TaskNotFound(task_id.to_string()).into());
    }

    let path = file_store::task_description_path(&root, &task_id);
    match std::fs::read_to_string(&path) {
        Ok(content) => print!("{content}"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!("No description file for {task_id}");
            println!("Create one at: {}", path.display());
        }
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

pub(crate) fn cmd_depend(from: &str, to: &str) -> Result<()> {
    modify_dependency(from, to, true)
}

pub(crate) fn cmd_undepend(from: &str, to: &str) -> Result<()> {
    modify_dependency(from, to, false)
}

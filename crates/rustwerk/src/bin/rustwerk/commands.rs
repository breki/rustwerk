use anyhow::{bail, Context, Result};

use rustwerk::domain::project::Project;
use rustwerk::domain::task::{
    Effort, EffortEntry, Task, TaskId,
};

use crate::{load_project, parse_status, save_project};

pub(super) fn cmd_init(name: &str) -> Result<()> {
    use std::env;
    use rustwerk::persistence::file_store;

    let root = env::current_dir()
        .context("failed to get current directory")?;
    let path = file_store::project_file_path(&root);
    if path.exists() {
        bail!(
            "project already exists: {}",
            path.display()
        );
    }
    let project = Project::new(name)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    file_store::save(&root, &project)
        .context("failed to save project")?;
    println!("Initialized project: {name}");
    println!("  {}", path.display());
    Ok(())
}

pub(super) fn cmd_show() -> Result<()> {
    let (_root, project) = load_project()?;
    println!("Project: {}", project.metadata.name);
    if let Some(desc) = &project.metadata.description {
        println!("  {desc}");
    }

    let s = project.summary();
    println!();
    println!(
        "Tasks:    {} total  ({} done, {} in-progress, \
         {} todo, {} blocked)",
        s.total, s.done, s.in_progress, s.todo, s.blocked
    );
    println!("Complete: {:.0}%", s.pct_complete);
    if s.total_complexity > 0 {
        println!("Complexity: {} total", s.total_complexity);
    }
    if s.total_estimated_hours > 0.0
        || s.total_actual_hours > 0.0
    {
        println!(
            "Effort:   {:.1}H estimated, {:.1}H actual",
            s.total_estimated_hours, s.total_actual_hours
        );
    }
    println!(
        "Created:  {}",
        project
            .metadata
            .created_at
            .format("%Y-%m-%d %H:%M UTC")
    );
    Ok(())
}

pub(super) fn cmd_task_add(
    title: &str,
    id: Option<&str>,
    desc: Option<&str>,
    complexity: Option<u32>,
    effort: Option<&str>,
) -> Result<()> {
    let (root, mut project) = load_project()?;
    let mut task = Task::new(title)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    task.description = desc.map(String::from);
    if let Some(c) = complexity {
        task.set_complexity(c)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    if let Some(e) = effort {
        task.effort_estimate = Some(
            Effort::parse(e)
                .map_err(|e| anyhow::anyhow!("{e}"))?,
        );
    }

    let task_id = if let Some(id_str) = id {
        let tid = TaskId::new(id_str)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        project
            .add_task(tid.clone(), task)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        tid
    } else {
        project.add_task_auto(task)
    };

    save_project(&root, &project)?;
    println!("Created task {task_id}");
    Ok(())
}

pub(super) fn cmd_task_assign(
    id: &str,
    to: &str,
) -> Result<()> {
    use rustwerk::domain::developer::DeveloperId;
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let dev_id = DeveloperId::new(to)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    project
        .assign(&task_id, &dev_id)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    save_project(&root, &project)?;
    println!("{task_id}: assigned to {dev_id}");
    Ok(())
}

pub(super) fn cmd_task_unassign(
    id: &str,
) -> Result<()> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    project
        .unassign(&task_id)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    save_project(&root, &project)?;
    println!("{task_id}: unassigned");
    Ok(())
}

pub(super) fn cmd_effort_log(
    id: &str,
    amount: &str,
    dev: &str,
    note: Option<&str>,
) -> Result<()> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let effort = Effort::parse(amount)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let entry = EffortEntry {
        effort,
        developer: dev.to_string(),
        timestamp: chrono::Utc::now(),
        note: note.map(String::from),
    };
    project
        .log_effort(&task_id, entry)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    save_project(&root, &project)?;
    let task = &project.tasks[&task_id];
    println!(
        "{task_id}: logged {amount} (total: {:.1}H)",
        task.total_actual_effort_hours()
    );
    Ok(())
}

pub(super) fn cmd_effort_estimate(
    id: &str,
    amount: &str,
) -> Result<()> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let effort = Effort::parse(amount)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    project
        .set_effort_estimate(&task_id, effort)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    save_project(&root, &project)?;
    println!("{task_id}: estimate set to {amount}");
    Ok(())
}

pub(super) fn cmd_task_remove(id: &str) -> Result<()> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let task = project
        .remove_task(&task_id)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    save_project(&root, &project)?;
    println!("Removed task {task_id}: {}", task.title);
    Ok(())
}

pub(super) fn cmd_task_update(
    id: &str,
    title: Option<&str>,
    desc: Option<&str>,
) -> Result<()> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    // Empty string for desc means clear it.
    let description = desc.map(|d| {
        if d.is_empty() {
            None
        } else {
            Some(d)
        }
    });
    project
        .update_task(&task_id, title, description)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    save_project(&root, &project)?;
    let task = &project.tasks[&task_id];
    println!("Updated {task_id}: {}", task.title);
    Ok(())
}

pub(super) fn cmd_task_status(
    id: &str,
    status: &str,
    force: bool,
) -> Result<()> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let new_status = parse_status(status)?;
    project
        .set_status(&task_id, new_status, force)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    save_project(&root, &project)?;
    println!("{task_id}: {new_status}");
    Ok(())
}

// NOTE: task titles are printed verbatim. A crafted
// project.json could contain ANSI escape sequences that
// affect terminal rendering. Sanitization should be added
// before this is used in untrusted environments.
pub(super) fn cmd_task_list(
    available_only: bool,
    active_only: bool,
) -> Result<()> {
    let (_root, project) = load_project()?;
    if project.tasks.is_empty() {
        println!("No tasks.");
        return Ok(());
    }

    let crit = project.remaining_critical_path_set();

    // Compute column widths from the data.
    let id_width = project
        .tasks
        .keys()
        .map(|id| id.as_str().len())
        .max()
        .unwrap_or(8)
        .max(8);


    if available_only {
        let avail = project.available_tasks();
        if avail.is_empty() {
            println!("No available tasks.");
            return Ok(());
        }
        for id in &avail {
            let task = &project.tasks[*id];
            let complexity = task
                .complexity
                .map_or(String::new(), |c| {
                    format!(" [{c}]")
                });
            let marker =
                if crit.contains(*id) { "*" } else { " " };
            println!(
                " {marker}{:<iw$} {}{complexity}",
                id.as_str(),
                task.title,
                iw = id_width,
            );
        }
    } else if active_only {
        let active = project.active_tasks();
        if active.is_empty() {
            println!("No active tasks.");
            return Ok(());
        }
        for id in &active {
            let task = &project.tasks[*id];
            let complexity = task
                .complexity
                .map_or(String::new(), |c| {
                    format!(" [{c}]")
                });
            let marker =
                if crit.contains(*id) { "*" } else { " " };
            println!(
                " {marker}{:<iw$} {}{complexity}",
                id.as_str(),
                task.title,
                iw = id_width,
            );
        }
    } else {
        for (id, task) in &project.tasks {
            let complexity = task
                .complexity
                .map_or(String::new(), |c| {
                    format!(" [{c}]")
                });
            let marker =
                if crit.contains(id) { "*" } else { " " };
            println!(
                " {marker}{:<iw$} {:<14} {}{complexity}",
                id.as_str(),
                task.status,
                task.title,
                iw = id_width,
            );
        }
    }
    Ok(())
}

/// Shared logic for depend/undepend commands.
fn modify_dependency(
    from: &str,
    to: &str,
    add: bool,
) -> Result<()> {
    let (root, mut project) = load_project()?;
    let from_id = TaskId::new(from)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let to_id = TaskId::new(to)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    if add {
        project
            .add_dependency(&from_id, &to_id)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        save_project(&root, &project)?;
        println!("{from_id} depends on {to_id}");
    } else {
        project
            .remove_dependency(&from_id, &to_id)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        save_project(&root, &project)?;
        println!("Removed: {from_id} depends on {to_id}");
    }
    Ok(())
}

pub(super) fn cmd_depend(
    from: &str,
    to: &str,
) -> Result<()> {
    modify_dependency(from, to, true)
}

pub(super) fn cmd_undepend(
    from: &str,
    to: &str,
) -> Result<()> {
    modify_dependency(from, to, false)
}

/// List all developers in the project.
pub(super) fn cmd_dev_list() -> Result<()> {
    let (_root, project) = load_project()?;
    if project.developers.is_empty() {
        println!("No developers.");
        return Ok(());
    }
    for (id, dev) in &project.developers {
        let role = dev
            .role
            .as_deref()
            .map_or(String::new(), |r| format!(" ({r})"));
        let email = dev
            .email
            .as_deref()
            .map_or(String::new(), |e| format!(" <{e}>"));
        println!("  {id}  {}{email}{role}", dev.name);
    }
    Ok(())
}

/// PM completion summary report.
pub(super) fn cmd_report_complete() -> Result<()> {
    let (_root, project) = load_project()?;
    let s = project.summary();
    let (crit_path, crit_len) =
        project.remaining_critical_path();

    println!(
        "Completion Report: {}",
        project.metadata.name
    );
    println!("{}", "=".repeat(40));

    // Status breakdown.
    println!();
    println!("Status Breakdown");
    println!("  Done:        {:>3}", s.done);
    println!("  In Progress: {:>3}", s.in_progress);
    println!("  Blocked:     {:>3}", s.blocked);
    println!("  Todo:        {:>3}", s.todo);
    println!("  Total:       {:>3}", s.total);

    // Completion bar.
    println!();
    let bar_width = 30;
    let filled = if s.total > 0 {
        (f64::from(s.done) / f64::from(s.total)
            * bar_width as f64)
            .round() as usize
    } else {
        0
    };
    let empty = bar_width - filled;
    println!(
        "Completion: [{}>{}] {:.0}%",
        "=".repeat(filled),
        " ".repeat(empty),
        s.pct_complete,
    );

    // Effort.
    if s.total_estimated_hours > 0.0
        || s.total_actual_hours > 0.0
    {
        println!();
        println!("Effort");
        println!(
            "  Estimated:   {:.1}H",
            s.total_estimated_hours
        );
        println!(
            "  Actual:      {:.1}H",
            s.total_actual_hours
        );
        if s.total_estimated_hours > 0.0 {
            let pct = s.total_actual_hours
                / s.total_estimated_hours
                * 100.0;
            println!("  Burn rate:   {pct:.0}%");
        }
    }

    // Complexity.
    if s.total_complexity > 0 {
        println!();
        println!(
            "Complexity:    {} total",
            s.total_complexity
        );
    }

    // Critical path.
    println!();
    if crit_path.is_empty() {
        println!("Critical Path: (none remaining)");
    } else {
        println!(
            "Critical Path: {} tasks, {} complexity",
            crit_path.len(),
            crit_len
        );
        print!("  ");
        for (i, id) in crit_path.iter().enumerate() {
            if i > 0 {
                print!(" → ");
            }
            print!("{id}");
        }
        println!();
    }

    Ok(())
}

/// Effort breakdown per developer.
pub(super) fn cmd_report_effort() -> Result<()> {
    use std::collections::BTreeMap;

    let (_root, project) = load_project()?;

    // Aggregate effort by developer across all tasks.
    let mut by_dev: BTreeMap<&str, f64> = BTreeMap::new();
    for task in project.tasks.values() {
        for entry in &task.effort_entries {
            *by_dev
                .entry(&entry.developer)
                .or_insert(0.0) += entry.effort.to_hours();
        }
    }

    if by_dev.is_empty() {
        println!("No effort logged.");
        return Ok(());
    }

    let total: f64 = by_dev.values().sum();
    println!("Effort by Developer");
    println!("{}", "=".repeat(40));
    for (dev, hours) in &by_dev {
        let pct = hours / total * 100.0;
        println!("  {dev:<20} {hours:>7.1}H ({pct:.0}%)");
    }
    println!("{}", "-".repeat(40));
    println!("  {:<20} {:>7.1}H", "Total", total);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustwerk::domain::task::Status;

    // --- parse_status ---

    #[test]
    fn parse_status_all_variants() {
        assert_eq!(
            parse_status("todo").unwrap(),
            Status::Todo
        );
        assert_eq!(
            parse_status("in-progress").unwrap(),
            Status::InProgress
        );
        assert_eq!(
            parse_status("in_progress").unwrap(),
            Status::InProgress
        );
        assert_eq!(
            parse_status("inprogress").unwrap(),
            Status::InProgress
        );
        assert_eq!(
            parse_status("blocked").unwrap(),
            Status::Blocked
        );
        assert_eq!(
            parse_status("done").unwrap(),
            Status::Done
        );
        assert_eq!(
            parse_status("TODO").unwrap(),
            Status::Todo
        );
    }

    #[test]
    fn parse_status_unknown() {
        assert!(parse_status("invalid").is_err());
    }
}

use anyhow::{bail, Context, Result};

use rustwerk::domain::project::Project;

use crate::load_project;

pub(crate) fn cmd_init(name: &str) -> Result<()> {
    use rustwerk::persistence::file_store;
    use std::env;

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

pub(crate) fn cmd_show() -> Result<()> {
    let (_root, project) = load_project()?;
    println!("Project: {}", project.metadata.name);
    if let Some(desc) = &project.metadata.description {
        println!("  {desc}");
    }

    let s = project.summary();
    println!();
    println!(
        "Tasks:    {} total  ({} done, {} in-progress, \
         {} todo, {} blocked, {} on-hold)",
        s.total, s.done, s.in_progress, s.todo, s.blocked,
        s.on_hold
    );
    println!("Complete: {:.0}%", s.pct_complete);
    if s.total_complexity > 0 {
        println!(
            "Complexity: {} total",
            s.total_complexity
        );
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

/// Compact project status dashboard.
pub(crate) fn cmd_status() -> Result<()> {
    let (_root, project) = load_project()?;
    let s = project.summary();

    // Completion bar.
    let bar_width = 20;
    let filled = if s.total > 0 {
        (f64::from(s.done) / f64::from(s.total)
            * bar_width as f64)
            .round() as usize
    } else {
        0
    }
    .min(bar_width);
    let empty = bar_width - filled;
    let bar = format!(
        "[{}{}] {:.0}%",
        "\u{2588}".repeat(filled),
        "\u{2591}".repeat(empty),
        s.pct_complete,
    );

    println!("{}", project.metadata.name);
    println!("{bar}");
    println!();
    println!("  {:<14} {:>3}", "done", s.done);
    println!(
        "  {:<14} {:>3}",
        "in-progress", s.in_progress
    );
    println!("  {:<14} {:>3}", "todo", s.todo);
    println!("  {:<14} {:>3}", "blocked", s.blocked);
    if s.on_hold > 0 {
        println!("  {:<14} {:>3}", "on-hold", s.on_hold);
    }
    println!("  {:<14} {:>3}", "total", s.total);

    // Active tasks.
    let active = project.active_tasks();
    if !active.is_empty() {
        println!();
        println!("Active:");
        for id in &active {
            let assignee = project
                .tasks
                .get(*id)
                .and_then(|t| t.assignee.as_deref())
                .unwrap_or("-");
            println!("  {id}  ({assignee})");
        }
    }

    // Bottleneck count.
    let bottlenecks = project.bottlenecks();
    if !bottlenecks.is_empty() {
        println!();
        println!(
            "{} bottleneck{}",
            bottlenecks.len(),
            if bottlenecks.len() == 1 { "" } else { "s" }
        );
    }

    // Remaining critical path.
    let (crit, crit_len) =
        project.remaining_critical_path();
    if !crit.is_empty() {
        println!(
            "Critical path: {} tasks, {} complexity",
            crit.len(),
            crit_len
        );
    }

    Ok(())
}

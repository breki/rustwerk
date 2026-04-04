use anyhow::Result;

use rustwerk::domain::task::{Effort, EffortEntry, TaskId};

use crate::{load_project, save_project};

pub(crate) fn cmd_effort_log(
    id: &str,
    amount: &str,
    dev: &str,
    note: Option<&str>,
) -> Result<()> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)?;
    let effort = Effort::parse(amount)?;
    let entry = EffortEntry {
        effort,
        developer: dev.to_string(),
        timestamp: chrono::Utc::now(),
        note: note.map(String::from),
    };
    project.log_effort(&task_id, entry)?;
    save_project(&root, &project)?;
    let task = &project.tasks[&task_id];
    println!(
        "{task_id}: logged {amount} (total: {:.1}H)",
        task.total_actual_effort_hours()
    );
    Ok(())
}

pub(crate) fn cmd_effort_estimate(id: &str, amount: &str) -> Result<()> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)?;
    let effort = Effort::parse(amount)?;
    project.set_effort_estimate(&task_id, effort)?;
    save_project(&root, &project)?;
    println!("{task_id}: estimate set to {amount}");
    Ok(())
}

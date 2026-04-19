use std::io::{self, Write};

use anyhow::Result;
use serde::Serialize;

use rustwerk::domain::task::{Effort, EffortEntry, TaskId};

use crate::render::{finite, RenderText};
use crate::{load_project, save_project};

/// `effort log` output.
#[derive(Serialize)]
pub(crate) struct EffortLogOutput {
    pub(crate) id: TaskId,
    pub(crate) logged_hours: Option<f64>,
    pub(crate) total_hours: Option<f64>,
    #[serde(skip_serializing)]
    display_amount: String,
}

impl RenderText for EffortLogOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        writeln!(
            w,
            "{}: logged {} (total: {:.1}H)",
            self.id,
            self.display_amount,
            self.total_hours.unwrap_or(0.0)
        )
    }
}

pub(crate) fn cmd_effort_log(
    id: &str,
    amount: &str,
    dev: &str,
    note: Option<&str>,
) -> Result<EffortLogOutput> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)?;
    let effort = Effort::parse(amount)?;
    let logged_hours = effort.to_hours();
    let entry = EffortEntry {
        effort,
        developer: dev.to_string(),
        timestamp: chrono::Utc::now(),
        note: note.map(String::from),
    };
    project.log_effort(&task_id, entry)?;
    save_project(&root, &project)?;
    let task = &project.tasks[&task_id];
    let total_hours = task.total_actual_effort_hours();
    Ok(EffortLogOutput {
        id: task_id,
        logged_hours: finite(logged_hours),
        total_hours: finite(total_hours),
        display_amount: amount.to_string(),
    })
}

/// `effort estimate` output.
#[derive(Serialize)]
pub(crate) struct EffortEstimateOutput {
    pub(crate) id: TaskId,
    pub(crate) estimate_hours: Option<f64>,
    #[serde(skip_serializing)]
    display_amount: String,
}

impl RenderText for EffortEstimateOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        writeln!(w, "{}: estimate set to {}", self.id, self.display_amount)
    }
}

pub(crate) fn cmd_effort_estimate(
    id: &str,
    amount: &str,
) -> Result<EffortEstimateOutput> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)?;
    let effort = Effort::parse(amount)?;
    let hours = effort.to_hours();
    project.set_effort_estimate(&task_id, effort)?;
    save_project(&root, &project)?;
    Ok(EffortEstimateOutput {
        id: task_id,
        estimate_hours: finite(hours),
        display_amount: amount.to_string(),
    })
}

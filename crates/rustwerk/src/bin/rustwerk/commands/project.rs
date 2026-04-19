use std::io::{self, Write};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;

use rustwerk::domain::project::{Project, ProjectSummary};
use rustwerk::domain::task::TaskId;

use crate::load_project;
use crate::render::{finite, RenderText};

/// `init` command output.
#[derive(Serialize)]
pub(crate) struct InitOutput {
    pub(crate) name: String,
    pub(crate) path: String,
}

impl RenderText for InitOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        writeln!(w, "Initialized project: {}", self.name)?;
        writeln!(w, "  {}", self.path)
    }
}

pub(crate) fn cmd_init(name: &str) -> Result<InitOutput> {
    use rustwerk::persistence::file_store;

    let root =
        std::env::current_dir().context("failed to get current directory")?;
    let path = file_store::project_file_path(&root);
    if path.exists() {
        bail!("project already exists: {}", path.display());
    }
    let project = Project::new(name)?;
    file_store::save(&root, &project).context("failed to save project")?;
    Ok(InitOutput {
        name: project.metadata.name,
        path: path.display().to_string(),
    })
}

/// Common serializable summary body.
#[derive(Serialize)]
pub(crate) struct SummaryJson {
    pub(crate) total: u32,
    pub(crate) todo: u32,
    pub(crate) in_progress: u32,
    pub(crate) blocked: u32,
    pub(crate) done: u32,
    pub(crate) on_hold: u32,
    pub(crate) pct_complete: Option<f64>,
    pub(crate) total_estimated_hours: Option<f64>,
    pub(crate) total_actual_hours: Option<f64>,
    pub(crate) total_complexity: u32,
}

impl From<&ProjectSummary> for SummaryJson {
    fn from(s: &ProjectSummary) -> Self {
        Self {
            total: s.total,
            todo: s.todo,
            in_progress: s.in_progress,
            blocked: s.blocked,
            done: s.done,
            on_hold: s.on_hold,
            pct_complete: finite(s.pct_complete),
            total_estimated_hours: finite(s.total_estimated_hours),
            total_actual_hours: finite(s.total_actual_hours),
            total_complexity: s.total_complexity,
        }
    }
}

/// `show` command output.
#[derive(Serialize)]
pub(crate) struct ShowOutput {
    pub(crate) name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) summary: SummaryJson,
}

impl RenderText for ShowOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        writeln!(w, "Project: {}", self.name)?;
        if let Some(desc) = &self.description {
            writeln!(w, "  {desc}")?;
        }
        writeln!(w)?;
        let s = &self.summary;
        writeln!(
            w,
            "Tasks:    {} total  ({} done, {} in-progress, \
             {} todo, {} blocked, {} on-hold)",
            s.total, s.done, s.in_progress, s.todo, s.blocked, s.on_hold
        )?;
        writeln!(
            w,
            "Complete: {:.0}%",
            s.pct_complete.unwrap_or(0.0)
        )?;
        if s.total_complexity > 0 {
            writeln!(w, "Complexity: {} total", s.total_complexity)?;
        }
        let est = s.total_estimated_hours.unwrap_or(0.0);
        let act = s.total_actual_hours.unwrap_or(0.0);
        if est > 0.0 || act > 0.0 {
            writeln!(w, "Effort:   {est:.1}H estimated, {act:.1}H actual")?;
        }
        writeln!(
            w,
            "Created:  {}",
            self.created_at.format("%Y-%m-%d %H:%M UTC")
        )
    }
}

pub(crate) fn cmd_show() -> Result<ShowOutput> {
    let (_root, project) = load_project()?;
    let s = project.summary();
    Ok(ShowOutput {
        name: project.metadata.name.clone(),
        description: project.metadata.description.clone(),
        created_at: project.metadata.created_at,
        summary: (&s).into(),
    })
}

/// Active-task reference for `status`.
#[derive(Serialize)]
pub(crate) struct StatusActive {
    pub(crate) id: TaskId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) assignee: Option<String>,
}

/// `status` command output.
#[derive(Serialize)]
pub(crate) struct StatusOutput {
    pub(crate) name: String,
    pub(crate) summary: SummaryJson,
    pub(crate) active: Vec<StatusActive>,
    pub(crate) bottleneck_count: usize,
    pub(crate) critical_path: Vec<TaskId>,
    pub(crate) critical_path_complexity: u32,
}

impl RenderText for StatusOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        let s = &self.summary;
        let bar_width: usize = 20;
        let filled = if s.total > 0 {
            (f64::from(s.done) / f64::from(s.total) * bar_width as f64).round()
                as usize
        } else {
            0
        }
        .min(bar_width);
        let empty = bar_width - filled;
        let bar = format!(
            "[{}{}] {:.0}%",
            "\u{2588}".repeat(filled),
            "\u{2591}".repeat(empty),
            s.pct_complete.unwrap_or(0.0),
        );

        writeln!(w, "{}", self.name)?;
        writeln!(w, "{bar}")?;
        writeln!(w)?;
        writeln!(w, "  {:<14} {:>3}", "done", s.done)?;
        writeln!(w, "  {:<14} {:>3}", "in-progress", s.in_progress)?;
        writeln!(w, "  {:<14} {:>3}", "todo", s.todo)?;
        writeln!(w, "  {:<14} {:>3}", "blocked", s.blocked)?;
        if s.on_hold > 0 {
            writeln!(w, "  {:<14} {:>3}", "on-hold", s.on_hold)?;
        }
        writeln!(w, "  {:<14} {:>3}", "total", s.total)?;

        if !self.active.is_empty() {
            writeln!(w)?;
            writeln!(w, "Active:")?;
            for a in &self.active {
                let assignee = a.assignee.as_deref().unwrap_or("-");
                writeln!(w, "  {}  ({assignee})", a.id)?;
            }
        }

        if self.bottleneck_count > 0 {
            writeln!(w)?;
            writeln!(
                w,
                "{} bottleneck{}",
                self.bottleneck_count,
                if self.bottleneck_count == 1 { "" } else { "s" }
            )?;
        }

        if !self.critical_path.is_empty() {
            writeln!(
                w,
                "Critical path: {} tasks, {} complexity",
                self.critical_path.len(),
                self.critical_path_complexity,
            )?;
        }

        Ok(())
    }
}

pub(crate) fn cmd_status() -> Result<StatusOutput> {
    let (_root, project) = load_project()?;
    let s = project.summary();
    let active = project.active_tasks();
    let bottleneck_count = project.bottlenecks().len();
    let (crit, crit_len) = project.remaining_critical_path();
    let active_items = active
        .iter()
        .filter_map(|id| {
            project.tasks.get(*id).map(|t| StatusActive {
                id: (*id).clone(),
                assignee: t.assignee.clone(),
            })
        })
        .collect();
    Ok(StatusOutput {
        name: project.metadata.name.clone(),
        summary: (&s).into(),
        active: active_items,
        bottleneck_count,
        critical_path: crit,
        critical_path_complexity: crit_len,
    })
}

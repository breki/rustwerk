use std::collections::BTreeMap;
use std::io::{self, Write};

use anyhow::Result;
use serde::Serialize;

use rustwerk::domain::task::{Status, TaskId};

use crate::commands::project::SummaryJson;
use crate::load_project;
use crate::render::{finite, RenderText};

/// `report complete` output. Embeds the shared
/// [`SummaryJson`] instead of duplicating its fields.
#[derive(Serialize)]
pub(crate) struct CompleteReportOutput {
    pub(crate) name: String,
    pub(crate) summary: SummaryJson,
    pub(crate) critical_path: Vec<TaskId>,
    pub(crate) critical_path_complexity: u32,
}

impl RenderText for CompleteReportOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        let s = &self.summary;
        writeln!(w, "Completion Report: {}", self.name)?;
        writeln!(w, "{}", "=".repeat(40))?;

        writeln!(w)?;
        writeln!(w, "Status Breakdown")?;
        writeln!(w, "  Done:        {:>3}", s.done)?;
        writeln!(w, "  In Progress: {:>3}", s.in_progress)?;
        writeln!(w, "  Blocked:     {:>3}", s.blocked)?;
        writeln!(w, "  On Hold:     {:>3}", s.on_hold)?;
        writeln!(w, "  Todo:        {:>3}", s.todo)?;
        writeln!(w, "  Total:       {:>3}", s.total)?;

        writeln!(w)?;
        let bar_width = 30usize;
        let filled = if s.total > 0 {
            (f64::from(s.done) / f64::from(s.total) * bar_width as f64).round()
                as usize
        } else {
            0
        };
        let filled = filled.min(bar_width);
        let empty = bar_width - filled;
        writeln!(
            w,
            "Completion: [{}>{}] {:.0}%",
            "=".repeat(filled),
            " ".repeat(empty),
            s.pct_complete.unwrap_or(0.0),
        )?;

        let est = s.total_estimated_hours.unwrap_or(0.0);
        let act = s.total_actual_hours.unwrap_or(0.0);
        if est > 0.0 || act > 0.0 {
            writeln!(w)?;
            writeln!(w, "Effort")?;
            writeln!(w, "  Estimated:   {est:.1}H")?;
            writeln!(w, "  Actual:      {act:.1}H")?;
            if est > 0.0 {
                let pct = act / est * 100.0;
                writeln!(w, "  Burn rate:   {pct:.0}%")?;
            }
        }

        if s.total_complexity > 0 {
            writeln!(w)?;
            writeln!(w, "Complexity:    {} total", s.total_complexity)?;
        }

        writeln!(w)?;
        if self.critical_path.is_empty() {
            writeln!(w, "Critical Path: (none remaining)")?;
        } else {
            writeln!(
                w,
                "Critical Path: {} tasks, {} complexity",
                self.critical_path.len(),
                self.critical_path_complexity
            )?;
            write!(w, "  ")?;
            for (i, id) in self.critical_path.iter().enumerate() {
                if i > 0 {
                    write!(w, " → ")?;
                }
                write!(w, "{id}")?;
            }
            writeln!(w)?;
        }
        Ok(())
    }
}

pub(crate) fn cmd_report_complete() -> Result<CompleteReportOutput> {
    let (_root, project) = load_project()?;
    let s = project.summary();
    let (crit_path, crit_len) = project.remaining_critical_path();
    Ok(CompleteReportOutput {
        name: project.metadata.name.clone(),
        summary: (&s).into(),
        critical_path: crit_path,
        critical_path_complexity: crit_len,
    })
}

/// One developer row in the effort report.
#[derive(Serialize)]
pub(crate) struct EffortByDev {
    pub(crate) developer: String,
    pub(crate) hours: Option<f64>,
}

/// `report effort` output.
#[derive(Serialize)]
pub(crate) struct EffortReportOutput {
    pub(crate) developers: Vec<EffortByDev>,
    pub(crate) total_hours: Option<f64>,
}

impl RenderText for EffortReportOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        if self.developers.is_empty() {
            return writeln!(w, "No effort logged.");
        }
        let total = self.total_hours.unwrap_or(0.0);
        writeln!(w, "Effort by Developer")?;
        writeln!(w, "{}", "=".repeat(40))?;
        for entry in &self.developers {
            let hours = entry.hours.unwrap_or(0.0);
            let pct = if total > 0.0 { hours / total * 100.0 } else { 0.0 };
            writeln!(
                w,
                "  {:<20} {hours:>7.1}H ({pct:.0}%)",
                entry.developer
            )?;
        }
        writeln!(w, "{}", "-".repeat(40))?;
        writeln!(w, "  {:<20} {total:>7.1}H", "Total")
    }
}

pub(crate) fn cmd_report_effort() -> Result<EffortReportOutput> {
    let (_root, project) = load_project()?;
    let mut by_dev: BTreeMap<String, f64> = BTreeMap::new();
    for task in project.tasks.values() {
        for entry in &task.effort_entries {
            *by_dev.entry(entry.developer.clone()).or_insert(0.0) +=
                entry.effort.to_hours();
        }
    }
    let total: f64 = by_dev.values().sum();
    let developers = by_dev
        .into_iter()
        .map(|(developer, hours)| EffortByDev {
            developer,
            hours: finite(hours),
        })
        .collect();
    Ok(EffortReportOutput {
        developers,
        total_hours: finite(total),
    })
}

/// One bottleneck row.
#[derive(Serialize)]
pub(crate) struct BottleneckItem {
    pub(crate) id: TaskId,
    pub(crate) downstream_count: usize,
    pub(crate) status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) assignee: Option<String>,
    pub(crate) ready: bool,
}

/// `report bottlenecks` output.
#[derive(Serialize)]
pub(crate) struct BottleneckReportOutput {
    pub(crate) bottlenecks: Vec<BottleneckItem>,
}

impl RenderText for BottleneckReportOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        if self.bottlenecks.is_empty() {
            return writeln!(w, "No bottlenecks detected.");
        }

        let iw = self
            .bottlenecks
            .iter()
            .map(|bn| bn.id.as_str().len())
            .max()
            .unwrap_or(2)
            .max(2);

        writeln!(w, "Bottleneck Report")?;
        writeln!(
            w,
            "  {:<iw$}  {:>6}  {:<13} Assignee",
            "ID", "Blocks", "State"
        )?;
        writeln!(w, "{}", "-".repeat(iw + 36))?;

        for bn in &self.bottlenecks {
            let assignee = bn.assignee.as_deref().unwrap_or("-");
            let state = if bn.status == Status::InProgress {
                "in progress"
            } else if bn.status == Status::OnHold {
                "on hold"
            } else if bn.ready {
                "ready"
            } else {
                "blocked"
            };
            writeln!(
                w,
                "  {:<iw$}  {:>6}  {:<13} {}",
                bn.id, bn.downstream_count, state, assignee
            )?;
        }
        Ok(())
    }
}

pub(crate) fn cmd_report_bottlenecks() -> Result<BottleneckReportOutput> {
    let (_root, project) = load_project()?;
    let bottlenecks = project
        .bottlenecks()
        .into_iter()
        .map(|bn| BottleneckItem {
            id: bn.id,
            downstream_count: bn.downstream_count,
            status: bn.status,
            assignee: bn.assignee,
            ready: bn.ready,
        })
        .collect();
    Ok(BottleneckReportOutput { bottlenecks })
}

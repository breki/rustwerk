use std::collections::BTreeMap;

use anyhow::Result;

use crate::load_project;

/// PM completion summary report.
pub(crate) fn cmd_report_complete() -> Result<()> {
    let (_root, project) = load_project()?;
    let s = project.summary();
    let (crit_path, crit_len) = project.remaining_critical_path();

    println!("Completion Report: {}", project.metadata.name);
    println!("{}", "=".repeat(40));

    // Status breakdown.
    println!();
    println!("Status Breakdown");
    println!("  Done:        {:>3}", s.done);
    println!("  In Progress: {:>3}", s.in_progress);
    println!("  Blocked:     {:>3}", s.blocked);
    println!("  On Hold:     {:>3}", s.on_hold);
    println!("  Todo:        {:>3}", s.todo);
    println!("  Total:       {:>3}", s.total);

    // Completion bar.
    println!();
    let bar_width = 30;
    let filled = if s.total > 0 {
        (f64::from(s.done) / f64::from(s.total) * bar_width as f64).round()
            as usize
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
    if s.total_estimated_hours > 0.0 || s.total_actual_hours > 0.0 {
        println!();
        println!("Effort");
        println!("  Estimated:   {:.1}H", s.total_estimated_hours);
        println!("  Actual:      {:.1}H", s.total_actual_hours);
        if s.total_estimated_hours > 0.0 {
            let pct = s.total_actual_hours / s.total_estimated_hours * 100.0;
            println!("  Burn rate:   {pct:.0}%");
        }
    }

    // Complexity.
    if s.total_complexity > 0 {
        println!();
        println!("Complexity:    {} total", s.total_complexity);
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
pub(crate) fn cmd_report_effort() -> Result<()> {
    let (_root, project) = load_project()?;

    // Aggregate effort by developer across all tasks.
    let mut by_dev: BTreeMap<&str, f64> = BTreeMap::new();
    for task in project.tasks.values() {
        for entry in &task.effort_entries {
            *by_dev.entry(&entry.developer).or_insert(0.0) +=
                entry.effort.to_hours();
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

/// PM bottleneck report — tasks blocking the most
/// downstream work, enriched with assignee and status.
pub(crate) fn cmd_report_bottlenecks() -> Result<()> {
    let (_root, project) = load_project()?;
    let bottlenecks = project.bottlenecks();

    if bottlenecks.is_empty() {
        println!("No bottlenecks detected.");
        return Ok(());
    }

    let iw = bottlenecks
        .iter()
        .map(|bn| bn.id.as_str().len())
        .max()
        .unwrap_or(2)
        .max(2);

    println!("Bottleneck Report");
    println!("  {:<iw$}  {:>6}  {:<13} Assignee", "ID", "Blocks", "State",);
    println!("{}", "-".repeat(iw + 36));

    for bn in &bottlenecks {
        let assignee = bn.assignee.as_deref().unwrap_or("-");
        let state = if bn.status == rustwerk::domain::task::Status::InProgress {
            "in progress"
        } else if bn.status == rustwerk::domain::task::Status::OnHold {
            "on hold"
        } else if bn.ready {
            "ready"
        } else {
            "blocked"
        };
        println!(
            "  {:<iw$}  {:>6}  {:<13} {}",
            bn.id, bn.downstream_count, state, assignee,
        );
    }

    Ok(())
}

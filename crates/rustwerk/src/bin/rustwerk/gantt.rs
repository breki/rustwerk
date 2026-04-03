use std::env;
use std::io::IsTerminal;

use anyhow::Result;

use rustwerk::domain::project::GanttRow;
use rustwerk::domain::task::Status;

use crate::load_project;

/// Default terminal width when detection fails.
const FALLBACK_WIDTH: usize = 80;

/// Get terminal width. Uses `terminal_size` crate,
/// falls back to 80.
fn term_width() -> usize {
    terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(FALLBACK_WIDTH)
}

/// Scale a value by a factor, with minimum 1 (for bar
/// widths that must be visible).
fn scale_min1(value: u32, factor: f64) -> usize {
    (f64::from(value) * factor).round().max(1.0) as usize
}

/// Scale a value by a factor (no minimum — used for
/// positions where 0 is valid).
fn scale_pos(value: u32, factor: f64) -> usize {
    (f64::from(value) * factor).round() as usize
}

/// Check whether color output is enabled.
/// Colors are on if stdout is a terminal, unless
/// `NO_COLOR` env var is set.
fn use_color() -> bool {
    std::io::stdout().is_terminal()
        && env::var_os("NO_COLOR").is_none()
}

/// ANSI color codes.
mod ansi {
    pub(super) const RESET: &str = "\x1b[0m";
    pub(super) const BOLD: &str = "\x1b[1m";
    pub(super) const DIM: &str = "\x1b[2m";
    pub(super) const GREEN: &str = "\x1b[32m";
    pub(super) const YELLOW: &str = "\x1b[33m";
    pub(super) const RED: &str = "\x1b[31m";
}

/// Select bar and ID ANSI styles based on task status
/// and whether the task is on the critical path.
/// Critical path tasks render the entire line in red.
/// Returns `(bar_color, id_style)`.
fn bar_style(
    status: Status,
    critical: bool,
) -> (&'static str, &'static str) {
    if critical {
        (ansi::RED, ansi::RED)
    } else {
        match status {
            Status::Done => (ansi::GREEN, ""),
            Status::InProgress => {
                (ansi::YELLOW, ansi::BOLD)
            }
            Status::Blocked => (ansi::RED, ansi::RED),
            Status::Todo => (ansi::DIM, ""),
        }
    }
}

/// Entry point for the `gantt` command.
pub(super) fn cmd_gantt() -> Result<()> {
    let (_root, project) = load_project()?;
    let rows = project.gantt_schedule();
    let width = term_width();
    render_gantt(&rows, width, use_color());
    Ok(())
}

/// Render a Gantt chart to stdout. Separated from
/// `cmd_gantt` for testability.
fn render_gantt(
    rows: &[GanttRow],
    terminal_width: usize,
    color: bool,
) {
    if rows.is_empty() {
        println!("No tasks.");
        return;
    }

    let max_end = rows
        .iter()
        .map(|r| r.end())
        .max()
        .unwrap_or(0);

    // Find the longest ID for padding.
    let id_width = rows
        .iter()
        .map(|r| r.id.as_str().len())
        .max()
        .unwrap_or(8)
        .max(8);

    // Compute scale factor for terminal width.
    let label_width = id_width + 2; // marker + id + space
    let tw = terminal_width;
    let bar_area = tw
        .saturating_sub(label_width)
        .saturating_sub(1); // trailing newline margin
    let scale_factor = if max_end == 0 {
        1.0
    } else {
        bar_area as f64 / f64::from(max_end)
    };

    let scaled_max = scale_pos(max_end, scale_factor);

    // Header with scale.
    let dim = if color { ansi::DIM } else { "" };
    let rst = if color { ansi::RESET } else { "" };

    // Tick interval: every 5 unscaled units, but widen
    // if they'd overlap when scaled.
    let tick_interval = if scale_factor < 0.5 {
        10
    } else {
        5
    };

    print!("{dim}{:width$}", "", width = label_width);
    for i in (0..max_end).step_by(tick_interval as usize) {
        let col = scale_pos(i, scale_factor);
        let next_col = scale_pos(
            (i + tick_interval).min(max_end),
            scale_factor,
        );
        let gap = next_col.saturating_sub(col);
        if gap > 0 {
            print!("{i:<gap$}");
        }
    }
    println!("{rst}");
    print!("{dim}{:width$}", "", width = label_width);
    for i in 0..scaled_max {
        // Find if this column corresponds to a tick.
        let is_tick = (0..max_end)
            .step_by(tick_interval as usize)
            .any(|t| scale_pos(t, scale_factor) == i);
        if is_tick {
            print!("|");
        } else {
            print!(" ");
        }
    }
    println!("{rst}");

    // Rows — bar rendering uses domain methods.
    for row in rows {
        let marker =
            if row.critical { "*" } else { " " };
        let (filled, empty) = row.bar_fill();
        let fill_ch = row.fill_char();
        let empty_ch = row.empty_char();

        // Scale positions. The total bar width includes
        // left and right caps (2 chars), so the body is
        // the remainder.
        let s_start = scale_pos(row.start, scale_factor);
        let s_total =
            scale_min1(row.width, scale_factor).max(2);
        let s_body = s_total - 2; // room for caps
        let (s_filled, s_empty) = if s_body == 0 {
            (0, 0)
        } else if filled == 0 {
            (0, s_body)
        } else if empty == 0 {
            (s_body, 0)
        } else {
            let sf = (f64::from(filled)
                / f64::from(filled + empty)
                * s_body as f64)
                .round() as usize;
            let sf = sf.clamp(1, s_body - 1);
            (sf, s_body - sf)
        };

        // Color the bar based on status and critical path.
        let (bar_color, id_style) = if color {
            bar_style(row.status, row.critical)
        } else {
            ("", "")
        };

        let crit_style = if color && row.critical {
            ansi::RED
        } else {
            ""
        };

        let filled_str: String =
            std::iter::repeat_n(fill_ch, s_filled)
                .collect();
        let empty_str: String =
            std::iter::repeat_n(empty_ch, s_empty)
                .collect();
        let left_cap = GanttRow::LEFT_CAP;
        let right_cap = GanttRow::RIGHT_CAP;

        let padding = " ".repeat(s_start);
        print!(
            "{crit_style}{marker}{rst}\
             {id_style}{:<width$}{rst} \
             {padding}{bar_color}\
             {left_cap}{filled_str}{empty_str}{right_cap}\
             {rst}",
            row.id,
            width = id_width,
        );
        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn critical_bar_is_red() {
        let (bar, id) = bar_style(Status::Done, true);
        assert_eq!(bar, ansi::RED);
        assert_eq!(id, ansi::RED);
    }

    #[test]
    fn critical_todo_bar_is_red() {
        let (bar, id) = bar_style(Status::Todo, true);
        assert_eq!(bar, ansi::RED);
        assert_eq!(id, ansi::RED);
    }

    #[test]
    fn non_critical_done_uses_green() {
        let (bar, _) = bar_style(Status::Done, false);
        assert_eq!(bar, ansi::GREEN);
    }

    #[test]
    fn non_critical_todo_uses_dim() {
        let (bar, _) = bar_style(Status::Todo, false);
        assert_eq!(bar, ansi::DIM);
    }
}

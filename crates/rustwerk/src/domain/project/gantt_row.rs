use crate::domain::task::{Status, TaskId};

/// A row in the Gantt chart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GanttRow {
    /// Task ID.
    pub id: TaskId,
    /// Start column (complexity units from time 0).
    pub start: u32,
    /// Bar width (complexity).
    pub width: u32,
    /// Task status.
    pub status: Status,
    /// Whether this task is on the critical path.
    pub critical: bool,
}

impl GanttRow {
    /// End column (start + width).
    pub fn end(&self) -> u32 {
        self.start + self.width
    }

    /// Bar fill characters: (`filled_count`, `empty_count`).
    /// Done = all filled, Todo = all empty, `InProgress` =
    /// half-filled, Blocked = all filled with `!`.
    pub fn bar_fill(&self) -> (u32, u32) {
        match self.status {
            Status::Done | Status::Blocked => (self.width, 0),
            Status::InProgress if self.width > 1 => {
                let done = self.width / 2;
                (done, self.width - done)
            }
            Status::InProgress
            | Status::Todo
            | Status::OnHold => (0, self.width),
        }
    }

    /// Left cap character for the bar.
    pub const LEFT_CAP: char = '\u{2590}'; // ▐

    /// Right cap character for the bar.
    pub const RIGHT_CAP: char = '\u{258C}'; // ▌

    /// Fill character for the completed portion of the bar.
    pub fn fill_char(&self) -> char {
        match self.status {
            Status::Done => '\u{2588}',       // █
            Status::Blocked => '\u{2592}',    // ▒
            Status::InProgress => '\u{2593}', // ▓
            // bar_fill() returns filled=0 for Todo/OnHold,
            // so this is only reached if bar_fill changes.
            Status::Todo | Status::OnHold => '\u{2591}', // ░
        }
    }

    /// Fill character for the remaining portion of the bar.
    pub fn empty_char(&self) -> char {
        '\u{2591}' // ░
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(status: Status, width: u32) -> GanttRow {
        GanttRow {
            id: TaskId::new("T").unwrap(),
            start: 0,
            width,
            status,
            critical: false,
        }
    }

    #[test]
    fn end_returns_start_plus_width() {
        let r = GanttRow {
            id: TaskId::new("T").unwrap(),
            start: 5,
            width: 10,
            status: Status::Todo,
            critical: false,
        };
        assert_eq!(r.end(), 15);
    }

    #[test]
    fn bar_fill_done_all_filled() {
        assert_eq!(row(Status::Done, 10).bar_fill(), (10, 0));
    }

    #[test]
    fn bar_fill_blocked_all_filled() {
        assert_eq!(row(Status::Blocked, 8).bar_fill(), (8, 0));
    }

    #[test]
    fn bar_fill_in_progress_half() {
        assert_eq!(row(Status::InProgress, 10).bar_fill(), (5, 5));
    }

    #[test]
    fn bar_fill_in_progress_odd_width() {
        assert_eq!(row(Status::InProgress, 11).bar_fill(), (5, 6));
    }

    #[test]
    fn bar_fill_in_progress_width_1() {
        assert_eq!(row(Status::InProgress, 1).bar_fill(), (0, 1));
    }

    #[test]
    fn bar_fill_todo_all_empty() {
        assert_eq!(row(Status::Todo, 10).bar_fill(), (0, 10));
    }

    #[test]
    fn fill_char_done() {
        assert_eq!(row(Status::Done, 1).fill_char(), '\u{2588}');
    }

    #[test]
    fn fill_char_blocked() {
        assert_eq!(row(Status::Blocked, 1).fill_char(), '\u{2592}');
    }

    #[test]
    fn fill_char_in_progress() {
        assert_eq!(row(Status::InProgress, 1).fill_char(), '\u{2593}');
    }

    #[test]
    fn fill_char_todo() {
        assert_eq!(row(Status::Todo, 1).fill_char(), '\u{2591}');
    }

    #[test]
    fn empty_char_is_light_shade() {
        assert_eq!(row(Status::Todo, 1).empty_char(), '\u{2591}');
    }
}

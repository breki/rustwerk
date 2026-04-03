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

    /// Bar fill characters: (filled_count, empty_count).
    /// Done = all filled, Todo = all empty, InProgress =
    /// half-filled, Blocked = all filled with `!`.
    pub fn bar_fill(&self) -> (u32, u32) {
        match self.status {
            Status::Done => (self.width, 0),
            Status::Blocked => (self.width, 0),
            Status::InProgress if self.width > 1 => {
                let done = self.width / 2;
                (done, self.width - done)
            }
            Status::InProgress => (0, self.width),
            Status::Todo => (0, self.width),
        }
    }

    /// Left cap character for the bar.
    pub const LEFT_CAP: char = '\u{2590}'; // ▐

    /// Right cap character for the bar.
    pub const RIGHT_CAP: char = '\u{258C}'; // ▌

    /// Fill character for the completed portion of the bar.
    pub fn fill_char(&self) -> char {
        match self.status {
            Status::Done => '\u{2588}',    // █
            Status::Blocked => '\u{2592}', // ▒
            Status::InProgress => '\u{2593}', // ▓
            // bar_fill() returns filled=0 for Todo, so this
            // is only reached if bar_fill logic changes.
            Status::Todo => '\u{2591}', // ░
        }
    }

    /// Fill character for the remaining portion of the bar.
    pub fn empty_char(&self) -> char {
        '\u{2591}' // ░
    }
}

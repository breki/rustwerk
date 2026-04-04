/// Summary of project status.
#[derive(Debug)]
pub struct ProjectSummary {
    /// Total number of tasks.
    pub total: u32,
    /// Tasks in TODO status.
    pub todo: u32,
    /// Tasks in `IN_PROGRESS` status.
    pub in_progress: u32,
    /// Tasks in BLOCKED status.
    pub blocked: u32,
    /// Tasks in DONE status.
    pub done: u32,
    /// Tasks in `ON_HOLD` status.
    pub on_hold: u32,
    /// Percentage complete (done/total * 100).
    pub pct_complete: f64,
    /// Sum of all effort estimates in hours.
    pub total_estimated_hours: f64,
    /// Sum of all logged effort in hours.
    pub total_actual_hours: f64,
    /// Sum of all complexity scores.
    pub total_complexity: u32,
}

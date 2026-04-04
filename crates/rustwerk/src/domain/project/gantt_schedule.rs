use std::collections::HashMap;

use super::gantt_row::GanttRow;
use super::Project;
use crate::domain::task::{Status, TaskId};

impl Project {
    /// Compute Gantt chart schedule. Returns rows sorted
    /// by start position then task ID.
    pub fn gantt_schedule(&self) -> Vec<GanttRow> {
        let order = self.topological_sort();
        let crit = self.critical_path_set();
        let mut end_at: HashMap<&TaskId, u32> = HashMap::new();
        let mut rows = Vec::new();

        for id in &order {
            let task = &self.tasks[id];
            let width = task.complexity.unwrap_or(1);

            // Start after all dependencies finish.
            let start = task
                .dependencies
                .iter()
                .filter_map(|dep| end_at.get(dep))
                .copied()
                .max()
                .unwrap_or(0);
            let end = start + width;
            end_at.insert(id, end);

            rows.push(GanttRow {
                id: id.clone(),
                start,
                width,
                status: task.status,
                critical: crit.contains(id),
            });
        }

        // Sort by start position, then by ID.
        rows.sort_by(|a, b| {
            a.start.cmp(&b.start).then_with(|| a.id.cmp(&b.id))
        });

        rows
    }

    /// Compute Gantt schedule for remaining (undone) tasks
    /// only. Done dependencies are treated as satisfied
    /// (start at 0), and the critical path is recalculated
    /// for remaining work.
    pub fn gantt_schedule_remaining(&self) -> Vec<GanttRow> {
        let order = self.topological_sort();
        let crit = self.remaining_critical_path_set();
        let mut end_at: HashMap<&TaskId, u32> = HashMap::new();
        let mut rows = Vec::new();

        for id in &order {
            let task = &self.tasks[id];
            if task.status == Status::Done || task.status == Status::OnHold {
                continue;
            }
            let width = task.complexity.unwrap_or(1);

            // Start after active dependencies finish.
            // Done and OnHold dependencies are ignored
            // (satisfied or deferred).
            let start = task
                .dependencies
                .iter()
                .filter(|dep| {
                    self.tasks.get(*dep).is_some_and(|t| {
                        t.status != Status::Done && t.status != Status::OnHold
                    })
                })
                .filter_map(|dep| end_at.get(dep))
                .copied()
                .max()
                .unwrap_or(0);
            let end = start + width;
            end_at.insert(id, end);

            rows.push(GanttRow {
                id: id.clone(),
                start,
                width,
                status: task.status,
                critical: crit.contains(id),
            });
        }

        rows.sort_by(|a, b| {
            a.start.cmp(&b.start).then_with(|| a.id.cmp(&b.id))
        });

        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::task::{Task, TaskId};

    fn project_with_tasks(ids: &[&str]) -> (Project, Vec<TaskId>) {
        let mut p = Project::new("Test").unwrap();
        let task_ids: Vec<TaskId> = ids
            .iter()
            .map(|id| {
                let tid = TaskId::new(id).unwrap();
                p.add_task(
                    tid.clone(),
                    Task::new(&format!("Task {id}")).unwrap(),
                )
                .unwrap();
                tid
            })
            .collect();
        (p, task_ids)
    }

    #[test]
    fn gantt_single_task() {
        let (p, _) = project_with_tasks(&["A"]);
        let rows = p.gantt_schedule();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].start, 0);
        assert_eq!(rows[0].width, 1); // default complexity
    }

    #[test]
    fn gantt_sequential_tasks() {
        let (mut p, ids) = project_with_tasks(&["A", "B"]);
        p.tasks.get_mut(&ids[0]).unwrap().complexity = Some(3);
        p.tasks.get_mut(&ids[1]).unwrap().complexity = Some(2);
        p.add_dependency(&ids[1], &ids[0]).unwrap(); // B->A
        let rows = p.gantt_schedule();
        let a = rows.iter().find(|r| r.id == ids[0]).unwrap();
        let b = rows.iter().find(|r| r.id == ids[1]).unwrap();
        assert_eq!(a.start, 0);
        assert_eq!(a.width, 3);
        assert_eq!(b.start, 3); // starts after A ends
        assert_eq!(b.width, 2);
    }

    #[test]
    fn gantt_parallel_tasks() {
        let (mut p, ids) = project_with_tasks(&["A", "B"]);
        p.tasks.get_mut(&ids[0]).unwrap().complexity = Some(5);
        p.tasks.get_mut(&ids[1]).unwrap().complexity = Some(3);
        // No dependencies — both start at 0.
        let rows = p.gantt_schedule();
        assert_eq!(rows[0].start, 0);
        assert_eq!(rows[1].start, 0);
    }

    #[test]
    fn gantt_diamond() {
        let (mut p, ids) = project_with_tasks(&["A", "B", "C"]);
        p.tasks.get_mut(&ids[0]).unwrap().complexity = Some(5);
        p.tasks.get_mut(&ids[1]).unwrap().complexity = Some(2);
        p.tasks.get_mut(&ids[2]).unwrap().complexity = Some(3);
        // C depends on A and B.
        p.add_dependency(&ids[2], &ids[0]).unwrap();
        p.add_dependency(&ids[2], &ids[1]).unwrap();
        let rows = p.gantt_schedule();
        let c = rows.iter().find(|r| r.id == ids[2]).unwrap();
        // C starts after the longer dep (A=5).
        assert_eq!(c.start, 5);
    }

    #[test]
    fn gantt_marks_critical_path() {
        let (mut p, ids) = project_with_tasks(&["A", "B"]);
        p.tasks.get_mut(&ids[0]).unwrap().complexity = Some(5);
        p.tasks.get_mut(&ids[1]).unwrap().complexity = Some(3);
        p.add_dependency(&ids[1], &ids[0]).unwrap();
        let rows = p.gantt_schedule();
        // Both are on the critical path (only chain).
        assert!(rows.iter().all(|r| r.critical));
    }

    #[test]
    fn gantt_empty_project() {
        let p = Project::new("Empty").unwrap();
        let rows = p.gantt_schedule();
        assert!(rows.is_empty());
    }

    // --- GanttRow Unicode rendering tests ---

    fn gantt_row(status: Status, width: u32) -> GanttRow {
        GanttRow {
            id: TaskId::new("T").unwrap(),
            start: 0,
            width,
            status,
            critical: false,
        }
    }

    #[test]
    fn fill_char_done_is_full_block() {
        let row = gantt_row(Status::Done, 5);
        assert_eq!(row.fill_char(), '\u{2588}'); // █
    }

    #[test]
    fn fill_char_in_progress_is_dark_shade() {
        let row = gantt_row(Status::InProgress, 5);
        assert_eq!(row.fill_char(), '\u{2593}'); // ▓
    }

    #[test]
    fn fill_char_blocked_is_medium_shade() {
        let row = gantt_row(Status::Blocked, 5);
        assert_eq!(row.fill_char(), '\u{2592}'); // ▒
    }

    #[test]
    fn fill_char_todo_is_light_shade() {
        let row = gantt_row(Status::Todo, 5);
        assert_eq!(row.fill_char(), '\u{2591}'); // ░
    }

    #[test]
    fn empty_char_is_light_shade() {
        let row = gantt_row(Status::Todo, 5);
        assert_eq!(row.empty_char(), '\u{2591}'); // ░
    }

    #[test]
    fn left_cap_is_right_half_block() {
        assert_eq!(GanttRow::LEFT_CAP, '\u{2590}'); // ▐
    }

    #[test]
    fn right_cap_is_left_half_block() {
        assert_eq!(GanttRow::RIGHT_CAP, '\u{258C}'); // ▌
    }
}

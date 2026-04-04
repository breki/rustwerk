use std::collections::HashMap;

use super::summary::ProjectSummary;
use super::Project;
use crate::domain::task::{Status, TaskId};

impl Project {
    /// Topological sort of all tasks (Kahn's algorithm).
    /// Returns task IDs in dependency order — a task
    /// appears after all its dependencies.
    pub fn topological_sort(&self) -> Vec<TaskId> {
        // Build in-degree map. A task's in-degree is the
        // number of tasks that list it as a dependency
        // (i.e. how many tasks it blocks).
        // NOTE: we reverse the semantics here — "depends
        // on" means the dependency must come first, so we
        // compute in-degree based on reverse edges.
        let mut in_degree: HashMap<&TaskId, usize> =
            self.tasks.keys().map(|id| (id, 0)).collect();
        let mut dependents: HashMap<&TaskId, Vec<&TaskId>> = HashMap::new();

        for (id, task) in &self.tasks {
            for dep in &task.dependencies {
                if self.tasks.contains_key(dep) {
                    *in_degree.entry(id).or_insert(0) += 1;
                    dependents.entry(dep).or_default().push(id);
                }
            }
        }

        // Start with tasks that have no dependencies
        // (in-degree 0).
        let mut queue: std::collections::VecDeque<&TaskId> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();
        // Sort the initial queue for deterministic output.
        let mut sorted_queue: Vec<&TaskId> = queue.drain(..).collect();
        sorted_queue.sort();
        queue.extend(sorted_queue);

        let mut result = Vec::with_capacity(self.tasks.len());
        while let Some(id) = queue.pop_front() {
            result.push(id.clone());
            if let Some(deps) = dependents.get(id) {
                let mut next = Vec::new();
                for &dep_id in deps {
                    let deg = in_degree.get_mut(dep_id).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        next.push(dep_id);
                    }
                }
                next.sort();
                queue.extend(next);
            }
        }
        result
    }

    /// Compute a project status summary.
    pub fn summary(&self) -> ProjectSummary {
        let mut todo = 0u32;
        let mut in_progress = 0u32;
        let mut blocked = 0u32;
        let mut done = 0u32;
        let mut on_hold = 0u32;
        let mut total_estimated_hours = 0.0_f64;
        let mut total_actual_hours = 0.0_f64;
        let mut total_complexity = 0u32;

        for task in self.tasks.values() {
            match task.status {
                Status::Todo => todo += 1,
                Status::InProgress => in_progress += 1,
                Status::Blocked => blocked += 1,
                Status::Done => done += 1,
                Status::OnHold => on_hold += 1,
            }
            if let Some(est) = &task.effort_estimate {
                total_estimated_hours += est.to_hours();
            }
            total_actual_hours += task.total_actual_effort_hours();
            if let Some(c) = task.complexity {
                total_complexity += c;
            }
        }

        let total = self.tasks.len() as u32;
        let pct_complete = if total == 0 {
            0.0
        } else {
            f64::from(done) / f64::from(total) * 100.0
        };

        ProjectSummary {
            total,
            todo,
            in_progress,
            blocked,
            done,
            on_hold,
            pct_complete,
            total_estimated_hours,
            total_actual_hours,
            total_complexity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::task::{Effort, EffortEntry, Task};
    use chrono::Utc;

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
    fn topological_sort_simple_chain() {
        let (mut p, ids) = project_with_tasks(&["A", "B", "C"]);
        p.add_dependency(&ids[0], &ids[1]).unwrap(); // A->B
        p.add_dependency(&ids[1], &ids[2]).unwrap(); // B->C
        let order = p.topological_sort();
        let names: Vec<&str> = order.iter().map(TaskId::as_str).collect();
        assert_eq!(names, vec!["C", "B", "A"]);
    }

    #[test]
    fn topological_sort_diamond() {
        let (mut p, ids) = project_with_tasks(&["A", "B", "C", "D"]);
        // A depends on B and C; B and C depend on D.
        p.add_dependency(&ids[0], &ids[1]).unwrap(); // A->B
        p.add_dependency(&ids[0], &ids[2]).unwrap(); // A->C
        p.add_dependency(&ids[1], &ids[3]).unwrap(); // B->D
        p.add_dependency(&ids[2], &ids[3]).unwrap(); // C->D
        let order = p.topological_sort();
        let names: Vec<&str> = order.iter().map(TaskId::as_str).collect();
        // D must come first, A must come last.
        assert_eq!(names[0], "D");
        assert_eq!(*names.last().unwrap(), "A");
    }

    #[test]
    fn topological_sort_no_deps() {
        let (p, _) = project_with_tasks(&["A", "B", "C"]);
        let order = p.topological_sort();
        // All independent — alphabetical order.
        assert_eq!(order.len(), 3);
    }

    #[test]
    fn summary_empty_project() {
        let p = Project::new("Test").unwrap();
        let s = p.summary();
        assert_eq!(s.total, 0);
        assert_eq!(s.done, 0);
        assert!((s.pct_complete - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn summary_counts_by_status() {
        let (mut p, ids) = project_with_tasks(&["A", "B", "C", "D"]);
        p.set_status(&ids[0], Status::InProgress, false).unwrap();
        p.set_status(&ids[0], Status::Done, false).unwrap();
        p.set_status(&ids[1], Status::InProgress, false).unwrap();
        // C stays TODO, D stays TODO.
        let s = p.summary();
        assert_eq!(s.total, 4);
        assert_eq!(s.done, 1);
        assert_eq!(s.in_progress, 1);
        assert_eq!(s.todo, 2);
        assert_eq!(s.blocked, 0);
        assert!((s.pct_complete - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn summary_effort_totals() {
        let (mut p, ids) = project_with_tasks(&["A"]);
        p.tasks.get_mut(&ids[0]).unwrap().complexity = Some(5);
        p.set_effort_estimate(&ids[0], Effort::parse("8H").unwrap())
            .unwrap();
        p.set_status(&ids[0], Status::InProgress, false).unwrap();
        p.log_effort(
            &ids[0],
            EffortEntry {
                effort: Effort::parse("3H").unwrap(),
                developer: "alice".into(),
                timestamp: Utc::now(),
                note: None,
            },
        )
        .unwrap();
        let s = p.summary();
        assert!((s.total_estimated_hours - 8.0).abs() < f64::EPSILON);
        assert!((s.total_actual_hours - 3.0).abs() < f64::EPSILON);
        assert_eq!(s.total_complexity, 5);
    }
}

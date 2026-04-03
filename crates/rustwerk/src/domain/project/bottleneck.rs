use std::collections::{HashMap, HashSet};

use super::Project;
use crate::domain::task::{Status, Task, TaskId};

/// A task identified as a scheduling bottleneck.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bottleneck {
    /// The bottleneck task ID.
    pub id: TaskId,
    /// Number of non-done tasks transitively blocked by
    /// this task.
    pub downstream_count: usize,
    /// Current status of the bottleneck task.
    pub status: Status,
    /// Assignee, if any.
    pub assignee: Option<String>,
    /// Whether this task is ready for implementation
    /// (all dependencies are done).
    pub ready: bool,
}

impl Project {
    /// Build reverse adjacency: for each task, the list of
    /// tasks that directly depend on it. Only includes
    /// dependents matching the predicate.
    pub(super) fn reverse_dependents(
        &self,
        include: impl Fn(&Task) -> bool,
    ) -> HashMap<&TaskId, Vec<&TaskId>> {
        let mut map: HashMap<&TaskId, Vec<&TaskId>> =
            HashMap::new();
        for (id, task) in &self.tasks {
            if !include(task) {
                continue;
            }
            for dep in &task.dependencies {
                if self.tasks.contains_key(dep) {
                    map.entry(dep).or_default().push(id);
                }
            }
        }
        map
    }

    /// Detect bottleneck tasks — tasks with the most
    /// transitive downstream dependents. Returns
    /// [`Bottleneck`] entries sorted by count descending,
    /// then by ID ascending. Only includes non-done tasks
    /// with at least one non-done downstream dependent.
    pub fn bottlenecks(&self) -> Vec<Bottleneck> {
        let direct_dependents = self
            .reverse_dependents(|t| t.status != Status::Done);

        // For each non-done task, DFS to count all
        // transitive downstream dependents.
        let mut results: Vec<Bottleneck> = self
            .tasks
            .keys()
            .filter(|id| {
                self.tasks[*id].status != Status::Done
            })
            .filter_map(|id| {
                let mut visited = HashSet::new();
                let mut stack = vec![id];
                while let Some(current) = stack.pop() {
                    if let Some(deps) =
                        direct_dependents.get(current)
                    {
                        for &dep in deps {
                            if visited.insert(dep) {
                                stack.push(dep);
                            }
                        }
                    }
                }
                let count = visited.len();
                if count > 0 {
                    let task = &self.tasks[id];
                    let ready = task
                        .dependencies
                        .iter()
                        .all(|dep| {
                            self.tasks
                                .get(dep)
                                .is_some_and(|t| {
                                    t.status == Status::Done
                                })
                        });
                    Some(Bottleneck {
                        id: id.clone(),
                        downstream_count: count,
                        status: task.status,
                        assignee: task
                            .assignee
                            .clone(),
                        ready,
                    })
                } else {
                    None
                }
            })
            .collect();

        results.sort_by(|a, b| {
            b.downstream_count
                .cmp(&a.downstream_count)
                .then_with(|| a.id.cmp(&b.id))
        });
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::task::{Task, TaskId};

    fn project_with_tasks(
        ids: &[&str],
    ) -> (Project, Vec<TaskId>) {
        let mut p = Project::new("Test").unwrap();
        let task_ids: Vec<TaskId> = ids
            .iter()
            .map(|id| {
                let tid = TaskId::new(id).unwrap();
                p.add_task(
                    tid.clone(),
                    Task::new(&format!("Task {id}"))
                        .unwrap(),
                )
                .unwrap();
                tid
            })
            .collect();
        (p, task_ids)
    }

    #[test]
    fn bottlenecks_empty_project() {
        let p = Project::new("Empty").unwrap();
        assert!(p.bottlenecks().is_empty());
    }

    #[test]
    fn bottlenecks_no_deps() {
        let (p, _) = project_with_tasks(&["A", "B", "C"]);
        let bn = p.bottlenecks();
        // No dependencies -> all tasks have 0 downstream
        // dependents -> nothing returned.
        assert!(bn.is_empty());
    }

    #[test]
    fn bottlenecks_linear_chain() {
        // C depends on B, B depends on A.
        // A blocks B and C (2 downstream).
        // B blocks C (1 downstream).
        // C blocks nothing (0).
        let (mut p, ids) =
            project_with_tasks(&["A", "B", "C"]);
        p.add_dependency(&ids[1], &ids[0]).unwrap(); // B->A
        p.add_dependency(&ids[2], &ids[1]).unwrap(); // C->B
        let bn = p.bottlenecks();
        assert_eq!(bn.len(), 2);
        assert_eq!(bn[0].id.as_str(), "A");
        assert_eq!(bn[0].downstream_count, 2);
        assert!(bn[0].ready); // A has no deps -> ready
        assert_eq!(bn[1].id.as_str(), "B");
        assert_eq!(bn[1].downstream_count, 1);
        assert!(!bn[1].ready); // B depends on A (todo)
    }

    #[test]
    fn bottlenecks_fan_out() {
        // B, C, D all depend on A.
        // A blocks 3 downstream tasks.
        let (mut p, ids) =
            project_with_tasks(&["A", "B", "C", "D"]);
        p.add_dependency(&ids[1], &ids[0]).unwrap(); // B->A
        p.add_dependency(&ids[2], &ids[0]).unwrap(); // C->A
        p.add_dependency(&ids[3], &ids[0]).unwrap(); // D->A
        let bn = p.bottlenecks();
        assert_eq!(bn.len(), 1);
        assert_eq!(bn[0].id.as_str(), "A");
        assert_eq!(bn[0].downstream_count, 3);
    }

    #[test]
    fn bottlenecks_diamond() {
        // B and C depend on A. D depends on B and C.
        // A blocks B, C, D = 3 downstream.
        // B blocks D = 1 downstream.
        // C blocks D = 1 downstream.
        let (mut p, ids) =
            project_with_tasks(&["A", "B", "C", "D"]);
        p.add_dependency(&ids[1], &ids[0]).unwrap(); // B->A
        p.add_dependency(&ids[2], &ids[0]).unwrap(); // C->A
        p.add_dependency(&ids[3], &ids[1]).unwrap(); // D->B
        p.add_dependency(&ids[3], &ids[2]).unwrap(); // D->C
        let bn = p.bottlenecks();
        assert_eq!(bn.len(), 3);
        assert_eq!(bn[0].id.as_str(), "A");
        assert_eq!(bn[0].downstream_count, 3);
        // B and C both have 1 downstream, sorted by ID.
        assert_eq!(bn[1].id.as_str(), "B");
        assert_eq!(bn[1].downstream_count, 1);
        assert_eq!(bn[2].id.as_str(), "C");
        assert_eq!(bn[2].downstream_count, 1);
    }

    #[test]
    fn bottlenecks_ready_after_dep_done() {
        // C depends on B, B depends on A.
        // Complete A -> B becomes ready.
        let (mut p, ids) =
            project_with_tasks(&["A", "B", "C"]);
        p.add_dependency(&ids[1], &ids[0]).unwrap(); // B->A
        p.add_dependency(&ids[2], &ids[1]).unwrap(); // C->B
        p.set_status(&ids[0], Status::InProgress, false)
            .unwrap();
        p.set_status(&ids[0], Status::Done, false).unwrap();
        let bn = p.bottlenecks();
        // Only B is a bottleneck (A is done).
        assert_eq!(bn.len(), 1);
        assert_eq!(bn[0].id.as_str(), "B");
        assert!(bn[0].ready); // A is done -> B ready
    }

    #[test]
    fn bottlenecks_includes_status_and_assignee() {
        let (mut p, ids) =
            project_with_tasks(&["A", "B"]);
        p.add_dependency(&ids[1], &ids[0]).unwrap(); // B->A
        p.set_status(&ids[0], Status::InProgress, false)
            .unwrap();
        let bn = p.bottlenecks();
        assert_eq!(bn.len(), 1);
        assert_eq!(bn[0].id.as_str(), "A");
        assert_eq!(bn[0].status, Status::InProgress);
        assert_eq!(bn[0].assignee, None);
    }

    #[test]
    fn bottlenecks_excludes_done_tasks() {
        // B depends on A. A is done -> not a bottleneck.
        let (mut p, ids) =
            project_with_tasks(&["A", "B"]);
        p.add_dependency(&ids[1], &ids[0]).unwrap(); // B->A
        p.set_status(&ids[0], Status::InProgress, false)
            .unwrap();
        p.set_status(&ids[0], Status::Done, false).unwrap();
        let bn = p.bottlenecks();
        // A is done, so excluded.
        assert!(bn.is_empty());
    }
}

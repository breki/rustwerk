use std::collections::HashMap;

use super::Project;
use crate::domain::task::{Status, Task, TaskId};

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
        let mut dependents: HashMap<
            &TaskId,
            Vec<&TaskId>,
        > = HashMap::new();

        for (id, task) in &self.tasks {
            for dep in &task.dependencies {
                if self.tasks.contains_key(dep) {
                    *in_degree.entry(id).or_insert(0) += 1;
                    dependents
                        .entry(dep)
                        .or_default()
                        .push(id);
                }
            }
        }

        // Start with tasks that have no dependencies
        // (in-degree 0).
        let mut queue: std::collections::VecDeque<&TaskId> =
            in_degree
                .iter()
                .filter(|(_, &deg)| deg == 0)
                .map(|(&id, _)| id)
                .collect();
        // Sort the initial queue for deterministic output.
        let mut sorted_queue: Vec<&TaskId> =
            queue.drain(..).collect();
        sorted_queue.sort();
        queue.extend(sorted_queue);

        let mut result = Vec::with_capacity(self.tasks.len());
        while let Some(id) = queue.pop_front() {
            result.push(id.clone());
            if let Some(deps) = dependents.get(id) {
                let mut next = Vec::new();
                for &dep_id in deps {
                    let deg =
                        in_degree.get_mut(dep_id).unwrap();
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

    /// Compute the critical path — the longest chain of
    /// tasks by complexity weight. Returns (path, total
    /// complexity). Uses the topological sort and
    /// dynamic programming.
    pub fn critical_path(&self) -> (Vec<TaskId>, u32) {
        let order = self.topological_sort();
        if order.is_empty() {
            return (Vec::new(), 0);
        }

        // dist[id] = longest path ending at id.
        // prev[id] = predecessor on that path.
        let mut dist: HashMap<&TaskId, u32> = HashMap::new();
        let mut prev: HashMap<&TaskId, Option<&TaskId>> =
            HashMap::new();

        for id in &order {
            let task = &self.tasks[id];
            let weight = task.complexity.unwrap_or(1);
            dist.insert(id, weight);
            prev.insert(id, None);
        }

        for id in &order {
            let id_dist = dist[id];

            // For each task that depends on `id`, see if
            // going through `id` gives a longer path.
            for (other_id, other_task) in &self.tasks {
                if other_task.dependencies.contains(id) {
                    let other_weight =
                        other_task.complexity.unwrap_or(1);
                    let candidate = id_dist + other_weight;
                    if candidate > dist[other_id] {
                        dist.insert(other_id, candidate);
                        prev.insert(
                            other_id,
                            Some(id),
                        );
                    }
                }
            }
        }

        // Find the task with the maximum distance.
        let (&end, &max_dist) =
            dist.iter().max_by_key(|(_, &d)| d).unwrap();

        // Trace back the path.
        let mut path = vec![end.clone()];
        let mut current = end;
        while let Some(Some(p)) = prev.get(current) {
            path.push((*p).clone());
            current = p;
        }
        path.reverse();

        (path, max_dist)
    }

    /// Compute the critical path considering only tasks
    /// that are not done. This shows the longest remaining
    /// chain of work.
    pub fn remaining_critical_path(
        &self,
    ) -> (Vec<TaskId>, u32) {
        // Filter to undone tasks only.
        let undone: std::collections::BTreeMap<&TaskId, &Task> =
            self.tasks
                .iter()
                .filter(|(_, t)| t.status != Status::Done)
                .collect();

        if undone.is_empty() {
            return (Vec::new(), 0);
        }

        // Build in-degree for undone tasks (only count
        // deps that are also undone).
        let mut in_degree: HashMap<&TaskId, usize> =
            undone.keys().map(|&id| (id, 0)).collect();
        let mut dependents: HashMap<
            &TaskId,
            Vec<&TaskId>,
        > = HashMap::new();

        for (&id, task) in &undone {
            for dep in &task.dependencies {
                if undone.contains_key(dep) {
                    *in_degree.entry(id).or_insert(0) += 1;
                    dependents
                        .entry(dep)
                        .or_default()
                        .push(id);
                }
            }
        }

        // Kahn's topological sort on undone tasks.
        let mut queue: std::collections::VecDeque<&TaskId> =
            in_degree
                .iter()
                .filter(|(_, &deg)| deg == 0)
                .map(|(&id, _)| id)
                .collect();
        let mut sorted_queue: Vec<&TaskId> =
            queue.drain(..).collect();
        sorted_queue.sort();
        queue.extend(sorted_queue);

        let mut order = Vec::new();
        while let Some(id) = queue.pop_front() {
            order.push(id);
            if let Some(deps) = dependents.get(id) {
                let mut next = Vec::new();
                for &dep_id in deps {
                    let deg =
                        in_degree.get_mut(dep_id).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        next.push(dep_id);
                    }
                }
                next.sort();
                queue.extend(next);
            }
        }

        if order.is_empty() {
            return (Vec::new(), 0);
        }

        // DP longest path on undone tasks.
        let mut dist: HashMap<&TaskId, u32> =
            HashMap::new();
        let mut prev: HashMap<&TaskId, Option<&TaskId>> =
            HashMap::new();

        for &id in &order {
            let weight =
                undone[id].complexity.unwrap_or(1);
            dist.insert(id, weight);
            prev.insert(id, None);
        }

        for &id in &order {
            let id_dist = dist[id];
            if let Some(deps) = dependents.get(id) {
                for &dep_id in deps {
                    let dep_weight =
                        undone[dep_id].complexity.unwrap_or(1);
                    let candidate = id_dist + dep_weight;
                    if candidate > dist[dep_id] {
                        dist.insert(dep_id, candidate);
                        prev.insert(dep_id, Some(id));
                    }
                }
            }
        }

        let (&end, &max_dist) =
            dist.iter().max_by_key(|(_, &d)| d).unwrap();

        let mut path = vec![end.clone()];
        let mut current = end;
        while let Some(Some(p)) = prev.get(current) {
            path.push((*p).clone());
            current = p;
        }
        path.reverse();

        (path, max_dist)
    }

    /// Return the set of task IDs on the critical path.
    /// Return the set of task IDs on the critical path
    /// (all tasks, including done).
    pub fn critical_path_set(
        &self,
    ) -> std::collections::HashSet<TaskId> {
        let (path, _) = self.critical_path();
        path.into_iter().collect()
    }

    /// Return the set of task IDs on the remaining
    /// critical path (excluding done tasks).
    pub fn remaining_critical_path_set(
        &self,
    ) -> std::collections::HashSet<TaskId> {
        let (path, _) = self.remaining_critical_path();
        path.into_iter().collect()
    }

    /// Return task IDs that are blocked by incomplete
    /// dependencies: status is TODO and at least one
    /// dependency is not done.
    pub fn dep_blocked_tasks(&self) -> Vec<&TaskId> {
        self.tasks
            .iter()
            .filter(|(_, task)| {
                task.status == Status::Todo
                    && !task.dependencies.is_empty()
                    && task.dependencies.iter().any(|dep| {
                        self.tasks
                            .get(dep)
                            .is_none_or(|t| {
                                t.status != Status::Done
                            })
                    })
            })
            .map(|(id, _)| id)
            .collect()
    }

    /// Return task IDs that are ready to start: status is
    /// TODO and all dependencies are done.
    pub fn available_tasks(&self) -> Vec<&TaskId> {
        self.tasks
            .iter()
            .filter(|(_, task)| {
                task.status == Status::Todo
                    && task.dependencies.iter().all(|dep| {
                        self.tasks
                            .get(dep)
                            .is_some_and(|t| {
                                t.status == Status::Done
                            })
                    })
            })
            .map(|(id, _)| id)
            .collect()
    }

    /// Return task IDs that are currently in progress.
    pub fn active_tasks(&self) -> Vec<&TaskId> {
        self.tasks
            .iter()
            .filter(|(_, task)| {
                task.status == Status::InProgress
            })
            .map(|(id, _)| id)
            .collect()
    }

    /// Compute a project status summary.
    pub fn summary(&self) -> ProjectSummary {
        let mut todo = 0u32;
        let mut in_progress = 0u32;
        let mut blocked = 0u32;
        let mut done = 0u32;
        let mut total_estimated_hours = 0.0_f64;
        let mut total_actual_hours = 0.0_f64;
        let mut total_complexity = 0u32;

        for task in self.tasks.values() {
            match task.status {
                Status::Todo => todo += 1,
                Status::InProgress => in_progress += 1,
                Status::Blocked => blocked += 1,
                Status::Done => done += 1,
            }
            if let Some(est) = &task.effort_estimate {
                total_estimated_hours += est.to_hours();
            }
            total_actual_hours +=
                task.total_actual_effort_hours();
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
            pct_complete,
            total_estimated_hours,
            total_actual_hours,
            total_complexity,
        }
    }

    /// Compute Gantt chart schedule. Returns rows sorted
    /// by start position then task ID.
    pub fn gantt_schedule(&self) -> Vec<GanttRow> {
        let order = self.topological_sort();
        let crit = self.critical_path_set();
        let mut end_at: HashMap<&TaskId, u32> =
            HashMap::new();
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
            a.start
                .cmp(&b.start)
                .then_with(|| a.id.cmp(&b.id))
        });

        rows
    }

    /// Compute Gantt schedule for remaining (undone) tasks
    /// only. Done dependencies are treated as satisfied
    /// (start at 0), and the critical path is recalculated
    /// for remaining work.
    pub fn gantt_schedule_remaining(
        &self,
    ) -> Vec<GanttRow> {
        let order = self.topological_sort();
        let crit = self.remaining_critical_path_set();
        let mut end_at: HashMap<&TaskId, u32> =
            HashMap::new();
        let mut rows = Vec::new();

        for id in &order {
            let task = &self.tasks[id];
            if task.status == Status::Done {
                continue;
            }
            let width = task.complexity.unwrap_or(1);

            // Start after undone dependencies finish.
            // Done dependencies are ignored (already
            // satisfied).
            let start = task
                .dependencies
                .iter()
                .filter(|dep| {
                    self.tasks
                        .get(*dep)
                        .is_some_and(|t| {
                            t.status != Status::Done
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
            a.start
                .cmp(&b.start)
                .then_with(|| a.id.cmp(&b.id))
        });

        rows
    }
}

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

/// Summary of project status.
#[derive(Debug)]
pub struct ProjectSummary {
    /// Total number of tasks.
    pub total: u32,
    /// Tasks in TODO status.
    pub todo: u32,
    /// Tasks in IN_PROGRESS status.
    pub in_progress: u32,
    /// Tasks in BLOCKED status.
    pub blocked: u32,
    /// Tasks in DONE status.
    pub done: u32,
    /// Percentage complete (done/total * 100).
    pub pct_complete: f64,
    /// Sum of all effort estimates in hours.
    pub total_estimated_hours: f64,
    /// Sum of all logged effort in hours.
    pub total_actual_hours: f64,
    /// Sum of all complexity scores.
    pub total_complexity: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use crate::domain::task::{Effort, EffortEntry};

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
    fn available_tasks_no_deps() {
        let (p, _) = project_with_tasks(&["A", "B"]);
        let avail = p.available_tasks();
        assert_eq!(avail.len(), 2);
    }

    #[test]
    fn available_tasks_with_deps() {
        let (mut p, ids) =
            project_with_tasks(&["A", "B", "C"]);
        p.add_dependency(&ids[1], &ids[0]).unwrap(); // B->A
        p.add_dependency(&ids[2], &ids[1]).unwrap(); // C->B
        // Only A is available (no deps).
        let avail = p.available_tasks();
        assert_eq!(avail.len(), 1);
        assert_eq!(avail[0].as_str(), "A");
    }

    #[test]
    fn available_tasks_after_completing_dep() {
        let (mut p, ids) =
            project_with_tasks(&["A", "B"]);
        p.add_dependency(&ids[1], &ids[0]).unwrap(); // B->A
        // Complete A.
        p.set_status(&ids[0], Status::InProgress, false)
            .unwrap();
        p.set_status(&ids[0], Status::Done, false).unwrap();
        // Now B is available.
        let avail = p.available_tasks();
        assert_eq!(avail.len(), 1);
        assert_eq!(avail[0].as_str(), "B");
    }

    #[test]
    fn available_tasks_excludes_done() {
        let (mut p, ids) = project_with_tasks(&["A"]);
        p.set_status(&ids[0], Status::InProgress, false)
            .unwrap();
        p.set_status(&ids[0], Status::Done, false).unwrap();
        let avail = p.available_tasks();
        assert!(avail.is_empty());
    }

    #[test]
    fn available_tasks_excludes_in_progress() {
        let (mut p, ids) = project_with_tasks(&["A"]);
        p.set_status(&ids[0], Status::InProgress, false)
            .unwrap();
        let avail = p.available_tasks();
        assert!(avail.is_empty());
    }

    #[test]
    fn active_tasks_returns_in_progress() {
        let (mut p, ids) =
            project_with_tasks(&["A", "B"]);
        p.set_status(&ids[0], Status::InProgress, false)
            .unwrap();
        let active = p.active_tasks();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].as_str(), "A");
    }

    #[test]
    fn active_tasks_empty_when_none_in_progress() {
        let (p, _) = project_with_tasks(&["A", "B"]);
        assert!(p.active_tasks().is_empty());
    }

    #[test]
    fn dep_blocked_with_incomplete_deps() {
        let (mut p, ids) =
            project_with_tasks(&["A", "B", "C"]);
        p.add_dependency(&ids[1], &ids[0]).unwrap(); // B->A
        p.add_dependency(&ids[2], &ids[1]).unwrap(); // C->B
        // A has no deps → not blocked.
        // B depends on A (todo) → blocked.
        // C depends on B (todo) → blocked.
        let blocked = p.dep_blocked_tasks();
        assert_eq!(blocked.len(), 2);
        assert!(blocked.contains(&&ids[1]));
        assert!(blocked.contains(&&ids[2]));
    }

    #[test]
    fn dep_blocked_clears_when_dep_done() {
        let (mut p, ids) =
            project_with_tasks(&["A", "B"]);
        p.add_dependency(&ids[1], &ids[0]).unwrap(); // B->A
        assert_eq!(p.dep_blocked_tasks().len(), 1);

        // Complete A.
        p.set_status(&ids[0], Status::InProgress, false)
            .unwrap();
        p.set_status(&ids[0], Status::Done, false)
            .unwrap();
        // B is no longer blocked.
        assert!(p.dep_blocked_tasks().is_empty());
    }

    #[test]
    fn dep_blocked_excludes_done_and_in_progress() {
        let (mut p, ids) =
            project_with_tasks(&["A", "B"]);
        p.add_dependency(&ids[1], &ids[0]).unwrap(); // B->A
        // Move B to in-progress (force, bypassing
        // normal transitions for test).
        p.set_status(&ids[1], Status::InProgress, true)
            .unwrap();
        // B is in-progress, not blocked by deps.
        assert!(p.dep_blocked_tasks().is_empty());
    }

    #[test]
    fn dep_blocked_no_deps_returns_empty() {
        let (p, _) = project_with_tasks(&["A", "B"]);
        assert!(p.dep_blocked_tasks().is_empty());
    }

    #[test]
    fn topological_sort_simple_chain() {
        let (mut p, ids) =
            project_with_tasks(&["A", "B", "C"]);
        p.add_dependency(&ids[0], &ids[1]).unwrap(); // A->B
        p.add_dependency(&ids[1], &ids[2]).unwrap(); // B->C
        let order = p.topological_sort();
        let names: Vec<&str> =
            order.iter().map(TaskId::as_str).collect();
        assert_eq!(names, vec!["C", "B", "A"]);
    }

    #[test]
    fn topological_sort_diamond() {
        let (mut p, ids) =
            project_with_tasks(&["A", "B", "C", "D"]);
        // A depends on B and C; B and C depend on D.
        p.add_dependency(&ids[0], &ids[1]).unwrap(); // A->B
        p.add_dependency(&ids[0], &ids[2]).unwrap(); // A->C
        p.add_dependency(&ids[1], &ids[3]).unwrap(); // B->D
        p.add_dependency(&ids[2], &ids[3]).unwrap(); // C->D
        let order = p.topological_sort();
        let names: Vec<&str> =
            order.iter().map(TaskId::as_str).collect();
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
    fn critical_path_linear() {
        let (mut p, ids) =
            project_with_tasks(&["A", "B", "C"]);
        p.tasks.get_mut(&ids[0]).unwrap().complexity =
            Some(3);
        p.tasks.get_mut(&ids[1]).unwrap().complexity =
            Some(2);
        p.tasks.get_mut(&ids[2]).unwrap().complexity =
            Some(1);
        p.add_dependency(&ids[0], &ids[1]).unwrap(); // A->B
        p.add_dependency(&ids[1], &ids[2]).unwrap(); // B->C
        let (path, total) = p.critical_path();
        let names: Vec<&str> =
            path.iter().map(TaskId::as_str).collect();
        assert_eq!(names, vec!["C", "B", "A"]);
        assert_eq!(total, 6);
    }

    #[test]
    fn critical_path_parallel_branches() {
        let (mut p, ids) = project_with_tasks(
            &["END", "LONG", "SHORT", "START"],
        );
        // END depends on LONG and SHORT.
        // LONG depends on START (weight 5).
        // SHORT depends on START (weight 1).
        p.tasks
            .get_mut(&ids[0])
            .unwrap()
            .complexity = Some(1); // END
        p.tasks
            .get_mut(&ids[1])
            .unwrap()
            .complexity = Some(5); // LONG
        p.tasks
            .get_mut(&ids[2])
            .unwrap()
            .complexity = Some(1); // SHORT
        p.tasks
            .get_mut(&ids[3])
            .unwrap()
            .complexity = Some(1); // START
        p.add_dependency(&ids[0], &ids[1]).unwrap(); // END->LONG
        p.add_dependency(&ids[0], &ids[2]).unwrap(); // END->SHORT
        p.add_dependency(&ids[1], &ids[3]).unwrap(); // LONG->START
        p.add_dependency(&ids[2], &ids[3]).unwrap(); // SHORT->START
        let (path, total) = p.critical_path();
        let names: Vec<&str> =
            path.iter().map(TaskId::as_str).collect();
        // Critical path goes through LONG, not SHORT.
        assert_eq!(names, vec!["START", "LONG", "END"]);
        assert_eq!(total, 7);
    }

    #[test]
    fn critical_path_empty_project() {
        let p = Project::new("Empty").unwrap();
        let (path, total) = p.critical_path();
        assert!(path.is_empty());
        assert_eq!(total, 0);
    }

    #[test]
    fn critical_path_set_contains_correct_ids() {
        let (mut p, ids) =
            project_with_tasks(&["A", "B", "C"]);
        p.tasks.get_mut(&ids[0]).unwrap().complexity =
            Some(3);
        p.tasks.get_mut(&ids[1]).unwrap().complexity =
            Some(2);
        p.tasks.get_mut(&ids[2]).unwrap().complexity =
            Some(1);
        p.add_dependency(&ids[0], &ids[1]).unwrap();
        p.add_dependency(&ids[1], &ids[2]).unwrap();
        let crit = p.critical_path_set();
        assert!(crit.contains(&ids[0]));
        assert!(crit.contains(&ids[1]));
        assert!(crit.contains(&ids[2]));
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
        let (mut p, ids) =
            project_with_tasks(&["A", "B", "C", "D"]);
        p.set_status(&ids[0], Status::InProgress, false)
            .unwrap();
        p.set_status(&ids[0], Status::Done, false)
            .unwrap();
        p.set_status(&ids[1], Status::InProgress, false)
            .unwrap();
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
        p.tasks.get_mut(&ids[0]).unwrap().complexity =
            Some(5);
        p.set_effort_estimate(
            &ids[0],
            Effort::parse("8H").unwrap(),
        )
        .unwrap();
        p.set_status(&ids[0], Status::InProgress, false)
            .unwrap();
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
        assert!(
            (s.total_estimated_hours - 8.0).abs()
                < f64::EPSILON
        );
        assert!(
            (s.total_actual_hours - 3.0).abs()
                < f64::EPSILON
        );
        assert_eq!(s.total_complexity, 5);
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
        let (mut p, ids) =
            project_with_tasks(&["A", "B"]);
        p.tasks.get_mut(&ids[0]).unwrap().complexity =
            Some(3);
        p.tasks.get_mut(&ids[1]).unwrap().complexity =
            Some(2);
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
        let (mut p, ids) =
            project_with_tasks(&["A", "B"]);
        p.tasks.get_mut(&ids[0]).unwrap().complexity =
            Some(5);
        p.tasks.get_mut(&ids[1]).unwrap().complexity =
            Some(3);
        // No dependencies — both start at 0.
        let rows = p.gantt_schedule();
        assert_eq!(rows[0].start, 0);
        assert_eq!(rows[1].start, 0);
    }

    #[test]
    fn gantt_diamond() {
        let (mut p, ids) =
            project_with_tasks(&["A", "B", "C"]);
        p.tasks.get_mut(&ids[0]).unwrap().complexity =
            Some(5);
        p.tasks.get_mut(&ids[1]).unwrap().complexity =
            Some(2);
        p.tasks.get_mut(&ids[2]).unwrap().complexity =
            Some(3);
        // C depends on A and B.
        p.add_dependency(&ids[2], &ids[0]).unwrap();
        p.add_dependency(&ids[2], &ids[1]).unwrap();
        let rows = p.gantt_schedule();
        let c = rows
            .iter()
            .find(|r| r.id == ids[2])
            .unwrap();
        // C starts after the longer dep (A=5).
        assert_eq!(c.start, 5);
    }

    #[test]
    fn gantt_marks_critical_path() {
        let (mut p, ids) =
            project_with_tasks(&["A", "B"]);
        p.tasks.get_mut(&ids[0]).unwrap().complexity =
            Some(5);
        p.tasks.get_mut(&ids[1]).unwrap().complexity =
            Some(3);
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

    fn gantt_row(
        status: Status,
        width: u32,
    ) -> GanttRow {
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

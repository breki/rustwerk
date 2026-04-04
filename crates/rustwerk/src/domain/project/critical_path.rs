use std::collections::HashMap;

use super::Project;
use crate::domain::task::{Status, TaskId};

impl Project {
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
        let mut prev: HashMap<&TaskId, Option<&TaskId>> = HashMap::new();

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
                    let other_weight = other_task.complexity.unwrap_or(1);
                    let candidate = id_dist + other_weight;
                    if candidate > dist[other_id] {
                        dist.insert(other_id, candidate);
                        prev.insert(other_id, Some(id));
                    }
                }
            }
        }

        // Find the task with the maximum distance.
        let (&end, &max_dist) = dist.iter().max_by_key(|(_, &d)| d).unwrap();

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

    /// Compute the critical path considering only active
    /// tasks (excludes Done and OnHold). This shows the
    /// longest remaining chain of work.
    pub fn remaining_critical_path(&self) -> (Vec<TaskId>, u32) {
        // Filter to active tasks (not done, not on hold).
        let undone: std::collections::BTreeMap<&TaskId, _> = self
            .tasks
            .iter()
            .filter(|(_, t)| {
                t.status != Status::Done && t.status != Status::OnHold
            })
            .collect();

        if undone.is_empty() {
            return (Vec::new(), 0);
        }

        // Build in-degree for undone tasks (only count
        // deps that are also undone).
        let mut in_degree: HashMap<&TaskId, usize> =
            undone.keys().map(|&id| (id, 0)).collect();
        let mut dependents: HashMap<&TaskId, Vec<&TaskId>> = HashMap::new();

        for (&id, task) in &undone {
            for dep in &task.dependencies {
                if undone.contains_key(dep) {
                    *in_degree.entry(id).or_insert(0) += 1;
                    dependents.entry(dep).or_default().push(id);
                }
            }
        }

        // Kahn's topological sort on undone tasks.
        let mut queue: std::collections::VecDeque<&TaskId> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();
        let mut sorted_queue: Vec<&TaskId> = queue.drain(..).collect();
        sorted_queue.sort();
        queue.extend(sorted_queue);

        let mut order = Vec::new();
        while let Some(id) = queue.pop_front() {
            order.push(id);
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

        if order.is_empty() {
            return (Vec::new(), 0);
        }

        // DP longest path on undone tasks.
        let mut dist: HashMap<&TaskId, u32> = HashMap::new();
        let mut prev: HashMap<&TaskId, Option<&TaskId>> = HashMap::new();

        for &id in &order {
            let weight = undone[id].complexity.unwrap_or(1);
            dist.insert(id, weight);
            prev.insert(id, None);
        }

        for &id in &order {
            let id_dist = dist[id];
            if let Some(deps) = dependents.get(id) {
                for &dep_id in deps {
                    let dep_weight = undone[dep_id].complexity.unwrap_or(1);
                    let candidate = id_dist + dep_weight;
                    if candidate > dist[dep_id] {
                        dist.insert(dep_id, candidate);
                        prev.insert(dep_id, Some(id));
                    }
                }
            }
        }

        let (&end, &max_dist) = dist.iter().max_by_key(|(_, &d)| d).unwrap();

        let mut path = vec![end.clone()];
        let mut current = end;
        while let Some(Some(p)) = prev.get(current) {
            path.push((*p).clone());
            current = p;
        }
        path.reverse();

        (path, max_dist)
    }

    /// Return the set of task IDs on the critical path
    /// (all tasks, including done).
    pub fn critical_path_set(&self) -> std::collections::HashSet<TaskId> {
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
    fn critical_path_linear() {
        let (mut p, ids) = project_with_tasks(&["A", "B", "C"]);
        p.tasks.get_mut(&ids[0]).unwrap().complexity = Some(3);
        p.tasks.get_mut(&ids[1]).unwrap().complexity = Some(2);
        p.tasks.get_mut(&ids[2]).unwrap().complexity = Some(1);
        p.add_dependency(&ids[0], &ids[1]).unwrap(); // A->B
        p.add_dependency(&ids[1], &ids[2]).unwrap(); // B->C
        let (path, total) = p.critical_path();
        let names: Vec<&str> = path.iter().map(TaskId::as_str).collect();
        assert_eq!(names, vec!["C", "B", "A"]);
        assert_eq!(total, 6);
    }

    #[test]
    fn critical_path_parallel_branches() {
        let (mut p, ids) =
            project_with_tasks(&["END", "LONG", "SHORT", "START"]);
        // END depends on LONG and SHORT.
        // LONG depends on START (weight 5).
        // SHORT depends on START (weight 1).
        p.tasks.get_mut(&ids[0]).unwrap().complexity = Some(1); // END
        p.tasks.get_mut(&ids[1]).unwrap().complexity = Some(5); // LONG
        p.tasks.get_mut(&ids[2]).unwrap().complexity = Some(1); // SHORT
        p.tasks.get_mut(&ids[3]).unwrap().complexity = Some(1); // START
        p.add_dependency(&ids[0], &ids[1]).unwrap(); // END->LONG
        p.add_dependency(&ids[0], &ids[2]).unwrap(); // END->SHORT
        p.add_dependency(&ids[1], &ids[3]).unwrap(); // LONG->START
        p.add_dependency(&ids[2], &ids[3]).unwrap(); // SHORT->START
        let (path, total) = p.critical_path();
        let names: Vec<&str> = path.iter().map(TaskId::as_str).collect();
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
        let (mut p, ids) = project_with_tasks(&["A", "B", "C"]);
        p.tasks.get_mut(&ids[0]).unwrap().complexity = Some(3);
        p.tasks.get_mut(&ids[1]).unwrap().complexity = Some(2);
        p.tasks.get_mut(&ids[2]).unwrap().complexity = Some(1);
        p.add_dependency(&ids[0], &ids[1]).unwrap();
        p.add_dependency(&ids[1], &ids[2]).unwrap();
        let crit = p.critical_path_set();
        assert!(crit.contains(&ids[0]));
        assert!(crit.contains(&ids[1]));
        assert!(crit.contains(&ids[2]));
    }
}

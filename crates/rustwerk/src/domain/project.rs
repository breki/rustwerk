use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::error::DomainError;
use super::task::{
    Effort, EffortEntry, Status, Task, TaskId,
};

/// Project metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    /// Project name.
    pub name: String,
    /// Optional description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// When the project was created.
    pub created_at: DateTime<Utc>,
    /// When the project was last modified.
    pub modified_at: DateTime<Utc>,
}

/// The root aggregate — a project with its tasks.
///
/// ## File format
///
/// Persisted as `.rustwerk/project.json`:
///
/// ```json
/// {
///   "metadata": {
///     "name": "my-project",
///     "description": "optional",
///     "created_at": "2026-04-02T10:00:00Z",
///     "modified_at": "2026-04-02T10:00:00Z"
///   },
///   "tasks": {
///     "AUTH-LOGIN": {
///       "title": "Implement login",
///       "status": "todo",
///       "dependencies": ["AUTH-DB"],
///       "effort_estimate": "5H",
///       "complexity": 5,
///       "assignee": "alice",
///       "effort_entries": []
///     }
///   },
///   "next_auto_id": 1
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Project metadata.
    pub metadata: ProjectMetadata,
    /// Tasks keyed by their ID.
    #[serde(default)]
    pub tasks: BTreeMap<TaskId, Task>,
    /// Counter for auto-generated task IDs.
    #[serde(default = "default_auto_id")]
    pub next_auto_id: u32,
}

/// Default starting value for auto-generated IDs.
const fn default_auto_id() -> u32 {
    1
}

impl Project {
    /// Create a new empty project with the given name.
    pub fn new(name: &str) -> Result<Self, DomainError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(DomainError::ValidationError(
                "project name must not be empty".into(),
            ));
        }
        let now = Utc::now();
        Ok(Self {
            metadata: ProjectMetadata {
                name: name.to_string(),
                description: None,
                created_at: now,
                modified_at: now,
            },
            tasks: BTreeMap::new(),
            next_auto_id: default_auto_id(),
        })
    }

    /// Return the number of tasks in the project.
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Add a task with a user-supplied ID.
    pub fn add_task(
        &mut self,
        id: TaskId,
        task: Task,
    ) -> Result<(), DomainError> {
        if self.tasks.contains_key(&id) {
            return Err(DomainError::DuplicateTaskId(
                id.to_string(),
            ));
        }
        self.tasks.insert(id, task);
        self.metadata.modified_at = Utc::now();
        Ok(())
    }

    /// Add a task with an auto-generated ID (T1, T2, ...).
    /// Skips IDs that already exist. Returns the generated
    /// `TaskId`.
    pub fn add_task_auto(
        &mut self,
        task: Task,
    ) -> TaskId {
        loop {
            let id = TaskId::auto(self.next_auto_id);
            self.next_auto_id += 1;
            if !self.tasks.contains_key(&id) {
                self.tasks.insert(id.clone(), task);
                self.metadata.modified_at = Utc::now();
                return id;
            }
        }
    }

    /// Remove a task by ID. Fails if other tasks depend
    /// on it.
    pub fn remove_task(
        &mut self,
        id: &TaskId,
    ) -> Result<Task, DomainError> {
        if !self.tasks.contains_key(id) {
            return Err(DomainError::TaskNotFound(
                id.to_string(),
            ));
        }
        // Check if any other task depends on this one.
        for (other_id, other_task) in &self.tasks {
            if other_id != id
                && other_task.dependencies.contains(id)
            {
                return Err(DomainError::ValidationError(
                    format!(
                        "cannot remove {id}: {other_id} \
                         depends on it"
                    ),
                ));
            }
        }
        let task = self.tasks.remove(id).unwrap();
        self.metadata.modified_at = Utc::now();
        Ok(task)
    }

    /// Update a task's title and/or description.
    pub fn update_task(
        &mut self,
        id: &TaskId,
        title: Option<&str>,
        description: Option<Option<&str>>,
    ) -> Result<(), DomainError> {
        let task =
            self.tasks.get_mut(id).ok_or_else(|| {
                DomainError::TaskNotFound(id.to_string())
            })?;
        if let Some(t) = title {
            let t = t.trim();
            if t.is_empty() {
                return Err(DomainError::ValidationError(
                    "task title must not be empty".into(),
                ));
            }
            task.title = t.to_string();
        }
        if let Some(d) = description {
            task.description = d.map(String::from);
        }
        self.metadata.modified_at = Utc::now();
        Ok(())
    }

    /// Assign a developer to a task.
    pub fn assign(
        &mut self,
        id: &TaskId,
        assignee: &str,
    ) -> Result<(), DomainError> {
        let assignee = assignee.trim();
        if assignee.is_empty() {
            return Err(DomainError::ValidationError(
                "assignee must not be empty".into(),
            ));
        }
        let task =
            self.tasks.get_mut(id).ok_or_else(|| {
                DomainError::TaskNotFound(id.to_string())
            })?;
        task.assignee = Some(assignee.to_string());
        self.metadata.modified_at = Utc::now();
        Ok(())
    }

    /// Remove the assignee from a task.
    pub fn unassign(
        &mut self,
        id: &TaskId,
    ) -> Result<(), DomainError> {
        let task =
            self.tasks.get_mut(id).ok_or_else(|| {
                DomainError::TaskNotFound(id.to_string())
            })?;
        task.assignee = None;
        self.metadata.modified_at = Utc::now();
        Ok(())
    }

    /// Log effort on a task. Task must be IN_PROGRESS.
    pub fn log_effort(
        &mut self,
        id: &TaskId,
        entry: EffortEntry,
    ) -> Result<(), DomainError> {
        let task =
            self.tasks.get_mut(id).ok_or_else(|| {
                DomainError::TaskNotFound(id.to_string())
            })?;
        if task.status != Status::InProgress {
            return Err(DomainError::ValidationError(
                format!(
                    "can only log effort on IN_PROGRESS \
                     tasks (current: {})",
                    task.status
                ),
            ));
        }
        task.effort_entries.push(entry);
        self.metadata.modified_at = Utc::now();
        Ok(())
    }

    /// Set the effort estimate on a task.
    pub fn set_effort_estimate(
        &mut self,
        id: &TaskId,
        effort: Effort,
    ) -> Result<(), DomainError> {
        let task =
            self.tasks.get_mut(id).ok_or_else(|| {
                DomainError::TaskNotFound(id.to_string())
            })?;
        task.effort_estimate = Some(effort);
        self.metadata.modified_at = Utc::now();
        Ok(())
    }

    /// Set the status of a task, enforcing valid
    /// transitions unless `force` is true.
    pub fn set_status(
        &mut self,
        id: &TaskId,
        new_status: Status,
        force: bool,
    ) -> Result<(), DomainError> {
        let task = self.tasks.get_mut(id).ok_or_else(|| {
            DomainError::TaskNotFound(id.to_string())
        })?;
        let old = task.status;
        if old == new_status {
            return Ok(());
        }
        if !force && !old.can_transition_to(new_status) {
            return Err(DomainError::InvalidTransition {
                from: old.to_string(),
                to: new_status.to_string(),
            });
        }
        task.status = new_status;
        self.metadata.modified_at = Utc::now();
        Ok(())
    }

    /// Add a dependency: `from` depends on `to`.
    /// Rejects self-dependencies, unknown task IDs, duplicate
    /// edges, and cycles.
    pub fn add_dependency(
        &mut self,
        from: &TaskId,
        to: &TaskId,
    ) -> Result<(), DomainError> {
        if from == to {
            return Err(DomainError::CycleDetected(format!(
                "{from} -> {from}"
            )));
        }
        if !self.tasks.contains_key(from) {
            return Err(DomainError::TaskNotFound(
                from.to_string(),
            ));
        }
        if !self.tasks.contains_key(to) {
            return Err(DomainError::TaskNotFound(
                to.to_string(),
            ));
        }

        // Check for duplicate edge.
        let from_task = &self.tasks[from];
        if from_task.dependencies.contains(to) {
            return Ok(()); // idempotent
        }

        // Temporarily add the edge, then check for cycles.
        self.tasks
            .get_mut(from)
            .unwrap()
            .dependencies
            .push(to.clone());

        if self.has_cycle(from) {
            // Roll back.
            self.tasks
                .get_mut(from)
                .unwrap()
                .dependencies
                .pop();
            return Err(DomainError::CycleDetected(format!(
                "{from} -> {to}"
            )));
        }

        self.metadata.modified_at = Utc::now();
        Ok(())
    }

    /// Remove a dependency: `from` no longer depends on `to`.
    pub fn remove_dependency(
        &mut self,
        from: &TaskId,
        to: &TaskId,
    ) -> Result<(), DomainError> {
        let task =
            self.tasks.get_mut(from).ok_or_else(|| {
                DomainError::TaskNotFound(from.to_string())
            })?;
        let before = task.dependencies.len();
        task.dependencies.retain(|d| d != to);
        if task.dependencies.len() == before {
            return Err(DomainError::ValidationError(
                format!(
                    "{from} does not depend on {to}"
                ),
            ));
        }
        self.metadata.modified_at = Utc::now();
        Ok(())
    }

    /// DFS cycle detection starting from `start`.
    fn has_cycle(&self, start: &TaskId) -> bool {
        use std::collections::HashSet;
        let mut visited = HashSet::new();
        let mut stack = vec![start.clone()];
        while let Some(current) = stack.pop() {
            if !visited.insert(current.clone()) {
                continue;
            }
            if let Some(task) = self.tasks.get(&current) {
                for dep in &task.dependencies {
                    if dep == start {
                        return true;
                    }
                    stack.push(dep.clone());
                }
            }
        }
        false
    }

    /// Topological sort of all tasks (Kahn's algorithm).
    /// Returns task IDs in dependency order — a task
    /// appears after all its dependencies.
    pub fn topological_sort(&self) -> Vec<TaskId> {
        use std::collections::HashMap;

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
        use std::collections::HashMap;

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

    /// Return the set of task IDs on the critical path.
    pub fn critical_path_set(
        &self,
    ) -> std::collections::HashSet<TaskId> {
        let (path, _) = self.critical_path();
        path.into_iter().collect()
    }

    /// Return task IDs whose dependencies are all done.
    pub fn available_tasks(&self) -> Vec<&TaskId> {
        self.tasks
            .iter()
            .filter(|(_, task)| {
                task.status != Status::Done
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

    #[test]
    fn create_project_with_name() {
        let p = Project::new("My Project").unwrap();
        assert_eq!(p.metadata.name, "My Project");
        assert_eq!(p.task_count(), 0);
        assert_eq!(p.next_auto_id, 1);
    }

    #[test]
    fn create_project_trims_whitespace() {
        let p = Project::new("  hello  ").unwrap();
        assert_eq!(p.metadata.name, "hello");
    }

    #[test]
    fn create_project_empty_name_rejected() {
        assert!(Project::new("").is_err());
        assert!(Project::new("   ").is_err());
    }

    #[test]
    fn project_timestamps_are_populated() {
        let p = Project::new("Test").unwrap();
        let now = Utc::now();
        // Timestamps should be within the last second.
        let diff = now - p.metadata.created_at;
        assert!(diff.num_seconds() < 2);
        assert_eq!(
            p.metadata.created_at,
            p.metadata.modified_at
        );
    }

    #[test]
    fn project_description_defaults_to_none() {
        let p = Project::new("Test").unwrap();
        assert!(p.metadata.description.is_none());
    }

    #[test]
    fn add_task_with_id() {
        let mut p = Project::new("Test").unwrap();
        let task = Task::new("Login").unwrap();
        let id = TaskId::new("AUTH").unwrap();
        p.add_task(id.clone(), task).unwrap();
        assert_eq!(p.task_count(), 1);
        assert!(p.tasks.contains_key(&id));
    }

    #[test]
    fn add_task_duplicate_id_rejected() {
        let mut p = Project::new("Test").unwrap();
        let id = TaskId::new("AUTH").unwrap();
        p.add_task(id.clone(), Task::new("A").unwrap())
            .unwrap();
        let result =
            p.add_task(id, Task::new("B").unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn add_task_auto_id() {
        let mut p = Project::new("Test").unwrap();
        let id1 =
            p.add_task_auto(Task::new("A").unwrap());
        let id2 =
            p.add_task_auto(Task::new("B").unwrap());
        assert_eq!(id1.as_str(), "T0001");
        assert_eq!(id2.as_str(), "T0002");
        assert_eq!(p.next_auto_id, 3);
    }

    #[test]
    fn remove_task() {
        let (mut p, ids) = project_with_tasks(&["A", "B"]);
        let removed = p.remove_task(&ids[0]).unwrap();
        assert_eq!(removed.title, "Task A");
        assert_eq!(p.task_count(), 1);
    }

    #[test]
    fn remove_task_nonexistent_errors() {
        let (mut p, _) = project_with_tasks(&["A"]);
        let id = TaskId::new("NOPE").unwrap();
        assert!(p.remove_task(&id).is_err());
    }

    #[test]
    fn remove_task_with_dependents_errors() {
        let (mut p, ids) = project_with_tasks(&["A", "B"]);
        p.add_dependency(&ids[1], &ids[0]).unwrap(); // B->A
        let result = p.remove_task(&ids[0]); // A has dependent B
        assert!(result.is_err());
    }

    #[test]
    fn remove_task_after_removing_dependency() {
        let (mut p, ids) = project_with_tasks(&["A", "B"]);
        p.add_dependency(&ids[1], &ids[0]).unwrap(); // B->A
        p.remove_dependency(&ids[1], &ids[0]).unwrap();
        p.remove_task(&ids[0]).unwrap(); // now OK
        assert_eq!(p.task_count(), 1);
    }

    #[test]
    fn update_task_title() {
        let (mut p, ids) = project_with_tasks(&["A"]);
        p.update_task(&ids[0], Some("New title"), None)
            .unwrap();
        assert_eq!(
            p.tasks.get(&ids[0]).unwrap().title,
            "New title"
        );
    }

    #[test]
    fn update_task_description() {
        let (mut p, ids) = project_with_tasks(&["A"]);
        p.update_task(
            &ids[0],
            None,
            Some(Some("A description")),
        )
        .unwrap();
        assert_eq!(
            p.tasks
                .get(&ids[0])
                .unwrap()
                .description
                .as_deref(),
            Some("A description")
        );
    }

    #[test]
    fn update_task_clear_description() {
        let (mut p, ids) = project_with_tasks(&["A"]);
        p.update_task(
            &ids[0],
            None,
            Some(Some("desc")),
        )
        .unwrap();
        p.update_task(&ids[0], None, Some(None)).unwrap();
        assert!(
            p.tasks
                .get(&ids[0])
                .unwrap()
                .description
                .is_none()
        );
    }

    #[test]
    fn update_task_empty_title_rejected() {
        let (mut p, ids) = project_with_tasks(&["A"]);
        assert!(
            p.update_task(&ids[0], Some(""), None).is_err()
        );
    }

    #[test]
    fn update_task_nonexistent_errors() {
        let (mut p, _) = project_with_tasks(&["A"]);
        let id = TaskId::new("NOPE").unwrap();
        assert!(
            p.update_task(&id, Some("X"), None).is_err()
        );
    }

    #[test]
    fn assign_task() {
        let (mut p, ids) = project_with_tasks(&["A"]);
        p.assign(&ids[0], "alice").unwrap();
        assert_eq!(
            p.tasks.get(&ids[0]).unwrap().assignee.as_deref(),
            Some("alice")
        );
    }

    #[test]
    fn assign_trims_whitespace() {
        let (mut p, ids) = project_with_tasks(&["A"]);
        p.assign(&ids[0], "  bob  ").unwrap();
        assert_eq!(
            p.tasks.get(&ids[0]).unwrap().assignee.as_deref(),
            Some("bob")
        );
    }

    #[test]
    fn assign_empty_rejected() {
        let (mut p, ids) = project_with_tasks(&["A"]);
        assert!(p.assign(&ids[0], "").is_err());
        assert!(p.assign(&ids[0], "   ").is_err());
    }

    #[test]
    fn assign_nonexistent_task_errors() {
        let (mut p, _) = project_with_tasks(&["A"]);
        let id = TaskId::new("NOPE").unwrap();
        assert!(p.assign(&id, "alice").is_err());
    }

    #[test]
    fn unassign_task() {
        let (mut p, ids) = project_with_tasks(&["A"]);
        p.assign(&ids[0], "alice").unwrap();
        p.unassign(&ids[0]).unwrap();
        assert!(
            p.tasks.get(&ids[0]).unwrap().assignee.is_none()
        );
    }

    #[test]
    fn unassign_nonexistent_task_errors() {
        let (mut p, _) = project_with_tasks(&["A"]);
        let id = TaskId::new("NOPE").unwrap();
        assert!(p.unassign(&id).is_err());
    }

    #[test]
    fn log_effort_on_in_progress_task() {
        let (mut p, ids) = project_with_tasks(&["A"]);
        p.set_status(&ids[0], Status::InProgress, false)
            .unwrap();
        let entry = EffortEntry {
            effort: Effort::parse("2.5H").unwrap(),
            developer: "alice".into(),
            timestamp: Utc::now(),
            note: Some("initial work".into()),
        };
        p.log_effort(&ids[0], entry).unwrap();
        let task = p.tasks.get(&ids[0]).unwrap();
        assert_eq!(task.effort_entries.len(), 1);
        assert!(
            (task.total_actual_effort_hours() - 2.5).abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn log_effort_requires_in_progress() {
        let (mut p, ids) = project_with_tasks(&["A"]);
        let entry = EffortEntry {
            effort: Effort::parse("1H").unwrap(),
            developer: "alice".into(),
            timestamp: Utc::now(),
            note: None,
        };
        // Task is TODO — should fail.
        assert!(p.log_effort(&ids[0], entry).is_err());
    }

    #[test]
    fn log_effort_nonexistent_task_errors() {
        let (mut p, _) = project_with_tasks(&["A"]);
        let id = TaskId::new("NOPE").unwrap();
        let entry = EffortEntry {
            effort: Effort::parse("1H").unwrap(),
            developer: "alice".into(),
            timestamp: Utc::now(),
            note: None,
        };
        assert!(p.log_effort(&id, entry).is_err());
    }

    #[test]
    fn set_effort_estimate() {
        let (mut p, ids) = project_with_tasks(&["A"]);
        let effort = Effort::parse("8H").unwrap();
        p.set_effort_estimate(&ids[0], effort).unwrap();
        assert_eq!(
            p.tasks
                .get(&ids[0])
                .unwrap()
                .effort_estimate
                .as_ref()
                .unwrap()
                .to_string(),
            "8H"
        );
    }

    #[test]
    fn total_effort_sums_entries() {
        let (mut p, ids) = project_with_tasks(&["A"]);
        p.set_status(&ids[0], Status::InProgress, false)
            .unwrap();
        for hours in &["2H", "3.5H", "1D"] {
            let entry = EffortEntry {
                effort: Effort::parse(hours).unwrap(),
                developer: "alice".into(),
                timestamp: Utc::now(),
                note: None,
            };
            p.log_effort(&ids[0], entry).unwrap();
        }
        let task = p.tasks.get(&ids[0]).unwrap();
        // 2 + 3.5 + 8 (1D) = 13.5H
        assert!(
            (task.total_actual_effort_hours() - 13.5).abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn set_status_valid_transition() {
        let mut p = Project::new("Test").unwrap();
        let id = TaskId::new("T").unwrap();
        p.add_task(id.clone(), Task::new("X").unwrap())
            .unwrap();
        p.set_status(&id, Status::InProgress, false).unwrap();
        assert_eq!(
            p.tasks.get(&id).unwrap().status,
            Status::InProgress
        );
    }

    #[test]
    fn set_status_invalid_transition_rejected() {
        let mut p = Project::new("Test").unwrap();
        let id = TaskId::new("T").unwrap();
        p.add_task(id.clone(), Task::new("X").unwrap())
            .unwrap();
        let result = p.set_status(&id, Status::Done, false);
        assert!(result.is_err());
    }

    #[test]
    fn set_status_same_status_is_noop() {
        let mut p = Project::new("Test").unwrap();
        let id = TaskId::new("T").unwrap();
        p.add_task(id.clone(), Task::new("X").unwrap())
            .unwrap();
        p.set_status(&id, Status::Todo, false).unwrap();
    }

    #[test]
    fn set_status_nonexistent_task_errors() {
        let mut p = Project::new("Test").unwrap();
        let id = TaskId::new("NOPE").unwrap();
        let result =
            p.set_status(&id, Status::InProgress, false);
        assert!(result.is_err());
    }

    #[test]
    fn set_status_force_bypasses_validation() {
        let mut p = Project::new("Test").unwrap();
        let id = TaskId::new("T").unwrap();
        p.add_task(id.clone(), Task::new("X").unwrap())
            .unwrap();
        p.set_status(&id, Status::InProgress, false)
            .unwrap();
        p.set_status(&id, Status::Done, false).unwrap();
        // DONE -> TODO is normally invalid.
        p.set_status(&id, Status::Todo, true).unwrap();
        assert_eq!(
            p.tasks.get(&id).unwrap().status,
            Status::Todo
        );
    }

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
    fn add_dependency() {
        let (mut p, ids) = project_with_tasks(&["A", "B"]);
        p.add_dependency(&ids[1], &ids[0]).unwrap();
        let task_b = p.tasks.get(&ids[1]).unwrap();
        assert_eq!(task_b.dependencies.len(), 1);
        assert_eq!(task_b.dependencies[0].as_str(), "A");
    }

    #[test]
    fn add_dependency_idempotent() {
        let (mut p, ids) = project_with_tasks(&["A", "B"]);
        p.add_dependency(&ids[1], &ids[0]).unwrap();
        p.add_dependency(&ids[1], &ids[0]).unwrap();
        let task_b = p.tasks.get(&ids[1]).unwrap();
        assert_eq!(task_b.dependencies.len(), 1);
    }

    #[test]
    fn add_self_dependency_rejected() {
        let (mut p, ids) = project_with_tasks(&["A"]);
        let result = p.add_dependency(&ids[0], &ids[0]);
        assert!(result.is_err());
    }

    #[test]
    fn add_dependency_unknown_task_rejected() {
        let (mut p, ids) = project_with_tasks(&["A"]);
        let unknown = TaskId::new("NOPE").unwrap();
        assert!(
            p.add_dependency(&ids[0], &unknown).is_err()
        );
        assert!(
            p.add_dependency(&unknown, &ids[0]).is_err()
        );
    }

    #[test]
    fn direct_cycle_detected() {
        let (mut p, ids) =
            project_with_tasks(&["A", "B"]);
        p.add_dependency(&ids[0], &ids[1]).unwrap();
        let result = p.add_dependency(&ids[1], &ids[0]);
        assert!(result.is_err());
        // Edge should not have been added.
        assert!(
            p.tasks
                .get(&ids[1])
                .unwrap()
                .dependencies
                .is_empty()
        );
    }

    #[test]
    fn indirect_cycle_detected() {
        let (mut p, ids) =
            project_with_tasks(&["A", "B", "C"]);
        p.add_dependency(&ids[0], &ids[1]).unwrap(); // A->B
        p.add_dependency(&ids[1], &ids[2]).unwrap(); // B->C
        let result =
            p.add_dependency(&ids[2], &ids[0]); // C->A
        assert!(result.is_err());
    }

    #[test]
    fn valid_dag_no_cycle() {
        let (mut p, ids) =
            project_with_tasks(&["A", "B", "C"]);
        p.add_dependency(&ids[0], &ids[1]).unwrap(); // A->B
        p.add_dependency(&ids[0], &ids[2]).unwrap(); // A->C
        p.add_dependency(&ids[1], &ids[2]).unwrap(); // B->C
        // Diamond DAG, no cycle.
        assert_eq!(
            p.tasks
                .get(&ids[0])
                .unwrap()
                .dependencies
                .len(),
            2
        );
    }

    #[test]
    fn remove_dependency() {
        let (mut p, ids) = project_with_tasks(&["A", "B"]);
        p.add_dependency(&ids[1], &ids[0]).unwrap();
        p.remove_dependency(&ids[1], &ids[0]).unwrap();
        assert!(
            p.tasks
                .get(&ids[1])
                .unwrap()
                .dependencies
                .is_empty()
        );
    }

    #[test]
    fn remove_nonexistent_dependency_errors() {
        let (mut p, ids) = project_with_tasks(&["A", "B"]);
        assert!(
            p.remove_dependency(&ids[0], &ids[1]).is_err()
        );
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
}

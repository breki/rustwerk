use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::error::DomainError;
use super::task::{Status, Task, TaskId};

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

    /// Set the status of a task, enforcing valid
    /// transitions.
    pub fn set_status(
        &mut self,
        id: &TaskId,
        new_status: Status,
    ) -> Result<(), DomainError> {
        let task = self.tasks.get_mut(id).ok_or_else(|| {
            DomainError::TaskNotFound(id.to_string())
        })?;
        let old = task.status;
        if old == new_status {
            return Ok(());
        }
        if !old.can_transition_to(new_status) {
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
    fn set_status_valid_transition() {
        let mut p = Project::new("Test").unwrap();
        let id = TaskId::new("T").unwrap();
        p.add_task(id.clone(), Task::new("X").unwrap())
            .unwrap();
        p.set_status(&id, Status::InProgress).unwrap();
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
        let result = p.set_status(&id, Status::Done);
        assert!(result.is_err());
    }

    #[test]
    fn set_status_same_status_is_noop() {
        let mut p = Project::new("Test").unwrap();
        let id = TaskId::new("T").unwrap();
        p.add_task(id.clone(), Task::new("X").unwrap())
            .unwrap();
        p.set_status(&id, Status::Todo).unwrap();
    }

    #[test]
    fn set_status_nonexistent_task_errors() {
        let mut p = Project::new("Test").unwrap();
        let id = TaskId::new("NOPE").unwrap();
        let result =
            p.set_status(&id, Status::InProgress);
        assert!(result.is_err());
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
        p.set_status(&ids[0], Status::InProgress)
            .unwrap();
        p.set_status(&ids[0], Status::Done).unwrap();
        // Now B is available.
        let avail = p.available_tasks();
        assert_eq!(avail.len(), 1);
        assert_eq!(avail[0].as_str(), "B");
    }

    #[test]
    fn available_tasks_excludes_done() {
        let (mut p, ids) = project_with_tasks(&["A"]);
        p.set_status(&ids[0], Status::InProgress)
            .unwrap();
        p.set_status(&ids[0], Status::Done).unwrap();
        let avail = p.available_tasks();
        assert!(avail.is_empty());
    }
}

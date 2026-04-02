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
}

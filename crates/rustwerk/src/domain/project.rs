use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::error::DomainError;
use super::task::{Task, TaskId};

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
}

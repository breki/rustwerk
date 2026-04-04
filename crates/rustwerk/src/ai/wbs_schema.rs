//! WBS import/export schema.
//!
//! Defines the JSON format that AI agents use to bulk-create
//! tasks with dependencies. The format is an array of task
//! entries, each with an ID and optional fields matching the
//! internal project structure.
//!
//! ## Example
//!
//! ```json
//! [
//!   {
//!     "id": "AUTH-LOGIN",
//!     "title": "Implement login",
//!     "description": "OAuth2 flow with JWT tokens",
//!     "dependencies": ["AUTH-DB"],
//!     "complexity": 5,
//!     "effort_estimate": "8H",
//!     "assignee": "alice"
//!   },
//!   {
//!     "id": "AUTH-DB",
//!     "title": "Set up auth database",
//!     "complexity": 3
//!   }
//! ]
//! ```

use serde::{Deserialize, Serialize};

use crate::domain::error::DomainError;
use crate::domain::project::Project;
use crate::domain::task::{Effort, Task, TaskId};

/// A single task entry in the WBS import format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WbsTaskEntry {
    /// Mnemonic task ID.
    pub id: String,
    /// Human-readable title.
    pub title: String,
    /// Optional description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// IDs of tasks this task depends on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
    /// Complexity score (e.g. Fibonacci).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complexity: Option<u32>,
    /// Effort estimate (e.g. "8H", "2D").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort_estimate: Option<String>,
    /// Assigned developer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
}

/// Parse a WBS JSON string into task entries.
pub fn parse_wbs(json: &str) -> Result<Vec<WbsTaskEntry>, serde_json::Error> {
    serde_json::from_str(json)
}

/// Serialize task entries to a WBS JSON string.
pub fn serialize_wbs(
    entries: &[WbsTaskEntry],
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(entries)
}

/// Export a project's tasks as WBS entries.
pub fn export_from_project(project: &Project) -> Vec<WbsTaskEntry> {
    project
        .tasks
        .iter()
        .map(|(id, task)| WbsTaskEntry {
            id: id.as_str().to_string(),
            title: task.title.clone(),
            description: task.description.clone(),
            dependencies: task
                .dependencies
                .iter()
                .map(|d| d.as_str().to_string())
                .collect(),
            complexity: task.complexity,
            effort_estimate: task
                .effort_estimate
                .as_ref()
                .map(|e| e.to_string()),
            assignee: task.assignee.clone(),
        })
        .collect()
}

/// Maximum number of entries in a WBS import.
const MAX_WBS_ENTRIES: usize = 10_000;

/// Import WBS entries into a project. Creates tasks first,
/// then adds dependencies. Returns the number of tasks
/// created.
///
/// For existing task IDs: fails if the entry's dependencies
/// differ from those already stored.
///
/// Atomic: on any error, the project is left unmodified.
pub fn import_into_project(
    project: &mut Project,
    entries: &[WbsTaskEntry],
) -> Result<usize, DomainError> {
    if entries.len() > MAX_WBS_ENTRIES {
        return Err(DomainError::ValidationError(format!(
            "WBS import contains {} entries \
                 (max {MAX_WBS_ENTRIES})",
            entries.len()
        )));
    }

    // Clone the project for atomicity — restore on error.
    let snapshot = project.clone();

    match import_inner(project, entries) {
        Ok(created) => Ok(created),
        Err(e) => {
            *project = snapshot;
            Err(e)
        }
    }
}

/// Inner import logic (not atomic on its own).
fn import_inner(
    project: &mut Project,
    entries: &[WbsTaskEntry],
) -> Result<usize, DomainError> {
    let mut created = 0;

    // First pass: create new tasks, validate existing.
    for entry in entries {
        let id = TaskId::new(&entry.id)?;
        if let Some(existing) = project.tasks.get(&id) {
            // Existing task: verify dependencies match.
            let expected_deps: Vec<TaskId> = entry
                .dependencies
                .iter()
                .map(|d| TaskId::new(d))
                .collect::<Result<Vec<_>, _>>()?;
            if existing.dependencies != expected_deps {
                return Err(DomainError::ValidationError(format!(
                    "task {id} already exists with \
                         different dependencies"
                )));
            }
            continue;
        }
        let mut task = Task::new(&entry.title)?;
        task.description = entry.description.clone();
        if let Some(c) = entry.complexity {
            task.set_complexity(c)?;
        }
        if let Some(e) = &entry.effort_estimate {
            task.effort_estimate = Some(Effort::parse(e)?);
        }
        task.assignee = entry.assignee.clone();
        project.add_task(id, task)?;
        created += 1;
    }

    // Second pass: add dependencies for new tasks only.
    for entry in entries {
        let from = TaskId::new(&entry.id)?;
        for dep_str in &entry.dependencies {
            let to = TaskId::new(dep_str)?;
            if project.tasks.contains_key(&from)
                && project.tasks.contains_key(&to)
            {
                // add_dependency is idempotent for
                // duplicate edges.
                project.add_dependency(&from, &to)?;
            }
        }
    }

    Ok(created)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_wbs_json() -> &'static str {
        r#"[
            {
                "id": "AUTH-DB",
                "title": "Set up auth database",
                "complexity": 3
            },
            {
                "id": "AUTH-LOGIN",
                "title": "Implement login",
                "description": "OAuth2 flow",
                "dependencies": ["AUTH-DB"],
                "complexity": 5,
                "effort_estimate": "8H",
                "assignee": "alice"
            }
        ]"#
    }

    #[test]
    fn parse_wbs_valid() {
        let entries = parse_wbs(sample_wbs_json()).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "AUTH-DB");
        assert_eq!(entries[1].id, "AUTH-LOGIN");
        assert_eq!(entries[1].dependencies, vec!["AUTH-DB"]);
        assert_eq!(entries[1].complexity, Some(5));
        assert_eq!(entries[1].effort_estimate.as_deref(), Some("8H"));
        assert_eq!(entries[1].assignee.as_deref(), Some("alice"));
    }

    #[test]
    fn parse_wbs_minimal() {
        let json = r#"[{"id": "X", "title": "Minimal"}]"#;
        let entries = parse_wbs(json).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].dependencies.is_empty());
        assert!(entries[0].complexity.is_none());
    }

    #[test]
    fn parse_wbs_invalid_json() {
        assert!(parse_wbs("not json").is_err());
    }

    #[test]
    fn serialize_and_parse_round_trip() {
        let entries = parse_wbs(sample_wbs_json()).unwrap();
        let json = serialize_wbs(&entries).unwrap();
        let reparsed = parse_wbs(&json).unwrap();
        assert_eq!(reparsed.len(), entries.len());
        assert_eq!(reparsed[0].id, entries[0].id);
        assert_eq!(reparsed[1].id, entries[1].id);
    }

    #[test]
    fn import_into_empty_project() {
        let mut project = Project::new("Test").unwrap();
        let entries = parse_wbs(sample_wbs_json()).unwrap();
        let created = import_into_project(&mut project, &entries).unwrap();
        assert_eq!(created, 2);
        assert_eq!(project.task_count(), 2);

        // Check dependencies were added.
        let login_id = TaskId::new("AUTH-LOGIN").unwrap();
        let task = &project.tasks[&login_id];
        assert_eq!(task.dependencies.len(), 1);
    }

    #[test]
    fn import_is_idempotent() {
        let mut project = Project::new("Test").unwrap();
        let entries = parse_wbs(sample_wbs_json()).unwrap();
        import_into_project(&mut project, &entries).unwrap();
        let created = import_into_project(&mut project, &entries).unwrap();
        assert_eq!(created, 0);
        assert_eq!(project.task_count(), 2);
    }

    #[test]
    fn import_with_cycle_rejected_and_rolled_back() {
        let json = r#"[
            {"id": "A", "title": "A", "dependencies": ["B"]},
            {"id": "B", "title": "B", "dependencies": ["A"]}
        ]"#;
        let mut project = Project::new("Test").unwrap();
        let entries = parse_wbs(json).unwrap();
        let result = import_into_project(&mut project, &entries);
        assert!(result.is_err());
        // Atomicity: no tasks should remain.
        assert_eq!(
            project.task_count(),
            0,
            "import should be atomic — no orphaned tasks"
        );
    }

    #[test]
    fn import_existing_task_different_deps_fails() {
        let mut project = Project::new("Test").unwrap();
        // First import: A depends on B.
        let json1 = r#"[
            {"id": "B", "title": "B"},
            {"id": "A", "title": "A", "dependencies": ["B"]}
        ]"#;
        let entries1 = parse_wbs(json1).unwrap();
        import_into_project(&mut project, &entries1).unwrap();

        // Second import: A now depends on C (different).
        let json2 = r#"[
            {"id": "C", "title": "C"},
            {"id": "A", "title": "A", "dependencies": ["C"]}
        ]"#;
        let entries2 = parse_wbs(json2).unwrap();
        let result = import_into_project(&mut project, &entries2);
        assert!(result.is_err(), "should fail: A has different deps");
        // Atomicity: C should not have been added.
        assert_eq!(project.task_count(), 2);
    }

    #[test]
    fn import_too_many_entries_rejected() {
        let entries: Vec<WbsTaskEntry> = (0..10_001)
            .map(|i| WbsTaskEntry {
                id: format!("T{i}"),
                title: format!("Task {i}"),
                description: None,
                dependencies: Vec::new(),
                complexity: None,
                effort_estimate: None,
                assignee: None,
            })
            .collect();
        let mut project = Project::new("Test").unwrap();
        let result = import_into_project(&mut project, &entries);
        assert!(result.is_err());
    }

    #[test]
    fn export_from_project_round_trip() {
        let mut project = Project::new("Test").unwrap();
        let entries = parse_wbs(sample_wbs_json()).unwrap();
        import_into_project(&mut project, &entries).unwrap();

        let exported = export_from_project(&project);
        assert_eq!(exported.len(), 2);

        // Re-import into fresh project.
        let mut project2 = Project::new("Test2").unwrap();
        let created = import_into_project(&mut project2, &exported).unwrap();
        assert_eq!(created, 2);
    }
}

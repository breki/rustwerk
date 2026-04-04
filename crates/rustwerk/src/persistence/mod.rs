/// File-based project store.
pub mod file_store;

use crate::domain::project::Project;

/// Serialize a project to a pretty-printed JSON string.
pub fn serialize_project(
    project: &Project,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(project)
}

/// Deserialize a project from a JSON string.
pub fn deserialize_project(json: &str) -> Result<Project, serde_json::Error> {
    serde_json::from_str(json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::task::{Effort, EffortEntry, Task, TaskId};
    use chrono::Utc;

    #[test]
    fn round_trip_empty_project() {
        let project = Project::new("Test").unwrap();
        let json = serialize_project(&project).unwrap();
        let loaded = deserialize_project(&json).unwrap();
        assert_eq!(loaded.metadata.name, "Test");
        assert_eq!(loaded.task_count(), 0);
    }

    #[test]
    fn round_trip_project_with_tasks() {
        let mut project = Project::new("Test").unwrap();
        let mut task = Task::new("Implement login").unwrap();
        task.effort_estimate = Some(Effort::parse("5H").unwrap());
        task.complexity = Some(5);
        task.assignee = Some("alice".into());
        project
            .tasks
            .insert(TaskId::new("AUTH-LOGIN").unwrap(), task);

        let json = serialize_project(&project).unwrap();
        let loaded = deserialize_project(&json).unwrap();
        assert_eq!(loaded.task_count(), 1);

        let id = TaskId::new("AUTH-LOGIN").unwrap();
        let task = loaded.tasks.get(&id).unwrap();
        assert_eq!(task.title, "Implement login");
        assert_eq!(task.effort_estimate.as_ref().unwrap().to_string(), "5H");
        assert_eq!(task.complexity, Some(5));
        assert_eq!(task.assignee.as_deref(), Some("alice"));
    }

    #[test]
    fn round_trip_effort_entries() {
        let mut project = Project::new("Test").unwrap();
        let mut task = Task::new("Do work").unwrap();
        task.effort_entries.push(EffortEntry {
            effort: Effort::parse("2.5H").unwrap(),
            developer: "bob".into(),
            timestamp: Utc::now(),
            note: Some("initial work".into()),
        });
        project.tasks.insert(TaskId::new("WORK").unwrap(), task);

        let json = serialize_project(&project).unwrap();
        let loaded = deserialize_project(&json).unwrap();
        let id = TaskId::new("WORK").unwrap();
        let task = loaded.tasks.get(&id).unwrap();
        assert_eq!(task.effort_entries.len(), 1);
        assert_eq!(task.effort_entries[0].effort.to_string(), "2.5H");
        assert_eq!(task.effort_entries[0].developer, "bob");
        assert_eq!(
            task.effort_entries[0].note.as_deref(),
            Some("initial work")
        );
    }

    #[test]
    fn round_trip_dependencies() {
        let mut project = Project::new("Test").unwrap();
        project
            .tasks
            .insert(TaskId::new("A").unwrap(), Task::new("Task A").unwrap());
        let mut task_b = Task::new("Task B").unwrap();
        task_b.dependencies.push(TaskId::new("A").unwrap());
        project.tasks.insert(TaskId::new("B").unwrap(), task_b);

        let json = serialize_project(&project).unwrap();
        let loaded = deserialize_project(&json).unwrap();
        let id_b = TaskId::new("B").unwrap();
        let task = loaded.tasks.get(&id_b).unwrap();
        assert_eq!(task.dependencies.len(), 1);
        assert_eq!(task.dependencies[0].as_str(), "A");
    }

    #[test]
    fn deserialize_invalid_json_errors() {
        let result = deserialize_project("not json");
        assert!(result.is_err());
    }
}

mod bottleneck;
mod critical_path;
mod gantt_row;
mod gantt_schedule;
mod queries;
mod scheduling;
mod summary;
mod tree;
mod tree_node;

pub use bottleneck::Bottleneck;
pub use gantt_row::GanttRow;
pub use summary::ProjectSummary;
pub use tree_node::TreeNode;

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::developer::{Developer, DeveloperId};
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
    /// Developers on the project.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub developers: BTreeMap<DeveloperId, Developer>,
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
            developers: BTreeMap::new(),
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

    /// Assign a registered developer to a task.
    /// The developer must exist in the project's
    /// developer registry.
    pub fn assign(
        &mut self,
        id: &TaskId,
        developer_id: &DeveloperId,
    ) -> Result<(), DomainError> {
        if !self.developers.contains_key(developer_id) {
            return Err(DomainError::DeveloperNotFound(
                developer_id.to_string(),
            ));
        }
        let task =
            self.tasks.get_mut(id).ok_or_else(|| {
                DomainError::TaskNotFound(id.to_string())
            })?;
        task.assignee =
            Some(developer_id.as_str().to_string());
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

    /// Add a developer to the project.
    pub fn add_developer(
        &mut self,
        id: DeveloperId,
        developer: Developer,
    ) -> Result<(), DomainError> {
        if self.developers.contains_key(&id) {
            return Err(
                DomainError::DeveloperAlreadyExists(
                    id.to_string(),
                ),
            );
        }
        self.developers.insert(id, developer);
        self.metadata.modified_at = Utc::now();
        Ok(())
    }

    /// Remove a developer from the project.
    pub fn remove_developer(
        &mut self,
        id: &DeveloperId,
    ) -> Result<Developer, DomainError> {
        // Check if any task is assigned to this developer.
        for (task_id, task) in &self.tasks {
            if task.assignee.as_deref()
                == Some(id.as_str())
            {
                return Err(
                    DomainError::ValidationError(
                        format!(
                            "cannot remove developer \
                             {id}: task {task_id} is \
                             assigned to them"
                        ),
                    ),
                );
            }
        }
        let dev = self.developers.remove(id).ok_or(
            DomainError::DeveloperNotFound(
                id.to_string(),
            ),
        )?;
        self.metadata.modified_at = Utc::now();
        Ok(dev)
    }

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
}

// Scheduling methods (topological_sort, critical_path,
// etc.) and output types (GanttRow, ProjectSummary) are
// in the `scheduling` submodule.

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

    /// Helper to add a developer to a test project.
    fn add_dev(p: &mut Project, id: &str, name: &str) {
        let dev_id = DeveloperId::new(id).unwrap();
        let dev = Developer::new(name).unwrap();
        p.add_developer(dev_id, dev).unwrap();
    }

    #[test]
    fn assign_task() {
        let (mut p, ids) = project_with_tasks(&["A"]);
        add_dev(&mut p, "alice", "Alice");
        let dev =
            DeveloperId::new("alice").unwrap();
        p.assign(&ids[0], &dev).unwrap();
        assert_eq!(
            p.tasks.get(&ids[0]).unwrap().assignee.as_deref(),
            Some("alice")
        );
    }

    #[test]
    fn assign_unregistered_developer_rejected() {
        let (mut p, ids) = project_with_tasks(&["A"]);
        let dev =
            DeveloperId::new("ghost").unwrap();
        assert!(p.assign(&ids[0], &dev).is_err());
    }

    #[test]
    fn assign_nonexistent_task_errors() {
        let (mut p, _) = project_with_tasks(&["A"]);
        add_dev(&mut p, "alice", "Alice");
        let id = TaskId::new("NOPE").unwrap();
        let dev =
            DeveloperId::new("alice").unwrap();
        assert!(p.assign(&id, &dev).is_err());
    }

    #[test]
    fn unassign_task() {
        let (mut p, ids) = project_with_tasks(&["A"]);
        add_dev(&mut p, "alice", "Alice");
        let dev =
            DeveloperId::new("alice").unwrap();
        p.assign(&ids[0], &dev).unwrap();
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

    // Scheduling tests (topological sort, critical path,
    // available/active tasks, summary, gantt) are in
    // scheduling.rs.


    #[test]
    fn add_developer() {
        let mut p = Project::new("Test").unwrap();
        let id = DeveloperId::new("igor").unwrap();
        let dev = Developer::new("Igor").unwrap();
        p.add_developer(id.clone(), dev).unwrap();
        assert_eq!(p.developers.len(), 1);
        assert_eq!(
            p.developers[&id].name,
            "Igor"
        );
    }

    #[test]
    fn add_developer_duplicate_rejected() {
        let mut p = Project::new("Test").unwrap();
        let id = DeveloperId::new("igor").unwrap();
        p.add_developer(
            id.clone(),
            Developer::new("Igor").unwrap(),
        )
        .unwrap();
        let result = p.add_developer(
            id,
            Developer::new("Igor 2").unwrap(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn remove_developer() {
        let mut p = Project::new("Test").unwrap();
        let id = DeveloperId::new("igor").unwrap();
        p.add_developer(
            id.clone(),
            Developer::new("Igor").unwrap(),
        )
        .unwrap();
        let removed =
            p.remove_developer(&id).unwrap();
        assert_eq!(removed.name, "Igor");
        assert!(p.developers.is_empty());
    }

    #[test]
    fn remove_developer_not_found() {
        let mut p = Project::new("Test").unwrap();
        let id = DeveloperId::new("nope").unwrap();
        assert!(p.remove_developer(&id).is_err());
    }

    #[test]
    fn remove_developer_with_assigned_task_fails() {
        let (mut p, ids) =
            project_with_tasks(&["A"]);
        let dev_id =
            DeveloperId::new("igor").unwrap();
        p.add_developer(
            dev_id.clone(),
            Developer::new("Igor").unwrap(),
        )
        .unwrap();
        p.assign(&ids[0], &dev_id).unwrap();
        let result = p.remove_developer(&dev_id);
        assert!(result.is_err());
    }

    #[test]
    fn developer_serialization_round_trip() {
        let mut p = Project::new("Test").unwrap();
        let id = DeveloperId::new("igor").unwrap();
        let mut dev = Developer::new("Igor").unwrap();
        dev.role = Some("project-lead".into());
        dev.specialties =
            vec!["rust".into(), "cli".into()];
        p.add_developer(id.clone(), dev).unwrap();

        let json =
            serde_json::to_string_pretty(&p).unwrap();
        let loaded: Project =
            serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.developers.len(), 1);
        let d = &loaded.developers[&id];
        assert_eq!(d.name, "Igor");
        assert_eq!(
            d.role.as_deref(),
            Some("project-lead")
        );
        assert_eq!(
            d.specialties,
            vec!["rust", "cli"]
        );
    }
}

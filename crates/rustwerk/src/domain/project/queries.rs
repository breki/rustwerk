use std::collections::HashSet;

use super::Project;
use crate::domain::error::DomainError;
use crate::domain::task::{Status, TaskId};

impl Project {
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

    /// Return task IDs that match the given status.
    pub fn tasks_by_status(
        &self,
        status: Status,
    ) -> Vec<&TaskId> {
        self.tasks
            .iter()
            .filter(|(_, task)| task.status == status)
            .map(|(id, _)| id)
            .collect()
    }

    /// Return task IDs assigned to the given developer.
    pub fn tasks_by_assignee(
        &self,
        assignee: &str,
    ) -> Vec<&TaskId> {
        self.tasks
            .iter()
            .filter(|(_, task)| {
                task.assignee.as_deref() == Some(assignee)
            })
            .map(|(id, _)| id)
            .collect()
    }

    /// Return the dependency chain for a task: the task
    /// itself plus all its transitive dependencies, in
    /// dependency order (dependencies first).
    ///
    /// Returns an error if the task does not exist.
    pub fn dependency_chain<'a>(
        &'a self,
        id: &'a TaskId,
    ) -> Result<Vec<&'a TaskId>, DomainError> {
        if !self.tasks.contains_key(id) {
            return Err(DomainError::TaskNotFound(
                id.to_string(),
            ));
        }
        let mut visited = HashSet::new();
        let mut stack = vec![id];
        while let Some(current) = stack.pop() {
            if !visited.insert(current) {
                continue;
            }
            if let Some(task) = self.tasks.get(current) {
                for dep in &task.dependencies {
                    if !visited.contains(dep) {
                        stack.push(dep);
                    }
                }
            }
        }
        // Order by subgraph DFS post-order (dependencies
        // before dependents) without a full-graph topo sort.
        let mut ordered = Vec::new();
        let mut emitted = HashSet::new();
        self.dfs_postorder(
            id,
            &mut emitted,
            &mut ordered,
        );
        Ok(ordered)
    }

    /// DFS post-order traversal: emits each node after all
    /// its dependencies, producing topological order for the
    /// reachable subgraph.
    fn dfs_postorder<'a>(
        &'a self,
        id: &'a TaskId,
        emitted: &mut HashSet<&'a TaskId>,
        result: &mut Vec<&'a TaskId>,
    ) {
        if emitted.contains(id) {
            return;
        }
        if let Some(task) = self.tasks.get(id) {
            for dep in &task.dependencies {
                self.dfs_postorder(dep, emitted, result);
            }
        }
        if let Some((k, _)) = self.tasks.get_key_value(id)
        {
            emitted.insert(k);
            result.push(k);
        }
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
        // A has no deps -> not blocked.
        // B depends on A (todo) -> blocked.
        // C depends on B (todo) -> blocked.
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
    fn tasks_by_status_filters_correctly() {
        let (mut p, ids) =
            project_with_tasks(&["A", "B", "C"]);
        p.set_status(&ids[0], Status::InProgress, false)
            .unwrap();
        let todo = p.tasks_by_status(Status::Todo);
        assert_eq!(todo.len(), 2);
        let ip = p.tasks_by_status(Status::InProgress);
        assert_eq!(ip.len(), 1);
        assert_eq!(ip[0].as_str(), "A");
    }

    #[test]
    fn tasks_by_assignee_filters_correctly() {
        let (mut p, _) =
            project_with_tasks(&["A", "B", "C"]);
        p.tasks
            .get_mut(&TaskId::new("A").unwrap())
            .unwrap()
            .assignee = Some("alice".into());
        p.tasks
            .get_mut(&TaskId::new("B").unwrap())
            .unwrap()
            .assignee = Some("bob".into());
        let alice =
            p.tasks_by_assignee("alice");
        assert_eq!(alice.len(), 1);
        assert_eq!(alice[0].as_str(), "A");
        let none = p.tasks_by_assignee("carol");
        assert!(none.is_empty());
    }

    #[test]
    fn dependency_chain_returns_transitive_deps() {
        let (mut p, ids) =
            project_with_tasks(&["R", "M", "L", "O"]);
        p.add_dependency(&ids[1], &ids[0]).unwrap(); // M->R
        p.add_dependency(&ids[2], &ids[1]).unwrap(); // L->M
        let lid = TaskId::new("L").unwrap();
        let chain = p.dependency_chain(&lid).unwrap();
        let chain_strs: Vec<&str> =
            chain.iter().map(|id| id.as_str()).collect();
        assert!(chain_strs.contains(&"R"));
        assert!(chain_strs.contains(&"M"));
        assert!(chain_strs.contains(&"L"));
        assert!(!chain_strs.contains(&"O"));
        assert_eq!(chain.len(), 3);
    }

    #[test]
    fn dependency_chain_single_task() {
        let (p, _) = project_with_tasks(&["A"]);
        let aid = TaskId::new("A").unwrap();
        let chain = p.dependency_chain(&aid).unwrap();
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].as_str(), "A");
    }

    #[test]
    fn dependency_chain_unknown_task_errors() {
        let (p, _) = project_with_tasks(&["A"]);
        let nope = TaskId::new("NOPE").unwrap();
        let result = p.dependency_chain(&nope);
        assert!(result.is_err());
    }
}

use std::collections::{HashMap, HashSet};

use super::tree_node::TreeNode;
use super::Project;
use crate::domain::task::{Status, Task, TaskId};

impl Project {
    /// Build a tree representation of the task dependency
    /// DAG. Root nodes are tasks with no dependencies.
    /// Shared tasks appear as [`TreeNode::Task`] under
    /// their first parent (alphabetically) and as
    /// [`TreeNode::Reference`] under subsequent parents.
    pub fn task_tree(&self) -> Vec<TreeNode> {
        self.build_tree(|_| true)
    }

    /// Build a tree of remaining work, excluding Done and
    /// OnHold tasks. Tasks whose dependencies are all
    /// excluded become new roots.
    pub fn task_tree_remaining(&self) -> Vec<TreeNode> {
        self.build_tree(|t| {
            t.status != Status::Done
                && t.status != Status::OnHold
        })
    }

    /// Shared tree-building logic. Uses
    /// `reverse_dependents` to map each task to the tasks
    /// that depend on it, then DFS from roots to build
    /// the tree.
    fn build_tree(
        &self,
        include: impl Fn(&Task) -> bool,
    ) -> Vec<TreeNode> {
        let included: HashSet<&TaskId> = self
            .tasks
            .iter()
            .filter(|(_, t)| include(t))
            .map(|(id, _)| id)
            .collect();

        // Reuse reverse_dependents and filter to only
        // included tasks on both sides.
        let mut children_of: HashMap<&TaskId, Vec<&TaskId>> =
            self.reverse_dependents(&include);
        // Remove keys not in included set and filter
        // children to included only.
        children_of.retain(|k, _| included.contains(k));
        for v in children_of.values_mut() {
            v.retain(|id| included.contains(id));
            v.sort();
        }

        // Roots: included tasks whose dependencies are
        // all excluded (or empty).
        let mut roots: Vec<&TaskId> = included
            .iter()
            .copied()
            .filter(|id| {
                self.tasks[*id]
                    .dependencies
                    .iter()
                    .all(|dep| !included.contains(dep))
            })
            .collect();
        roots.sort();

        // DFS to build tree nodes. The `seen` set also
        // serves as a cycle guard: if a cycle exists
        // (prevented by add_dependency validation), the
        // revisited node becomes a Reference.
        let mut seen = HashSet::new();
        roots
            .into_iter()
            .map(|id| {
                Self::build_subtree(
                    &self.tasks,
                    id,
                    &children_of,
                    &mut seen,
                )
            })
            .collect()
    }

    /// Recursive DFS to build a subtree.
    fn build_subtree(
        tasks: &std::collections::BTreeMap<TaskId, Task>,
        id: &TaskId,
        children_of: &HashMap<&TaskId, Vec<&TaskId>>,
        seen: &mut HashSet<TaskId>,
    ) -> TreeNode {
        let status = tasks[id].status;

        if !seen.insert(id.clone()) {
            return TreeNode::Reference {
                id: id.clone(),
                status,
            };
        }

        let children = children_of
            .get(id)
            .map(|kids| {
                kids.iter()
                    .map(|kid| {
                        Self::build_subtree(
                            tasks,
                            kid,
                            children_of,
                            seen,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();

        TreeNode::Task {
            id: id.clone(),
            status,
            children,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn tree_empty_project() {
        let p = Project::new("Test").unwrap();
        assert!(p.task_tree().is_empty());
    }

    #[test]
    fn tree_single_task() {
        let (p, _) = project_with_tasks(&["A"]);
        let tree = p.task_tree();
        assert_eq!(tree.len(), 1);
        match &tree[0] {
            TreeNode::Task { id, children, .. } => {
                assert_eq!(id.as_str(), "A");
                assert!(children.is_empty());
            }
            _ => panic!("expected Task node"),
        }
    }

    #[test]
    fn tree_linear_chain() {
        let (mut p, ids) =
            project_with_tasks(&["A", "B", "C"]);
        p.add_dependency(&ids[1], &ids[0]).unwrap();
        p.add_dependency(&ids[2], &ids[1]).unwrap();
        let tree = p.task_tree();
        assert_eq!(tree.len(), 1);
        match &tree[0] {
            TreeNode::Task {
                id, children, ..
            } => {
                assert_eq!(id.as_str(), "A");
                assert_eq!(children.len(), 1);
                match &children[0] {
                    TreeNode::Task {
                        id, children, ..
                    } => {
                        assert_eq!(id.as_str(), "B");
                        assert_eq!(children.len(), 1);
                        match &children[0] {
                            TreeNode::Task {
                                id, ..
                            } => {
                                assert_eq!(
                                    id.as_str(),
                                    "C"
                                );
                            }
                            _ => panic!("expected Task"),
                        }
                    }
                    _ => panic!("expected Task"),
                }
            }
            _ => panic!("expected Task"),
        }
    }

    #[test]
    fn tree_diamond_dag() {
        let (mut p, ids) =
            project_with_tasks(&["A", "B", "C", "D"]);
        p.add_dependency(&ids[1], &ids[0]).unwrap();
        p.add_dependency(&ids[2], &ids[0]).unwrap();
        p.add_dependency(&ids[3], &ids[1]).unwrap();
        p.add_dependency(&ids[3], &ids[2]).unwrap();
        let tree = p.task_tree();
        assert_eq!(tree.len(), 1);
        match &tree[0] {
            TreeNode::Task { children, .. } => {
                assert_eq!(children.len(), 2);
                // B's child D: Task
                match &children[0] {
                    TreeNode::Task {
                        id, children, ..
                    } => {
                        assert_eq!(id.as_str(), "B");
                        assert_eq!(children.len(), 1);
                        assert!(matches!(
                            &children[0],
                            TreeNode::Task { id, .. }
                            if id.as_str() == "D"
                        ));
                    }
                    _ => panic!("expected Task B"),
                }
                // C's child D: Reference
                match &children[1] {
                    TreeNode::Task {
                        id, children, ..
                    } => {
                        assert_eq!(id.as_str(), "C");
                        assert_eq!(children.len(), 1);
                        assert!(matches!(
                            &children[0],
                            TreeNode::Reference { id, .. }
                            if id.as_str() == "D"
                        ));
                    }
                    _ => panic!("expected Task C"),
                }
            }
            _ => panic!("expected root Task"),
        }
    }

    #[test]
    fn tree_multiple_roots() {
        let (p, _) = project_with_tasks(&["A", "B"]);
        let tree = p.task_tree();
        assert_eq!(tree.len(), 2);
    }

    #[test]
    fn tree_remaining_excludes_done() {
        let (mut p, ids) =
            project_with_tasks(&["A", "B", "C"]);
        p.add_dependency(&ids[1], &ids[0]).unwrap();
        p.add_dependency(&ids[2], &ids[1]).unwrap();
        p.set_status(&ids[0], Status::InProgress, false)
            .unwrap();
        p.set_status(&ids[0], Status::Done, false).unwrap();
        let tree = p.task_tree_remaining();
        assert_eq!(tree.len(), 1);
        match &tree[0] {
            TreeNode::Task { id, .. } => {
                assert_eq!(id.as_str(), "B");
            }
            _ => panic!("expected Task B"),
        }
    }

    #[test]
    fn tree_remaining_all_done_empty() {
        let (mut p, ids) = project_with_tasks(&["A"]);
        p.set_status(&ids[0], Status::InProgress, false)
            .unwrap();
        p.set_status(&ids[0], Status::Done, false).unwrap();
        assert!(p.task_tree_remaining().is_empty());
    }
}

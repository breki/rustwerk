use crate::domain::task::{Status, TaskId};

/// A node in the dependency tree representation.
///
/// Since the underlying data is a DAG (tasks can have
/// multiple parents), shared tasks appear as `Task` under
/// their first parent and `Reference` under subsequent
/// parents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeNode {
    /// A fully expanded task node with children.
    Task {
        /// Task ID.
        id: TaskId,
        /// Task status.
        status: Status,
        /// Child nodes (tasks that depend on this one).
        children: Vec<TreeNode>,
    },
    /// A back-reference to a task already shown above.
    Reference {
        /// Task ID (already expanded elsewhere).
        id: TaskId,
        /// Task status.
        status: Status,
    },
}

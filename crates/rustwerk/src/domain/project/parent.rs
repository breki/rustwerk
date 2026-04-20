//! Hierarchical parent-forest operations on [`Project`]
//! (PLG-JIRA-PARENT).
//!
//! The parent forest is distinct from the dependency
//! DAG that lives in [`scheduling`](super::scheduling):
//! parent/child encodes *containment* ("this story
//! belongs under that epic"), whereas dependencies
//! encode *ordering* ("A blocks B"). A task has at most
//! one parent; cycles are rejected at every write path
//! and re-validated at load time.

use std::collections::HashSet;

use chrono::Utc;

use super::Project;
use crate::domain::task::TaskId;
use crate::domain::error::DomainError;

/// Layered view of task IDs ordered parent-first. Level 0
/// holds roots (or tasks whose parent isn't in the
/// requested push set); level N+1 holds tasks whose
/// parent sits at level N. Produced by
/// [`Project::parent_push_levels`] and consumed by the
/// plugin-push orchestrator so each level's DTOs carry
/// the prior level's just-persisted state.
///
/// Invariants enforced at construction:
/// - No empty levels (empty levels are dropped).
/// - Within a level, the caller's original task order is
///   preserved so push output stays deterministic.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PushLevels {
    levels: Vec<Vec<TaskId>>,
}

impl PushLevels {
    /// Number of non-empty levels.
    #[must_use]
    pub fn len(&self) -> usize {
        self.levels.len()
    }

    /// `true` when no tasks were scheduled for push.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }

    /// Iterate levels in parent-first order. Each item
    /// is the slice of task IDs at that depth.
    pub fn iter(&self) -> std::slice::Iter<'_, Vec<TaskId>> {
        self.levels.iter()
    }
}

impl<'a> IntoIterator for &'a PushLevels {
    type Item = &'a Vec<TaskId>;
    type IntoIter = std::slice::Iter<'a, Vec<TaskId>>;

    fn into_iter(self) -> Self::IntoIter {
        self.levels.iter()
    }
}

impl Project {
    /// Set `child`'s parent to `parent`, validating both
    /// endpoints exist and the new edge keeps the parent
    /// forest acyclic.
    ///
    /// The parent forest is distinct from the dependency
    /// DAG: parent/child encodes containment (an epic
    /// owns a story), dependencies encode ordering (A
    /// blocks B). A task has at most one parent; cycles
    /// are rejected.
    pub fn set_parent(
        &mut self,
        child: &TaskId,
        parent: &TaskId,
    ) -> Result<(), DomainError> {
        if child == parent {
            return Err(DomainError::CycleDetected(format!(
                "{child} -> {child} (self-parent)"
            )));
        }
        if !self.tasks.contains_key(child) {
            return Err(DomainError::TaskNotFound(child.to_string()));
        }
        if !self.tasks.contains_key(parent) {
            return Err(DomainError::TaskNotFound(parent.to_string()));
        }
        // Assigning `parent` as the ancestor of `child`
        // must not reach `child` by following parent
        // edges upwards — that would form a cycle.
        if self.is_ancestor_of(child, parent) {
            return Err(DomainError::CycleDetected(format!(
                "parent edge {child} -> {parent} would create a cycle"
            )));
        }
        self.tasks.get_mut(child).unwrap().parent = Some(parent.clone());
        self.metadata.modified_at = Utc::now();
        Ok(())
    }

    /// Clear `child`'s parent. No-op (success) when the
    /// task has no parent set.
    pub fn unparent(&mut self, child: &TaskId) -> Result<(), DomainError> {
        let task = self
            .tasks
            .get_mut(child)
            .ok_or_else(|| DomainError::TaskNotFound(child.to_string()))?;
        if task.parent.is_some() {
            task.parent = None;
            self.metadata.modified_at = Utc::now();
        }
        Ok(())
    }

    /// Return `true` when `candidate` is `descendant` or
    /// an ancestor of `descendant` via parent edges.
    /// Used by [`Self::set_parent`] to reject cycles.
    ///
    /// The `seen` guard is cheap insurance against a
    /// runtime-corrupted project that bypassed load-time
    /// validation — infinite-looping the CLI would be a
    /// worse failure mode than a false "not an ancestor"
    /// answer here (the caller re-validates the edge
    /// with its own explicit cycle check).
    fn is_ancestor_of(&self, candidate: &TaskId, descendant: &TaskId) -> bool {
        let mut seen: HashSet<TaskId> = HashSet::new();
        let mut cursor = Some(descendant.clone());
        while let Some(id) = cursor {
            if &id == candidate {
                return true;
            }
            if !seen.insert(id.clone()) {
                return false;
            }
            cursor = self
                .tasks
                .get(&id)
                .and_then(|t| t.parent.clone());
        }
        false
    }

    /// Group the given `task_ids` into parent-first
    /// levels. See [`PushLevels`] for the shape.
    ///
    /// Tasks with parents outside `task_ids` land at
    /// level 0 — the caller is pushing an arbitrary
    /// subset and the missing parent is assumed
    /// pre-existing. Orphan/missing-parent handling at
    /// the Jira wire level is the plugin's concern (it
    /// emits a warning and skips the parent field).
    ///
    /// Returns [`DomainError::CycleDetected`] if the
    /// parent-edge walk revisits a task — the in-memory
    /// forest is corrupted despite load validation, and
    /// silently truncating depth would push tasks in the
    /// wrong order (producing orphan `parent.key` refs
    /// in Jira that point to issues that don't yet exist).
    pub fn parent_push_levels(
        &self,
        task_ids: &[TaskId],
    ) -> Result<PushLevels, DomainError> {
        use std::collections::HashMap;
        let in_set: HashSet<&TaskId> = task_ids.iter().collect();
        let mut depth: HashMap<TaskId, usize> = HashMap::new();
        for id in task_ids {
            let mut d = 0usize;
            let mut seen: HashSet<TaskId> = HashSet::new();
            seen.insert(id.clone());
            let mut cursor = self.tasks.get(id).and_then(|t| t.parent.clone());
            while let Some(p) = cursor {
                if !in_set.contains(&p) {
                    break;
                }
                if !seen.insert(p.clone()) {
                    return Err(DomainError::CycleDetected(format!(
                        "parent-edge cycle involving {id} while computing push levels"
                    )));
                }
                d += 1;
                cursor = self.tasks.get(&p).and_then(|t| t.parent.clone());
            }
            depth.insert(id.clone(), d);
        }
        let max_depth = depth.values().copied().max().unwrap_or(0);
        let mut levels: Vec<Vec<TaskId>> =
            (0..=max_depth).map(|_| Vec::new()).collect();
        for id in task_ids {
            let d = depth[id];
            levels[d].push(id.clone());
        }
        levels.retain(|lvl| !lvl.is_empty());
        Ok(PushLevels { levels })
    }

    /// Project-load parent-edge validation. Called once
    /// after deserialization to reject self-parents,
    /// dangling parent references, and cycles in the
    /// parent forest.
    pub(crate) fn validate_parent_forest(&self) -> Result<(), DomainError> {
        for (id, task) in &self.tasks {
            let Some(parent) = task.parent.as_ref() else {
                continue;
            };
            if parent == id {
                return Err(DomainError::CycleDetected(format!(
                    "task {id} is its own parent"
                )));
            }
            if !self.tasks.contains_key(parent) {
                return Err(DomainError::ValidationError(format!(
                    "task {id} has parent {parent} which does not exist"
                )));
            }
            let mut cursor = Some(parent.clone());
            let mut seen = HashSet::new();
            seen.insert(id.clone());
            while let Some(cur) = cursor {
                if !seen.insert(cur.clone()) {
                    return Err(DomainError::CycleDetected(format!(
                        "parent cycle involving {id}"
                    )));
                }
                cursor = self.tasks.get(&cur).and_then(|t| t.parent.clone());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::task::Task;

    fn tid(s: &str) -> TaskId {
        TaskId::new(s).unwrap()
    }

    fn project_with(ids: &[&str]) -> Project {
        let mut p = Project::new("test").unwrap();
        for id in ids {
            p.add_task(tid(id), Task::new(id).unwrap()).unwrap();
        }
        p
    }

    #[test]
    fn set_parent_records_edge() {
        let mut p = project_with(&["EPIC", "CHILD"]);
        p.set_parent(&tid("CHILD"), &tid("EPIC")).unwrap();
        assert_eq!(
            p.tasks[&tid("CHILD")].parent.as_ref(),
            Some(&tid("EPIC"))
        );
    }

    #[test]
    fn set_parent_rejects_self_parent() {
        let mut p = project_with(&["A"]);
        let err = p.set_parent(&tid("A"), &tid("A")).unwrap_err();
        assert!(matches!(err, DomainError::CycleDetected(_)));
    }

    #[test]
    fn set_parent_rejects_cycle() {
        let mut p = project_with(&["A", "B", "C"]);
        p.set_parent(&tid("B"), &tid("A")).unwrap();
        p.set_parent(&tid("C"), &tid("B")).unwrap();
        let err = p.set_parent(&tid("A"), &tid("C")).unwrap_err();
        assert!(matches!(err, DomainError::CycleDetected(_)));
    }

    #[test]
    fn set_parent_rejects_missing_parent() {
        let mut p = project_with(&["A"]);
        let err = p.set_parent(&tid("A"), &tid("MISSING")).unwrap_err();
        assert!(matches!(err, DomainError::TaskNotFound(_)));
    }

    #[test]
    fn unparent_clears_edge() {
        let mut p = project_with(&["A", "B"]);
        p.set_parent(&tid("B"), &tid("A")).unwrap();
        p.unparent(&tid("B")).unwrap();
        assert!(p.tasks[&tid("B")].parent.is_none());
    }

    #[test]
    fn unparent_is_noop_when_already_root() {
        let mut p = project_with(&["A"]);
        p.unparent(&tid("A")).unwrap();
        assert!(p.tasks[&tid("A")].parent.is_none());
    }

    #[test]
    fn parent_push_levels_groups_by_depth() {
        let mut p = project_with(&["EPIC", "STORY1", "STORY2", "SUB"]);
        p.set_parent(&tid("STORY1"), &tid("EPIC")).unwrap();
        p.set_parent(&tid("STORY2"), &tid("EPIC")).unwrap();
        p.set_parent(&tid("SUB"), &tid("STORY1")).unwrap();
        let levels = p
            .parent_push_levels(&[
                tid("SUB"),
                tid("STORY1"),
                tid("EPIC"),
                tid("STORY2"),
            ])
            .unwrap();
        assert_eq!(levels.len(), 3);
        let v: Vec<&Vec<TaskId>> = levels.iter().collect();
        assert_eq!(v[0], &vec![tid("EPIC")]);
        assert!(v[1].contains(&tid("STORY1")) && v[1].contains(&tid("STORY2")));
        assert_eq!(v[2], &vec![tid("SUB")]);
    }

    #[test]
    fn parent_push_levels_treats_out_of_set_parent_as_root() {
        let mut p = project_with(&["EPIC", "CHILD"]);
        p.set_parent(&tid("CHILD"), &tid("EPIC")).unwrap();
        let levels = p.parent_push_levels(&[tid("CHILD")]).unwrap();
        assert_eq!(levels.len(), 1);
        let v: Vec<&Vec<TaskId>> = levels.iter().collect();
        assert_eq!(v[0], &vec![tid("CHILD")]);
    }

    #[test]
    fn parent_push_levels_rejects_runtime_cycle() {
        // RT-Y1: bypass `set_parent` validation to simulate
        // in-memory corruption. Must fail loud — silent
        // depth truncation would produce wrong push order.
        let mut p = project_with(&["A", "B"]);
        p.tasks.get_mut(&tid("A")).unwrap().parent = Some(tid("B"));
        p.tasks.get_mut(&tid("B")).unwrap().parent = Some(tid("A"));
        let err = p
            .parent_push_levels(&[tid("A"), tid("B")])
            .unwrap_err();
        assert!(matches!(err, DomainError::CycleDetected(_)));
    }

    #[test]
    fn parent_push_levels_preserves_caller_order_within_level() {
        let p = project_with(&["A", "B", "C"]);
        let levels = p
            .parent_push_levels(&[tid("C"), tid("A"), tid("B")])
            .unwrap();
        let v: Vec<&Vec<TaskId>> = levels.iter().collect();
        assert_eq!(v[0], &vec![tid("C"), tid("A"), tid("B")]);
    }

    #[test]
    fn parent_push_levels_empty_input_is_empty_output() {
        let p = Project::new("test").unwrap();
        let levels = p.parent_push_levels(&[]).unwrap();
        assert!(levels.is_empty());
    }

    #[test]
    fn validate_parent_forest_accepts_valid_forest() {
        let mut p = project_with(&["A", "B", "C"]);
        p.set_parent(&tid("B"), &tid("A")).unwrap();
        p.set_parent(&tid("C"), &tid("A")).unwrap();
        assert!(p.validate_parent_forest().is_ok());
    }

    #[test]
    fn validate_parent_forest_rejects_dangling_parent_reference() {
        let mut p = project_with(&["A"]);
        p.tasks.get_mut(&tid("A")).unwrap().parent = Some(tid("GHOST"));
        let err = p.validate_parent_forest().unwrap_err();
        assert!(matches!(err, DomainError::ValidationError(_)));
    }

    #[test]
    fn validate_parent_forest_rejects_self_parent() {
        let mut p = project_with(&["A"]);
        p.tasks.get_mut(&tid("A")).unwrap().parent = Some(tid("A"));
        let err = p.validate_parent_forest().unwrap_err();
        assert!(matches!(err, DomainError::CycleDetected(_)));
    }

    #[test]
    fn validate_parent_forest_rejects_cycle() {
        let mut p = project_with(&["A", "B"]);
        p.tasks.get_mut(&tid("A")).unwrap().parent = Some(tid("B"));
        p.tasks.get_mut(&tid("B")).unwrap().parent = Some(tid("A"));
        let err = p.validate_parent_forest().unwrap_err();
        assert!(matches!(err, DomainError::CycleDetected(_)));
    }

    #[test]
    fn is_ancestor_of_handles_runtime_cycle_without_hanging() {
        // RT-Y5: runtime-corrupted cycle must not hang.
        let mut p = project_with(&["A", "B"]);
        p.tasks.get_mut(&tid("A")).unwrap().parent = Some(tid("B"));
        p.tasks.get_mut(&tid("B")).unwrap().parent = Some(tid("A"));
        // Not actually in the forest; the cycle guard
        // keeps this bounded instead of looping forever.
        assert!(!p.is_ancestor_of(&tid("X"), &tid("A")));
    }
}

use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::project::Project;
use crate::domain::task::TaskId;

/// Errors that can occur during file-based persistence.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// I/O error reading or writing the project file.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Loaded project file is syntactically valid JSON
    /// but semantically broken (e.g. dependency graph
    /// contains a cycle).
    #[error("invalid project: {0}")]
    InvalidProject(String),
}

/// Convention: project file lives at
/// `.rustwerk/project.json` relative to the repo root.
const PROJECT_DIR: &str = ".rustwerk";

/// Project file name.
const PROJECT_FILE: &str = "project.json";

/// Return the path to the project file given a root
/// directory.
pub fn project_file_path(root: &Path) -> PathBuf {
    root.join(PROJECT_DIR).join(PROJECT_FILE)
}

/// Convention: task description files live at
/// `.rustwerk/tasks/<ID>.md` relative to the repo root.
const TASKS_DIR: &str = "tasks";

/// Return the path to a task description file given a
/// root directory and task ID.
pub fn task_description_path(root: &Path, task_id: &TaskId) -> PathBuf {
    root.join(PROJECT_DIR)
        .join(TASKS_DIR)
        .join(task_id.as_str())
        .with_extension("md")
}

/// Outcome of a description-file rename.
#[derive(Debug, PartialEq, Eq)]
pub enum DescriptionRenameOutcome {
    /// No source file existed; nothing to move.
    NoSource,
    /// The file was moved.
    Moved,
}

/// Errors specific to description-file operations.
#[derive(Debug, thiserror::Error)]
pub enum DescriptionFileError {
    /// The destination path already exists; refusing to
    /// overwrite.
    #[error(
        "destination description file already exists: \
             {path}"
    )]
    DestinationExists {
        /// The destination path that already exists.
        path: String,
    },
    /// Underlying I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Rename a task description file from `from` to `to`.
/// Returns `NoSource` if the source does not exist (not
/// an error). Refuses to overwrite an existing
/// destination.
pub fn rename_task_description(
    root: &Path,
    from: &TaskId,
    to: &TaskId,
) -> Result<DescriptionRenameOutcome, DescriptionFileError> {
    if from == to {
        return Ok(DescriptionRenameOutcome::NoSource);
    }
    let old_path = task_description_path(root, from);
    let new_path = task_description_path(root, to);
    if !old_path.exists() {
        return Ok(DescriptionRenameOutcome::NoSource);
    }
    if new_path.exists() {
        return Err(DescriptionFileError::DestinationExists {
            path: new_path.display().to_string(),
        });
    }
    fs::rename(&old_path, &new_path)?;
    Ok(DescriptionRenameOutcome::Moved)
}

/// Remove a task description file. Returns `true` if a
/// file was removed, `false` if none existed.
pub fn remove_task_description(
    root: &Path,
    id: &TaskId,
) -> Result<bool, std::io::Error> {
    let path = task_description_path(root, id);
    match fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// Save a project to `.rustwerk/project.json` under the
/// given root directory. Creates the directory if it does
/// not exist.
pub fn save(root: &Path, project: &Project) -> Result<(), StoreError> {
    let path = project_file_path(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(project)?;
    fs::write(&path, json)?;
    Ok(())
}

/// Load a project from `.rustwerk/project.json` under the
/// given root directory.
///
/// Validates that the loaded dependency graph is a DAG —
/// a hand-edited `project.json` with cycles would
/// otherwise make downstream commands like `gantt` or
/// `task list` panic when they compute the critical path.
pub fn load(root: &Path) -> Result<Project, StoreError> {
    let path = project_file_path(root);
    let json = fs::read_to_string(&path)?;
    let project: Project = serde_json::from_str(&json)?;
    // Kahn's algorithm drops cycle participants silently.
    // If the result is shorter than the task list, the
    // loaded graph contains at least one cycle.
    let order_len = project.topological_sort().len();
    if order_len != project.tasks.len() {
        return Err(StoreError::InvalidProject(format!(
            "dependency graph contains a cycle ({} tasks, {} in topological order)",
            project.tasks.len(),
            order_len,
        )));
    }
    project
        .validate_parent_forest()
        .map_err(|e| StoreError::InvalidProject(e.to_string()))?;
    Ok(project)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("rustwerk-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = temp_dir("round-trip");
        let project = Project::new("Round Trip").unwrap();
        save(&dir, &project).unwrap();
        let loaded = load(&dir).unwrap();
        assert_eq!(loaded.metadata.name, "Round Trip");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_creates_directory_and_file() {
        let dir = temp_dir("creates-file");
        assert!(!project_file_path(&dir).exists());
        let project = Project::new("Test").unwrap();
        save(&dir, &project).unwrap();
        assert!(project_file_path(&dir).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_nonexistent_file_errors() {
        let dir = temp_dir("nonexistent");
        let result = load(&dir);
        assert!(result.is_err());
    }

    #[test]
    fn project_file_path_convention() {
        let path = project_file_path(Path::new("/repo"));
        assert!(path.ends_with(".rustwerk/project.json"));
    }

    #[test]
    fn rename_description_no_source_is_ok() {
        let dir = temp_dir("rename-no-src");
        fs::create_dir_all(dir.join(".rustwerk/tasks")).unwrap();
        let from = TaskId::new("A").unwrap();
        let to = TaskId::new("B").unwrap();
        let outcome = rename_task_description(&dir, &from, &to).unwrap();
        assert_eq!(outcome, DescriptionRenameOutcome::NoSource);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rename_description_moves_file() {
        let dir = temp_dir("rename-moves");
        let tasks = dir.join(".rustwerk/tasks");
        fs::create_dir_all(&tasks).unwrap();
        fs::write(tasks.join("A.md"), "body").unwrap();
        let from = TaskId::new("A").unwrap();
        let to = TaskId::new("B").unwrap();
        let outcome = rename_task_description(&dir, &from, &to).unwrap();
        assert_eq!(outcome, DescriptionRenameOutcome::Moved);
        assert!(!tasks.join("A.md").exists());
        assert!(tasks.join("B.md").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rename_description_refuses_overwrite() {
        let dir = temp_dir("rename-refuse");
        let tasks = dir.join(".rustwerk/tasks");
        fs::create_dir_all(&tasks).unwrap();
        fs::write(tasks.join("A.md"), "a").unwrap();
        fs::write(tasks.join("B.md"), "b").unwrap();
        let from = TaskId::new("A").unwrap();
        let to = TaskId::new("B").unwrap();
        let err = rename_task_description(&dir, &from, &to).unwrap_err();
        assert!(matches!(
            err,
            DescriptionFileError::DestinationExists { .. }
        ));
        // Both files still present.
        assert!(tasks.join("A.md").exists());
        assert_eq!(fs::read_to_string(tasks.join("B.md")).unwrap(), "b");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rename_description_same_id_is_noop() {
        let dir = temp_dir("rename-same");
        let tasks = dir.join(".rustwerk/tasks");
        fs::create_dir_all(&tasks).unwrap();
        fs::write(tasks.join("A.md"), "body").unwrap();
        let id = TaskId::new("A").unwrap();
        let outcome = rename_task_description(&dir, &id, &id).unwrap();
        assert_eq!(outcome, DescriptionRenameOutcome::NoSource);
        assert!(tasks.join("A.md").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_description_missing_is_ok() {
        let dir = temp_dir("remove-missing");
        fs::create_dir_all(dir.join(".rustwerk/tasks")).unwrap();
        let id = TaskId::new("NOPE").unwrap();
        assert!(!remove_task_description(&dir, &id).unwrap());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_description_deletes_file() {
        let dir = temp_dir("remove-deletes");
        let tasks = dir.join(".rustwerk/tasks");
        fs::create_dir_all(&tasks).unwrap();
        fs::write(tasks.join("A.md"), "body").unwrap();
        let id = TaskId::new("A").unwrap();
        assert!(remove_task_description(&dir, &id).unwrap());
        assert!(!tasks.join("A.md").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn task_description_path_convention() {
        let tid = crate::domain::task::TaskId::new("PLG-API").unwrap();
        let path = task_description_path(Path::new("/repo"), &tid);
        assert!(path.ends_with(".rustwerk/tasks/PLG-API.md"));
    }

    #[test]
    fn load_rejects_cyclic_graph_from_disk() {
        // Hand-craft a `project.json` with a cycle
        // (A → B, B → A). The runtime `add_dependency`
        // prevents cycles, but a hand-edit or upstream
        // corruption could reach this state.
        let dir = temp_dir("cycle-load");
        let rustwerk = dir.join(".rustwerk");
        fs::create_dir_all(&rustwerk).unwrap();
        let json = r#"{
            "metadata": {
                "name": "Cycle",
                "created_at": "2026-04-19T00:00:00Z",
                "modified_at": "2026-04-19T00:00:00Z"
            },
            "tasks": {
                "A": {
                    "title": "A",
                    "status": "todo",
                    "dependencies": ["B"],
                    "effort_entries": [],
                    "tags": []
                },
                "B": {
                    "title": "B",
                    "status": "todo",
                    "dependencies": ["A"],
                    "effort_entries": [],
                    "tags": []
                }
            },
            "developers": {},
            "next_task_id": 1
        }"#;
        fs::write(rustwerk.join("project.json"), json).unwrap();
        let err = load(&dir).unwrap_err();
        match err {
            StoreError::InvalidProject(msg) => assert!(
                msg.contains("cycle"),
                "expected 'cycle' in error, got: {msg}"
            ),
            other => panic!("expected InvalidProject, got {other:?}"),
        }
        let _ = fs::remove_dir_all(&dir);
    }
}

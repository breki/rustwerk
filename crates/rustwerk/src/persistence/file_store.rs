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
pub fn load(root: &Path) -> Result<Project, StoreError> {
    let path = project_file_path(root);
    let json = fs::read_to_string(&path)?;
    let project = serde_json::from_str(&json)?;
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
    fn task_description_path_convention() {
        let tid = crate::domain::task::TaskId::new("PLG-API").unwrap();
        let path = task_description_path(Path::new("/repo"), &tid);
        assert!(path.ends_with(".rustwerk/tasks/PLG-API.md"));
    }
}

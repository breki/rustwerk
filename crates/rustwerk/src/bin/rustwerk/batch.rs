use std::io::Read as _;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use rustwerk::domain::developer::{Developer, DeveloperId};
use rustwerk::domain::project::Project;
use rustwerk::domain::task::{Effort, EffortEntry, IssueType, Task, TaskId};
use rustwerk::persistence::file_store;

use crate::{load_project, parse_status, save_project};

/// Maximum batch input size (10 MB).
const MAX_BATCH_BYTES: u64 = 10 * 1024 * 1024;

/// Maximum number of commands in a single batch.
const MAX_BATCH_COMMANDS: usize = 1000;

/// A single command in a batch request.
#[derive(Debug, Deserialize)]
struct BatchCommand {
    /// Command name (e.g. "task.add", "task.status").
    command: String,
    /// Command arguments as a JSON object.
    args: serde_json::Value,
}

/// A single result from a batch execution.
#[derive(Debug, Serialize)]
struct BatchResult {
    /// Zero-based index of the command.
    index: usize,
    /// Whether the command succeeded.
    ok: bool,
    /// Human-readable result or error message.
    message: String,
}

/// Parse a JSON array of tags, requiring all elements to
/// be strings.
fn parse_batch_tags(arr: &[serde_json::Value]) -> Result<Vec<&str>> {
    arr.iter()
        .map(|v| v.as_str().context("tags array must contain only strings"))
        .collect()
}

/// Side effects that must be applied to the filesystem
/// after the project JSON is persisted. Collected during
/// batch execution and replayed in order so chained
/// renames (A→B, B→C) process files correctly.
#[derive(Debug)]
enum FileSideEffect {
    RenameDescription { from: TaskId, to: TaskId },
    RemoveDescription { id: TaskId },
}

/// Execute a single batch command against a project.
/// File system changes (description file moves/deletes)
/// are recorded into `side_effects` rather than applied
/// directly, so they can be replayed after the
/// project JSON is persisted.
#[allow(clippy::too_many_lines)] // dispatch match
fn execute_one(
    project: &mut Project,
    cmd: &BatchCommand,
    side_effects: &mut Vec<FileSideEffect>,
) -> Result<String> {
    let args = &cmd.args;
    match cmd.command.as_str() {
        "task.add" => {
            let title = args["title"]
                .as_str()
                .context("task.add requires 'title'")?;
            let mut task = Task::new(title)?;
            if let Some(d) = args.get("desc").and_then(|v| v.as_str()) {
                task.description = Some(d.to_string());
            }
            if let Some(c) =
                args.get("complexity").and_then(serde_json::Value::as_u64)
            {
                let c = u32::try_from(c).map_err(|_| {
                    anyhow::anyhow!("complexity value too large: {c}")
                })?;
                task.set_complexity(c)?;
            }
            if let Some(e) = args.get("effort").and_then(|v| v.as_str()) {
                task.effort_estimate = Some(Effort::parse(e)?);
            }
            if let Some(tags) = args.get("tags").and_then(|v| v.as_array()) {
                let tag_strs = parse_batch_tags(tags)?;
                task.set_tags(&tag_strs)?;
            }
            if let Some(t) = args
                .get("issue_type")
                .or_else(|| args.get("type"))
                .and_then(|v| v.as_str())
            {
                task.issue_type = Some(IssueType::parse(t)?);
            }
            let task_id =
                if let Some(id_str) = args.get("id").and_then(|v| v.as_str()) {
                    let tid = TaskId::new(id_str)?;
                    project.add_task(tid.clone(), task)?;
                    tid
                } else {
                    project.add_task_auto(task)
                };
            if let Some(p) = args.get("parent").and_then(|v| v.as_str()) {
                let parent_id = TaskId::new(p)?;
                project.set_parent(&task_id, &parent_id)?;
            }
            Ok(format!("Created task {task_id}"))
        }
        "task.remove" => {
            let id =
                args["id"].as_str().context("task.remove requires 'id'")?;
            let task_id = TaskId::new(id)?;
            let task = project.remove_task(&task_id)?;
            side_effects
                .push(FileSideEffect::RemoveDescription { id: task_id.clone() });
            Ok(format!("Removed {task_id}: {}", task.title))
        }
        "task.update" => {
            let id =
                args["id"].as_str().context("task.update requires 'id'")?;
            let task_id = TaskId::new(id)?;
            let title = args.get("title").and_then(|v| v.as_str());
            let desc = args.get("desc").and_then(|v| v.as_str());
            let tags = args.get("tags").and_then(|v| v.as_array());
            let issue_type = args
                .get("issue_type")
                .or_else(|| args.get("type"))
                .and_then(|v| v.as_str());
            let parent = args.get("parent").and_then(|v| v.as_str());
            if title.is_none()
                && desc.is_none()
                && tags.is_none()
                && issue_type.is_none()
                && parent.is_none()
            {
                bail!(
                    "task.update requires at least one \
                     of 'title', 'desc', 'tags', 'issue_type', \
                     or 'parent'"
                );
            }
            let description =
                desc.map(|d| if d.is_empty() { None } else { Some(d) });
            project.update_task(&task_id, title, description)?;
            if let Some(tag_arr) = tags {
                let tag_strs = parse_batch_tags(tag_arr)?;
                project.set_task_tags(&task_id, &tag_strs)?;
            }
            if let Some(t) = issue_type {
                let parsed = if t.is_empty() {
                    None
                } else {
                    Some(IssueType::parse(t)?)
                };
                project.set_task_issue_type(&task_id, parsed)?;
            }
            if let Some(p) = parent {
                if p.is_empty() {
                    bail!(
                        "use the 'task.unparent' action to clear the parent edge; \
                         an empty 'parent' string is not accepted"
                    );
                }
                let parent_id = TaskId::new(p)?;
                project.set_parent(&task_id, &parent_id)?;
            }
            Ok(format!("Updated {task_id}"))
        }
        "task.unparent" => {
            let id = args["id"]
                .as_str()
                .context("task.unparent requires 'id'")?;
            let task_id = TaskId::new(id)?;
            project.unparent(&task_id)?;
            Ok(format!("Unparented {task_id}"))
        }
        "task.rename" => {
            let from = args["old_id"]
                .as_str()
                .context("task.rename requires 'old_id'")?;
            let to = args["new_id"]
                .as_str()
                .context("task.rename requires 'new_id'")?;
            let from_id = TaskId::new(from)?;
            let to_id = TaskId::new(to)?;
            project.rename_task(&from_id, &to_id)?;
            side_effects.push(FileSideEffect::RenameDescription {
                from: from_id.clone(),
                to: to_id.clone(),
            });
            Ok(format!("{from_id}: renamed to {to_id}"))
        }
        "task.status" => {
            let id =
                args["id"].as_str().context("task.status requires 'id'")?;
            let status = args["status"]
                .as_str()
                .context("task.status requires 'status'")?;
            let force = args
                .get("force")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let task_id = TaskId::new(id)?;
            let new_status = parse_status(status)?;
            project.set_status(&task_id, new_status, force)?;
            Ok(format!("{task_id}: {new_status}"))
        }
        // Batch commands are deterministic: all arguments
        // must be explicit in the JSON. No RUSTWERK_USER
        // fallback — the caller must always supply "to".
        "task.assign" => {
            let id =
                args["id"].as_str().context("task.assign requires 'id'")?;
            let to =
                args["to"].as_str().context("task.assign requires 'to'")?;
            let task_id = TaskId::new(id)?;
            let dev_id = DeveloperId::new(to)?;
            project.assign(&task_id, &dev_id)?;
            Ok(format!("{task_id}: assigned to {dev_id}"))
        }
        "task.unassign" => {
            let id =
                args["id"].as_str().context("task.unassign requires 'id'")?;
            let task_id = TaskId::new(id)?;
            project.unassign(&task_id)?;
            Ok(format!("{task_id}: unassigned"))
        }
        "task.depend" => {
            let from = args["from"]
                .as_str()
                .context("task.depend requires 'from'")?;
            let to =
                args["to"].as_str().context("task.depend requires 'to'")?;
            let from_id = TaskId::new(from)?;
            let to_id = TaskId::new(to)?;
            project.add_dependency(&from_id, &to_id)?;
            Ok(format!("{from_id} depends on {to_id}"))
        }
        "task.undepend" => {
            let from = args["from"]
                .as_str()
                .context("task.undepend requires 'from'")?;
            let to =
                args["to"].as_str().context("task.undepend requires 'to'")?;
            let from_id = TaskId::new(from)?;
            let to_id = TaskId::new(to)?;
            project.remove_dependency(&from_id, &to_id)?;
            Ok(format!("Removed: {from_id} depends on {to_id}"))
        }
        "effort.log" => {
            let id = args["id"].as_str().context("effort.log requires 'id'")?;
            let amount = args["amount"]
                .as_str()
                .context("effort.log requires 'amount'")?;
            let dev =
                args["dev"].as_str().context("effort.log requires 'dev'")?;
            let note = args.get("note").and_then(|v| v.as_str());
            let task_id = TaskId::new(id)?;
            let effort = Effort::parse(amount)?;
            let entry = EffortEntry {
                effort,
                developer: dev.to_string(),
                timestamp: chrono::Utc::now(),
                note: note.map(String::from),
            };
            project.log_effort(&task_id, entry)?;
            Ok(format!("{task_id}: logged {amount}"))
        }
        "effort.estimate" => {
            let id = args["id"]
                .as_str()
                .context("effort.estimate requires 'id'")?;
            let amount = args["amount"]
                .as_str()
                .context("effort.estimate requires 'amount'")?;
            let task_id = TaskId::new(id)?;
            let effort = Effort::parse(amount)?;
            project.set_effort_estimate(&task_id, effort)?;
            Ok(format!("{task_id}: estimate set to {amount}"))
        }
        "dev.add" => {
            let id = args["id"].as_str().context("dev.add requires 'id'")?;
            let name =
                args["name"].as_str().context("dev.add requires 'name'")?;
            let dev_id = DeveloperId::new(id)?;
            let mut dev = Developer::new(name)?;
            dev.email =
                args.get("email").and_then(|v| v.as_str()).map(String::from);
            dev.role =
                args.get("role").and_then(|v| v.as_str()).map(String::from);
            project.add_developer(dev_id.clone(), dev)?;
            Ok(format!("Added developer {dev_id}"))
        }
        "dev.remove" => {
            let id = args["id"].as_str().context("dev.remove requires 'id'")?;
            let dev_id = DeveloperId::new(id)?;
            let dev = project.remove_developer(&dev_id)?;
            Ok(format!("Removed developer {dev_id}: {}", dev.name))
        }
        other => bail!("unknown command: {other}"),
    }
}

/// Execute a batch of commands from a file or stdin.
pub(super) fn cmd_batch(file: Option<&str>) -> Result<()> {
    let json = if let Some(path) = file {
        std::fs::read_to_string(path).context("failed to read batch file")?
    } else {
        let mut buf = String::new();
        std::io::stdin()
            .take(MAX_BATCH_BYTES)
            .read_to_string(&mut buf)
            .context("failed to read stdin")?;
        buf
    };

    let commands: Vec<BatchCommand> =
        serde_json::from_str(&json).context("failed to parse batch JSON")?;

    if commands.len() > MAX_BATCH_COMMANDS {
        bail!(
            "batch contains {} commands (max {})",
            commands.len(),
            MAX_BATCH_COMMANDS
        );
    }

    // Always load the project to validate it exists,
    // even for empty batches.
    let (root, mut project) = load_project()?;

    if commands.is_empty() {
        println!("[]");
        return Ok(());
    }
    let mut results = Vec::with_capacity(commands.len());
    let mut side_effects: Vec<FileSideEffect> = Vec::new();

    for (i, cmd) in commands.iter().enumerate() {
        match execute_one(&mut project, cmd, &mut side_effects) {
            Ok(msg) => {
                results.push(BatchResult {
                    index: i,
                    ok: true,
                    message: msg,
                });
            }
            Err(e) => {
                // Sanitize command name: truncate and
                // strip non-printable characters.
                let safe_cmd: String = cmd
                    .command
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(64)
                    .collect();
                let error_output = serde_json::json!({
                    "error": format!(
                        "command {} ({}) failed: {e}",
                        i, safe_cmd
                    ),
                    "applied": i
                });
                eprintln!("{}", serde_json::to_string_pretty(&error_output)?);
                bail!("batch failed at command {i} ({safe_cmd})");
            }
        }
    }

    save_project(&root, &project)?;

    // Apply filesystem side effects in command order.
    // The project JSON is already persisted, so failures
    // here leave the JSON and the filesystem diverged.
    // We collect errors, print `results` first so agents
    // can see what commands succeeded at the JSON level,
    // then report fs failures on stderr and exit
    // non-zero.
    let mut fs_errors: Vec<String> = Vec::new();
    for effect in &side_effects {
        match effect {
            FileSideEffect::RenameDescription { from, to } => {
                if let Err(e) =
                    file_store::rename_task_description(&root, from, to)
                {
                    fs_errors.push(format!(
                        "rename description {from} -> {to}: {e}"
                    ));
                }
            }
            FileSideEffect::RemoveDescription { id } => {
                if let Err(e) = file_store::remove_task_description(&root, id) {
                    fs_errors.push(format!("remove description {id}: {e}"));
                }
            }
        }
    }

    println!("{}", serde_json::to_string_pretty(&results)?);
    if !fs_errors.is_empty() {
        let report = serde_json::json!({
            "error": "project.json was saved, but one or \
                      more description-file operations \
                      failed",
            "fs_errors": fs_errors,
        });
        eprintln!("{}", serde_json::to_string_pretty(&report)?);
        bail!(
            "batch applied to project.json but {} file \
             operation(s) failed",
            fs_errors.len()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustwerk::domain::task::Status;

    fn test_project() -> Project {
        Project::new("Test").unwrap()
    }

    /// Test helper that drops the collected side effects.
    fn run_one(p: &mut Project, cmd: &BatchCommand) -> Result<String> {
        let mut sfx = Vec::new();
        execute_one(p, cmd, &mut sfx)
    }

    fn add_test_dev(p: &mut Project, id: &str) {
        use rustwerk::domain::developer::{Developer, DeveloperId};
        let dev_id = DeveloperId::new(id).unwrap();
        let dev = Developer::new(id).unwrap();
        let _ = p.add_developer(dev_id, dev);
    }

    // --- execute_one: task.add ---

    #[test]
    fn batch_task_add_with_id() {
        let mut p = test_project();
        let cmd = BatchCommand {
            command: "task.add".into(),
            args: serde_json::json!({
                "title": "My task",
                "id": "MT",
                "complexity": 5,
                "effort": "8H",
                "desc": "A description"
            }),
        };
        let msg = run_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("MT"));
        assert_eq!(p.task_count(), 1);
        let task = &p.tasks[&TaskId::new("MT").unwrap()];
        assert_eq!(task.title, "My task");
        assert_eq!(task.complexity, Some(5));
        assert_eq!(task.description.as_deref(), Some("A description"));
    }

    #[test]
    fn batch_task_add_auto_id() {
        let mut p = test_project();
        let cmd = BatchCommand {
            command: "task.add".into(),
            args: serde_json::json!({"title": "Auto"}),
        };
        let msg = run_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("T0001"));
    }

    #[test]
    fn batch_task_add_missing_title() {
        let mut p = test_project();
        let cmd = BatchCommand {
            command: "task.add".into(),
            args: serde_json::json!({}),
        };
        assert!(run_one(&mut p, &cmd).is_err());
    }

    #[test]
    fn batch_task_add_large_complexity_rejected() {
        let mut p = test_project();
        let cmd = BatchCommand {
            command: "task.add".into(),
            args: serde_json::json!({
                "title": "X",
                "complexity": 5_000_000_000_u64
            }),
        };
        assert!(run_one(&mut p, &cmd).is_err());
    }

    // --- execute_one: task.remove ---

    #[test]
    fn batch_task_remove() {
        let mut p = test_project();
        p.add_task(TaskId::new("A").unwrap(), Task::new("A").unwrap())
            .unwrap();
        let cmd = BatchCommand {
            command: "task.remove".into(),
            args: serde_json::json!({"id": "A"}),
        };
        let msg = run_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("Removed"));
        assert_eq!(p.task_count(), 0);
    }

    // --- execute_one: task.update ---

    #[test]
    fn batch_task_update_title() {
        let mut p = test_project();
        p.add_task(TaskId::new("A").unwrap(), Task::new("Old").unwrap())
            .unwrap();
        let cmd = BatchCommand {
            command: "task.update".into(),
            args: serde_json::json!({
                "id": "A",
                "title": "New"
            }),
        };
        run_one(&mut p, &cmd).unwrap();
        assert_eq!(p.tasks[&TaskId::new("A").unwrap()].title, "New");
    }

    #[test]
    fn batch_task_update_no_fields_rejected() {
        let mut p = test_project();
        p.add_task(TaskId::new("A").unwrap(), Task::new("X").unwrap())
            .unwrap();
        let cmd = BatchCommand {
            command: "task.update".into(),
            args: serde_json::json!({"id": "A"}),
        };
        assert!(run_one(&mut p, &cmd).is_err());
    }

    // --- execute_one: task.status ---

    #[test]
    fn batch_task_status() {
        let mut p = test_project();
        p.add_task(TaskId::new("A").unwrap(), Task::new("X").unwrap())
            .unwrap();
        let cmd = BatchCommand {
            command: "task.status".into(),
            args: serde_json::json!({
                "id": "A",
                "status": "in-progress"
            }),
        };
        let msg = run_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("IN_PROGRESS"));
    }

    #[test]
    fn batch_task_status_force() {
        let mut p = test_project();
        p.add_task(TaskId::new("A").unwrap(), Task::new("X").unwrap())
            .unwrap();
        p.set_status(&TaskId::new("A").unwrap(), Status::InProgress, false)
            .unwrap();
        p.set_status(&TaskId::new("A").unwrap(), Status::Done, false)
            .unwrap();
        let cmd = BatchCommand {
            command: "task.status".into(),
            args: serde_json::json!({
                "id": "A",
                "status": "todo",
                "force": true
            }),
        };
        run_one(&mut p, &cmd).unwrap();
    }

    // --- execute_one: task.assign/unassign ---

    #[test]
    fn batch_task_assign() {
        let mut p = test_project();
        add_test_dev(&mut p, "alice");
        p.add_task(TaskId::new("A").unwrap(), Task::new("X").unwrap())
            .unwrap();
        let cmd = BatchCommand {
            command: "task.assign".into(),
            args: serde_json::json!({
                "id": "A",
                "to": "alice"
            }),
        };
        let msg = run_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("alice"));
    }

    #[test]
    fn batch_task_unassign() {
        let mut p = test_project();
        add_test_dev(&mut p, "bob");
        p.add_task(TaskId::new("A").unwrap(), Task::new("X").unwrap())
            .unwrap();
        {
            let dev = DeveloperId::new("bob").unwrap();
            p.assign(&TaskId::new("A").unwrap(), &dev).unwrap();
        }
        let cmd = BatchCommand {
            command: "task.unassign".into(),
            args: serde_json::json!({"id": "A"}),
        };
        let msg = run_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("unassigned"));
    }

    // --- execute_one: task.depend/undepend ---

    #[test]
    fn batch_task_depend() {
        let mut p = test_project();
        p.add_task(TaskId::new("A").unwrap(), Task::new("X").unwrap())
            .unwrap();
        p.add_task(TaskId::new("B").unwrap(), Task::new("Y").unwrap())
            .unwrap();
        let cmd = BatchCommand {
            command: "task.depend".into(),
            args: serde_json::json!({
                "from": "B",
                "to": "A"
            }),
        };
        let msg = run_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("depends on"));
    }

    #[test]
    fn batch_task_undepend() {
        let mut p = test_project();
        p.add_task(TaskId::new("A").unwrap(), Task::new("X").unwrap())
            .unwrap();
        p.add_task(TaskId::new("B").unwrap(), Task::new("Y").unwrap())
            .unwrap();
        p.add_dependency(
            &TaskId::new("B").unwrap(),
            &TaskId::new("A").unwrap(),
        )
        .unwrap();
        let cmd = BatchCommand {
            command: "task.undepend".into(),
            args: serde_json::json!({
                "from": "B",
                "to": "A"
            }),
        };
        let msg = run_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("Removed"));
    }

    // --- execute_one: task.rename ---

    #[test]
    fn batch_task_rename() {
        let mut p = test_project();
        p.add_task(TaskId::new("A").unwrap(), Task::new("X").unwrap())
            .unwrap();
        let cmd = BatchCommand {
            command: "task.rename".into(),
            args: serde_json::json!({
                "old_id": "A",
                "new_id": "B"
            }),
        };
        let msg = run_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("renamed to B"));
        assert!(p.tasks.contains_key(&TaskId::new("B").unwrap()));
        assert!(!p.tasks.contains_key(&TaskId::new("A").unwrap()));
    }

    #[test]
    fn batch_task_rename_missing_old() {
        let mut p = test_project();
        let cmd = BatchCommand {
            command: "task.rename".into(),
            args: serde_json::json!({
                "old_id": "NOPE",
                "new_id": "X"
            }),
        };
        assert!(run_one(&mut p, &cmd).is_err());
    }

    #[test]
    fn batch_task_rename_missing_args() {
        let mut p = test_project();
        let cmd = BatchCommand {
            command: "task.rename".into(),
            args: serde_json::json!({"old_id": "A"}),
        };
        assert!(run_one(&mut p, &cmd).is_err());
    }

    // --- execute_one: effort.log/estimate ---

    #[test]
    fn batch_effort_log() {
        let mut p = test_project();
        p.add_task(TaskId::new("A").unwrap(), Task::new("X").unwrap())
            .unwrap();
        p.set_status(&TaskId::new("A").unwrap(), Status::InProgress, false)
            .unwrap();
        let cmd = BatchCommand {
            command: "effort.log".into(),
            args: serde_json::json!({
                "id": "A",
                "amount": "2H",
                "dev": "alice",
                "note": "some work"
            }),
        };
        let msg = run_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("2H"));
    }

    #[test]
    fn batch_effort_estimate() {
        let mut p = test_project();
        p.add_task(TaskId::new("A").unwrap(), Task::new("X").unwrap())
            .unwrap();
        let cmd = BatchCommand {
            command: "effort.estimate".into(),
            args: serde_json::json!({
                "id": "A",
                "amount": "5H"
            }),
        };
        let msg = run_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("5H"));
    }

    // --- execute_one: dev.add/remove ---

    #[test]
    fn batch_dev_add() {
        let mut p = test_project();
        let cmd = BatchCommand {
            command: "dev.add".into(),
            args: serde_json::json!({
                "id": "alice",
                "name": "Alice Smith",
                "email": "alice@example.com",
                "role": "lead"
            }),
        };
        let msg = run_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("alice"));
        assert_eq!(p.developers.len(), 1);
    }

    #[test]
    fn batch_dev_add_minimal() {
        let mut p = test_project();
        let cmd = BatchCommand {
            command: "dev.add".into(),
            args: serde_json::json!({
                "id": "bob",
                "name": "Bob"
            }),
        };
        let msg = run_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("bob"));
    }

    #[test]
    fn batch_dev_add_missing_id() {
        let mut p = test_project();
        let cmd = BatchCommand {
            command: "dev.add".into(),
            args: serde_json::json!({"name": "Alice"}),
        };
        assert!(run_one(&mut p, &cmd).is_err());
    }

    #[test]
    fn batch_dev_add_missing_name() {
        let mut p = test_project();
        let cmd = BatchCommand {
            command: "dev.add".into(),
            args: serde_json::json!({"id": "x"}),
        };
        assert!(run_one(&mut p, &cmd).is_err());
    }

    #[test]
    fn batch_dev_add_duplicate_rejected() {
        let mut p = test_project();
        add_test_dev(&mut p, "alice");
        let cmd = BatchCommand {
            command: "dev.add".into(),
            args: serde_json::json!({
                "id": "alice",
                "name": "Alice"
            }),
        };
        assert!(run_one(&mut p, &cmd).is_err());
    }

    #[test]
    fn batch_dev_remove() {
        let mut p = test_project();
        add_test_dev(&mut p, "alice");
        let cmd = BatchCommand {
            command: "dev.remove".into(),
            args: serde_json::json!({"id": "alice"}),
        };
        let msg = run_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("alice"));
        assert!(p.developers.is_empty());
    }

    #[test]
    fn batch_dev_remove_nonexistent() {
        let mut p = test_project();
        let cmd = BatchCommand {
            command: "dev.remove".into(),
            args: serde_json::json!({"id": "nobody"}),
        };
        assert!(run_one(&mut p, &cmd).is_err());
    }

    // --- execute_one: unknown command ---

    #[test]
    fn batch_unknown_command() {
        let mut p = test_project();
        let cmd = BatchCommand {
            command: "nope".into(),
            args: serde_json::json!({}),
        };
        assert!(run_one(&mut p, &cmd).is_err());
    }
}

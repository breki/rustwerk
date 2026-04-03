use std::io::Read as _;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use rustwerk::domain::project::Project;
use rustwerk::domain::task::{
    Effort, EffortEntry, Task, TaskId,
};

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

/// Execute a single batch command against a project.
fn execute_one(
    project: &mut Project,
    cmd: &BatchCommand,
) -> Result<String> {
    let args = &cmd.args;
    match cmd.command.as_str() {
        "task.add" => {
            let title = args["title"]
                .as_str()
                .context("task.add requires 'title'")?;
            let mut task = Task::new(title)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            if let Some(d) = args.get("desc").and_then(|v| v.as_str()) {
                task.description = Some(d.to_string());
            }
            if let Some(c) = args.get("complexity").and_then(|v| v.as_u64()) {
                let c = u32::try_from(c)
                    .map_err(|_| anyhow::anyhow!(
                        "complexity value too large: {c}"
                    ))?;
                task.set_complexity(c)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
            }
            if let Some(e) = args.get("effort").and_then(|v| v.as_str()) {
                task.effort_estimate = Some(
                    Effort::parse(e)
                        .map_err(|e| anyhow::anyhow!("{e}"))?,
                );
            }
            let task_id = if let Some(id_str) =
                args.get("id").and_then(|v| v.as_str())
            {
                let tid = TaskId::new(id_str)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                project
                    .add_task(tid.clone(), task)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                tid
            } else {
                project.add_task_auto(task)
            };
            Ok(format!("Created task {task_id}"))
        }
        "task.remove" => {
            let id = args["id"]
                .as_str()
                .context("task.remove requires 'id'")?;
            let task_id = TaskId::new(id)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let task = project
                .remove_task(&task_id)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            Ok(format!("Removed {task_id}: {}", task.title))
        }
        "task.update" => {
            let id = args["id"]
                .as_str()
                .context("task.update requires 'id'")?;
            let task_id = TaskId::new(id)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let title =
                args.get("title").and_then(|v| v.as_str());
            let desc =
                args.get("desc").and_then(|v| v.as_str());
            if title.is_none() && desc.is_none() {
                bail!(
                    "task.update requires at least one \
                     of 'title' or 'desc'"
                );
            }
            let description = desc.map(|d| {
                if d.is_empty() { None } else { Some(d) }
            });
            project
                .update_task(&task_id, title, description)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            Ok(format!("Updated {task_id}"))
        }
        "task.status" => {
            let id = args["id"]
                .as_str()
                .context("task.status requires 'id'")?;
            let status = args["status"]
                .as_str()
                .context("task.status requires 'status'")?;
            let force = args
                .get("force")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let task_id = TaskId::new(id)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let new_status = parse_status(status)?;
            project
                .set_status(&task_id, new_status, force)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            Ok(format!("{task_id}: {new_status}"))
        }
        "task.assign" => {
            use rustwerk::domain::developer::DeveloperId;
            let id = args["id"]
                .as_str()
                .context("task.assign requires 'id'")?;
            let to = args["to"]
                .as_str()
                .context("task.assign requires 'to'")?;
            let task_id = TaskId::new(id)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let dev_id = DeveloperId::new(to)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            project
                .assign(&task_id, &dev_id)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            Ok(format!("{task_id}: assigned to {dev_id}"))
        }
        "task.unassign" => {
            let id = args["id"]
                .as_str()
                .context("task.unassign requires 'id'")?;
            let task_id = TaskId::new(id)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            project
                .unassign(&task_id)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            Ok(format!("{task_id}: unassigned"))
        }
        "task.depend" => {
            let from = args["from"]
                .as_str()
                .context("task.depend requires 'from'")?;
            let to = args["to"]
                .as_str()
                .context("task.depend requires 'to'")?;
            let from_id = TaskId::new(from)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let to_id = TaskId::new(to)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            project
                .add_dependency(&from_id, &to_id)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            Ok(format!("{from_id} depends on {to_id}"))
        }
        "task.undepend" => {
            let from = args["from"]
                .as_str()
                .context("task.undepend requires 'from'")?;
            let to = args["to"]
                .as_str()
                .context("task.undepend requires 'to'")?;
            let from_id = TaskId::new(from)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let to_id = TaskId::new(to)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            project
                .remove_dependency(&from_id, &to_id)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            Ok(format!(
                "Removed: {from_id} depends on {to_id}"
            ))
        }
        "effort.log" => {
            let id = args["id"]
                .as_str()
                .context("effort.log requires 'id'")?;
            let amount = args["amount"]
                .as_str()
                .context("effort.log requires 'amount'")?;
            let dev = args["dev"]
                .as_str()
                .context("effort.log requires 'dev'")?;
            let note =
                args.get("note").and_then(|v| v.as_str());
            let task_id = TaskId::new(id)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let effort = Effort::parse(amount)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let entry = EffortEntry {
                effort,
                developer: dev.to_string(),
                timestamp: chrono::Utc::now(),
                note: note.map(String::from),
            };
            project
                .log_effort(&task_id, entry)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            Ok(format!("{task_id}: logged {amount}"))
        }
        "effort.estimate" => {
            let id = args["id"]
                .as_str()
                .context("effort.estimate requires 'id'")?;
            let amount = args["amount"]
                .as_str()
                .context("effort.estimate requires 'amount'")?;
            let task_id = TaskId::new(id)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let effort = Effort::parse(amount)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            project
                .set_effort_estimate(&task_id, effort)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            Ok(format!("{task_id}: estimate set to {amount}"))
        }
        other => bail!("unknown command: {other}"),
    }
}

/// Execute a batch of commands from a file or stdin.
pub(super) fn cmd_batch(
    file: Option<&str>,
) -> Result<()> {
    let json = if let Some(path) = file {
        std::fs::read_to_string(path)
            .context("failed to read batch file")?
    } else {
        let mut buf = String::new();
        std::io::stdin()
            .take(MAX_BATCH_BYTES)
            .read_to_string(&mut buf)
            .context("failed to read stdin")?;
        buf
    };

    let commands: Vec<BatchCommand> =
        serde_json::from_str(&json)
            .context("failed to parse batch JSON")?;

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

    for (i, cmd) in commands.iter().enumerate() {
        match execute_one(&mut project, cmd) {
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
                eprintln!(
                    "{}",
                    serde_json::to_string_pretty(
                        &error_output
                    )?
                );
                bail!(
                    "batch failed at command {i} ({})",
                    safe_cmd
                );
            }
        }
    }

    save_project(&root, &project)?;
    println!("{}", serde_json::to_string_pretty(&results)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustwerk::domain::task::Status;

    fn test_project() -> Project {
        Project::new("Test").unwrap()
    }

    fn add_test_dev(p: &mut Project, id: &str) {
        use rustwerk::domain::developer::{
            Developer, DeveloperId,
        };
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
        let msg = execute_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("MT"));
        assert_eq!(p.task_count(), 1);
        let task = &p.tasks[&TaskId::new("MT").unwrap()];
        assert_eq!(task.title, "My task");
        assert_eq!(task.complexity, Some(5));
        assert_eq!(
            task.description.as_deref(),
            Some("A description")
        );
    }

    #[test]
    fn batch_task_add_auto_id() {
        let mut p = test_project();
        let cmd = BatchCommand {
            command: "task.add".into(),
            args: serde_json::json!({"title": "Auto"}),
        };
        let msg = execute_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("T0001"));
    }

    #[test]
    fn batch_task_add_missing_title() {
        let mut p = test_project();
        let cmd = BatchCommand {
            command: "task.add".into(),
            args: serde_json::json!({}),
        };
        assert!(execute_one(&mut p, &cmd).is_err());
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
        assert!(execute_one(&mut p, &cmd).is_err());
    }

    // --- execute_one: task.remove ---

    #[test]
    fn batch_task_remove() {
        let mut p = test_project();
        p.add_task(
            TaskId::new("A").unwrap(),
            Task::new("A").unwrap(),
        )
        .unwrap();
        let cmd = BatchCommand {
            command: "task.remove".into(),
            args: serde_json::json!({"id": "A"}),
        };
        let msg = execute_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("Removed"));
        assert_eq!(p.task_count(), 0);
    }

    // --- execute_one: task.update ---

    #[test]
    fn batch_task_update_title() {
        let mut p = test_project();
        p.add_task(
            TaskId::new("A").unwrap(),
            Task::new("Old").unwrap(),
        )
        .unwrap();
        let cmd = BatchCommand {
            command: "task.update".into(),
            args: serde_json::json!({
                "id": "A",
                "title": "New"
            }),
        };
        execute_one(&mut p, &cmd).unwrap();
        assert_eq!(
            p.tasks[&TaskId::new("A").unwrap()].title,
            "New"
        );
    }

    #[test]
    fn batch_task_update_no_fields_rejected() {
        let mut p = test_project();
        p.add_task(
            TaskId::new("A").unwrap(),
            Task::new("X").unwrap(),
        )
        .unwrap();
        let cmd = BatchCommand {
            command: "task.update".into(),
            args: serde_json::json!({"id": "A"}),
        };
        assert!(execute_one(&mut p, &cmd).is_err());
    }

    // --- execute_one: task.status ---

    #[test]
    fn batch_task_status() {
        let mut p = test_project();
        p.add_task(
            TaskId::new("A").unwrap(),
            Task::new("X").unwrap(),
        )
        .unwrap();
        let cmd = BatchCommand {
            command: "task.status".into(),
            args: serde_json::json!({
                "id": "A",
                "status": "in-progress"
            }),
        };
        let msg = execute_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("IN_PROGRESS"));
    }

    #[test]
    fn batch_task_status_force() {
        let mut p = test_project();
        p.add_task(
            TaskId::new("A").unwrap(),
            Task::new("X").unwrap(),
        )
        .unwrap();
        p.set_status(
            &TaskId::new("A").unwrap(),
            Status::InProgress,
            false,
        )
        .unwrap();
        p.set_status(
            &TaskId::new("A").unwrap(),
            Status::Done,
            false,
        )
        .unwrap();
        let cmd = BatchCommand {
            command: "task.status".into(),
            args: serde_json::json!({
                "id": "A",
                "status": "todo",
                "force": true
            }),
        };
        execute_one(&mut p, &cmd).unwrap();
    }

    // --- execute_one: task.assign/unassign ---

    #[test]
    fn batch_task_assign() {
        let mut p = test_project();
        add_test_dev(&mut p, "alice");
        p.add_task(
            TaskId::new("A").unwrap(),
            Task::new("X").unwrap(),
        )
        .unwrap();
        let cmd = BatchCommand {
            command: "task.assign".into(),
            args: serde_json::json!({
                "id": "A",
                "to": "alice"
            }),
        };
        let msg = execute_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("alice"));
    }

    #[test]
    fn batch_task_unassign() {
        let mut p = test_project();
        add_test_dev(&mut p, "bob");
        p.add_task(
            TaskId::new("A").unwrap(),
            Task::new("X").unwrap(),
        )
        .unwrap();
        {
            use rustwerk::domain::developer::DeveloperId;
            let dev =
                DeveloperId::new("bob").unwrap();
            p.assign(&TaskId::new("A").unwrap(), &dev)
                .unwrap();
        }
        let cmd = BatchCommand {
            command: "task.unassign".into(),
            args: serde_json::json!({"id": "A"}),
        };
        let msg = execute_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("unassigned"));
    }

    // --- execute_one: task.depend/undepend ---

    #[test]
    fn batch_task_depend() {
        let mut p = test_project();
        p.add_task(
            TaskId::new("A").unwrap(),
            Task::new("X").unwrap(),
        )
        .unwrap();
        p.add_task(
            TaskId::new("B").unwrap(),
            Task::new("Y").unwrap(),
        )
        .unwrap();
        let cmd = BatchCommand {
            command: "task.depend".into(),
            args: serde_json::json!({
                "from": "B",
                "to": "A"
            }),
        };
        let msg = execute_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("depends on"));
    }

    #[test]
    fn batch_task_undepend() {
        let mut p = test_project();
        p.add_task(
            TaskId::new("A").unwrap(),
            Task::new("X").unwrap(),
        )
        .unwrap();
        p.add_task(
            TaskId::new("B").unwrap(),
            Task::new("Y").unwrap(),
        )
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
        let msg = execute_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("Removed"));
    }

    // --- execute_one: effort.log/estimate ---

    #[test]
    fn batch_effort_log() {
        let mut p = test_project();
        p.add_task(
            TaskId::new("A").unwrap(),
            Task::new("X").unwrap(),
        )
        .unwrap();
        p.set_status(
            &TaskId::new("A").unwrap(),
            Status::InProgress,
            false,
        )
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
        let msg = execute_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("2H"));
    }

    #[test]
    fn batch_effort_estimate() {
        let mut p = test_project();
        p.add_task(
            TaskId::new("A").unwrap(),
            Task::new("X").unwrap(),
        )
        .unwrap();
        let cmd = BatchCommand {
            command: "effort.estimate".into(),
            args: serde_json::json!({
                "id": "A",
                "amount": "5H"
            }),
        };
        let msg = execute_one(&mut p, &cmd).unwrap();
        assert!(msg.contains("5H"));
    }

    // --- execute_one: unknown command ---

    #[test]
    fn batch_unknown_command() {
        let mut p = test_project();
        let cmd = BatchCommand {
            command: "nope".into(),
            args: serde_json::json!({}),
        };
        assert!(execute_one(&mut p, &cmd).is_err());
    }
}

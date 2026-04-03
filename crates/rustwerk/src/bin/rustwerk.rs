use std::env;
use std::io::Read as _;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

use rustwerk::domain::project::Project;
use rustwerk::domain::task::{
    Effort, EffortEntry, Status, Task, TaskId,
};
use rustwerk::persistence::file_store;

#[derive(Parser)]
#[command(
    name = "rustwerk",
    about = "Git-native, AI-agent-friendly project \
             orchestration CLI"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new project in the current directory.
    Init {
        /// Project name.
        name: String,
    },
    /// Show project summary.
    Show,
    /// Task management.
    Task {
        #[command(subcommand)]
        action: TaskAction,
    },
    /// Effort tracking.
    Effort {
        #[command(subcommand)]
        action: EffortAction,
    },
    /// Execute a batch of commands atomically from JSON.
    /// Reads from --file or stdin. All-or-nothing: saves
    /// only if every command succeeds.
    Batch {
        /// Path to JSON file (reads stdin if omitted).
        #[arg(long)]
        file: Option<String>,
    },
    /// Show ASCII Gantt chart of task schedule.
    Gantt,
}

#[derive(Subcommand)]
enum EffortAction {
    /// Log effort on a task (must be IN_PROGRESS).
    Log {
        /// Task ID.
        id: String,
        /// Effort amount (e.g. "2.5H", "1D").
        amount: String,
        /// Developer name.
        #[arg(long)]
        dev: String,
        /// Optional note.
        #[arg(long)]
        note: Option<String>,
    },
    /// Set effort estimate for a task.
    Estimate {
        /// Task ID.
        id: String,
        /// Effort estimate (e.g. "8H", "2D").
        amount: String,
    },
}

#[derive(Subcommand)]
enum TaskAction {
    /// Add a new task.
    Add {
        /// Task title.
        title: String,
        /// Optional mnemonic task ID (auto-generated if
        /// omitted).
        #[arg(long)]
        id: Option<String>,
        /// Optional description.
        #[arg(long)]
        desc: Option<String>,
        /// Optional complexity (e.g. Fibonacci: 1,2,3,5,8).
        #[arg(long)]
        complexity: Option<u32>,
        /// Optional effort estimate (e.g. "5H", "1D").
        #[arg(long)]
        effort: Option<String>,
    },
    /// Set task status.
    Status {
        /// Task ID.
        id: String,
        /// New status: todo, in-progress, blocked, done.
        status: String,
        /// Bypass transition validation.
        #[arg(long)]
        force: bool,
    },
    /// List all tasks.
    List {
        /// Show only tasks available to work on (all
        /// dependencies done, not itself done).
        #[arg(long)]
        available: bool,
    },
    /// Remove a task.
    Remove {
        /// Task ID to remove.
        id: String,
    },
    /// Update a task's title or description.
    Update {
        /// Task ID.
        id: String,
        /// New title.
        #[arg(long)]
        title: Option<String>,
        /// New description (use "" to clear).
        #[arg(long)]
        desc: Option<String>,
    },
    /// Assign a developer to a task.
    Assign {
        /// Task ID.
        id: String,
        /// Developer name.
        to: String,
    },
    /// Remove the assignee from a task.
    Unassign {
        /// Task ID.
        id: String,
    },
    /// Add a dependency: FROM depends on TO.
    Depend {
        /// Task that depends on another.
        from: String,
        /// Task that must be completed first.
        to: String,
    },
    /// Remove a dependency.
    Undepend {
        /// Task that depends on another.
        from: String,
        /// Dependency to remove.
        to: String,
    },
}

/// Find the project root by looking for `.rustwerk/`
/// starting from the current directory and walking up.
fn find_project_root() -> Result<PathBuf> {
    let mut dir = env::current_dir()
        .context("failed to get current directory")?;
    loop {
        if dir.join(".rustwerk").is_dir() {
            return Ok(dir);
        }
        if !dir.pop() {
            bail!(
                "not a rustwerk project (no .rustwerk/ \
                 directory found)"
            );
        }
    }
}

/// Load the project from the nearest root.
fn load_project() -> Result<(PathBuf, Project)> {
    let root = find_project_root()?;
    let project = file_store::load(&root)
        .context("failed to load project")?;
    Ok((root, project))
}

/// Save the project back to disk.
fn save_project(
    root: &std::path::Path,
    project: &Project,
) -> Result<()> {
    file_store::save(root, project)
        .context("failed to save project")
}

fn cmd_init(name: &str) -> Result<()> {
    let root = env::current_dir()
        .context("failed to get current directory")?;
    let path = file_store::project_file_path(&root);
    if path.exists() {
        bail!(
            "project already exists: {}",
            path.display()
        );
    }
    let project = Project::new(name)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    file_store::save(&root, &project)
        .context("failed to save project")?;
    println!("Initialized project: {name}");
    println!("  {}", path.display());
    Ok(())
}

fn cmd_show() -> Result<()> {
    let (_root, project) = load_project()?;
    println!("Project: {}", project.metadata.name);
    if let Some(desc) = &project.metadata.description {
        println!("  {desc}");
    }

    let s = project.summary();
    println!();
    println!(
        "Tasks:    {} total  ({} done, {} in-progress, \
         {} todo, {} blocked)",
        s.total, s.done, s.in_progress, s.todo, s.blocked
    );
    println!("Complete: {:.0}%", s.pct_complete);
    if s.total_complexity > 0 {
        println!("Complexity: {} total", s.total_complexity);
    }
    if s.total_estimated_hours > 0.0
        || s.total_actual_hours > 0.0
    {
        println!(
            "Effort:   {:.1}H estimated, {:.1}H actual",
            s.total_estimated_hours, s.total_actual_hours
        );
    }
    println!(
        "Created:  {}",
        project
            .metadata
            .created_at
            .format("%Y-%m-%d %H:%M UTC")
    );
    Ok(())
}

fn cmd_task_add(
    title: &str,
    id: Option<&str>,
    desc: Option<&str>,
    complexity: Option<u32>,
    effort: Option<&str>,
) -> Result<()> {
    let (root, mut project) = load_project()?;
    let mut task = Task::new(title)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    task.description = desc.map(String::from);
    if let Some(c) = complexity {
        task.set_complexity(c)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    if let Some(e) = effort {
        task.effort_estimate = Some(
            Effort::parse(e)
                .map_err(|e| anyhow::anyhow!("{e}"))?,
        );
    }

    let task_id = if let Some(id_str) = id {
        let tid = TaskId::new(id_str)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        project
            .add_task(tid.clone(), task)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        tid
    } else {
        project.add_task_auto(task)
    };

    save_project(&root, &project)?;
    println!("Created task {task_id}");
    Ok(())
}

fn cmd_task_assign(id: &str, to: &str) -> Result<()> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    project
        .assign(&task_id, to)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    save_project(&root, &project)?;
    println!("{task_id}: assigned to {to}");
    Ok(())
}

fn cmd_task_unassign(id: &str) -> Result<()> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    project
        .unassign(&task_id)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    save_project(&root, &project)?;
    println!("{task_id}: unassigned");
    Ok(())
}

fn cmd_effort_log(
    id: &str,
    amount: &str,
    dev: &str,
    note: Option<&str>,
) -> Result<()> {
    let (root, mut project) = load_project()?;
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
    save_project(&root, &project)?;
    let task = &project.tasks[&task_id];
    println!(
        "{task_id}: logged {amount} (total: {:.1}H)",
        task.total_actual_effort_hours()
    );
    Ok(())
}

fn cmd_effort_estimate(
    id: &str,
    amount: &str,
) -> Result<()> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let effort = Effort::parse(amount)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    project
        .set_effort_estimate(&task_id, effort)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    save_project(&root, &project)?;
    println!("{task_id}: estimate set to {amount}");
    Ok(())
}

fn cmd_task_remove(id: &str) -> Result<()> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let task = project
        .remove_task(&task_id)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    save_project(&root, &project)?;
    println!("Removed task {task_id}: {}", task.title);
    Ok(())
}

fn cmd_task_update(
    id: &str,
    title: Option<&str>,
    desc: Option<&str>,
) -> Result<()> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    // Empty string for desc means clear it.
    let description = desc.map(|d| {
        if d.is_empty() {
            None
        } else {
            Some(d)
        }
    });
    project
        .update_task(&task_id, title, description)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    save_project(&root, &project)?;
    let task = &project.tasks[&task_id];
    println!("Updated {task_id}: {}", task.title);
    Ok(())
}

fn parse_status(s: &str) -> Result<Status> {
    match s.to_lowercase().as_str() {
        "todo" => Ok(Status::Todo),
        "in-progress" | "in_progress" | "inprogress" => {
            Ok(Status::InProgress)
        }
        "blocked" => Ok(Status::Blocked),
        "done" => Ok(Status::Done),
        _ => bail!(
            "unknown status: {s} (expected: todo, \
             in-progress, blocked, done)"
        ),
    }
}

fn cmd_task_status(
    id: &str,
    status: &str,
    force: bool,
) -> Result<()> {
    let (root, mut project) = load_project()?;
    let task_id = TaskId::new(id)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let new_status = parse_status(status)?;
    project
        .set_status(&task_id, new_status, force)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    save_project(&root, &project)?;
    println!("{task_id}: {new_status}");
    Ok(())
}

// NOTE: task titles are printed verbatim. A crafted
// project.json could contain ANSI escape sequences that
// affect terminal rendering. Sanitization should be added
// before this is used in untrusted environments.
fn cmd_task_list(available_only: bool) -> Result<()> {
    let (_root, project) = load_project()?;
    if project.tasks.is_empty() {
        println!("No tasks.");
        return Ok(());
    }

    let crit = project.critical_path_set();

    if available_only {
        let avail = project.available_tasks();
        if avail.is_empty() {
            println!("No available tasks.");
            return Ok(());
        }
        for id in &avail {
            let task = &project.tasks[*id];
            let complexity = task
                .complexity
                .map_or(String::new(), |c| {
                    format!(" [{c}]")
                });
            let marker =
                if crit.contains(*id) { "*" } else { " " };
            println!(
                " {marker}{id:<16} {}{complexity}",
                task.title,
            );
        }
    } else {
        for (id, task) in &project.tasks {
            let complexity = task
                .complexity
                .map_or(String::new(), |c| {
                    format!(" [{c}]")
                });
            let marker =
                if crit.contains(id) { "*" } else { " " };
            println!(
                " {marker}{id:<16} {:<14} {}{complexity}",
                task.status, task.title,
            );
        }
    }
    Ok(())
}

fn cmd_depend(from: &str, to: &str) -> Result<()> {
    let (root, mut project) = load_project()?;
    let from_id = TaskId::new(from)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let to_id = TaskId::new(to)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    project
        .add_dependency(&from_id, &to_id)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    save_project(&root, &project)?;
    println!("{from_id} depends on {to_id}");
    Ok(())
}

fn cmd_undepend(from: &str, to: &str) -> Result<()> {
    let (root, mut project) = load_project()?;
    let from_id = TaskId::new(from)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let to_id = TaskId::new(to)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    project
        .remove_dependency(&from_id, &to_id)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    save_project(&root, &project)?;
    println!("Removed: {from_id} depends on {to_id}");
    Ok(())
}

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
            let id = args["id"]
                .as_str()
                .context("task.assign requires 'id'")?;
            let to = args["to"]
                .as_str()
                .context("task.assign requires 'to'")?;
            let task_id = TaskId::new(id)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            project
                .assign(&task_id, to)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            Ok(format!("{task_id}: assigned to {to}"))
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

/// Maximum batch input size (10 MB).
const MAX_BATCH_BYTES: u64 = 10 * 1024 * 1024;

/// Maximum number of commands in a single batch.
const MAX_BATCH_COMMANDS: usize = 1000;

fn cmd_gantt() -> Result<()> {
    let (_root, project) = load_project()?;
    let rows = project.gantt_schedule();

    if rows.is_empty() {
        println!("No tasks.");
        return Ok(());
    }

    let max_end = rows
        .iter()
        .map(|r| r.end())
        .max()
        .unwrap_or(0);

    // Find the longest ID for padding.
    let id_width = rows
        .iter()
        .map(|r| r.id.as_str().len())
        .max()
        .unwrap_or(8)
        .max(8);

    // Header with scale.
    let label_width = id_width + 2; // marker + id + space
    print!("{:width$}", "", width = label_width);
    for i in (0..max_end).step_by(5) {
        print!("{i:<5}");
    }
    println!();
    print!("{:width$}", "", width = label_width);
    for i in 0..max_end {
        if i % 5 == 0 {
            print!("|");
        } else {
            print!(" ");
        }
    }
    println!();

    // Rows — bar rendering uses domain methods.
    for row in &rows {
        let marker =
            if row.critical { "*" } else { " " };
        let (filled, empty) = row.bar_fill();
        let fill_ch = row.fill_char();
        let empty_ch = row.empty_char();
        let bar = format!(
            "{}{}",
            std::iter::repeat_n(fill_ch, filled as usize)
                .collect::<String>(),
            std::iter::repeat_n(
                empty_ch, empty as usize
            )
            .collect::<String>(),
        );

        let padding =
            " ".repeat(row.start as usize);
        print!(
            "{marker}{:<width$} {padding}[{bar}]",
            row.id,
            width = id_width,
        );
        println!();
    }

    Ok(())
}

fn cmd_batch(file: Option<&str>) -> Result<()> {
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init { name } => cmd_init(&name),
        Commands::Show => cmd_show(),
        Commands::Task { action } => match action {
            TaskAction::Add {
                title,
                id,
                desc,
                complexity,
                effort,
            } => cmd_task_add(
                &title,
                id.as_deref(),
                desc.as_deref(),
                complexity,
                effort.as_deref(),
            ),
            TaskAction::Status { id, status, force } => {
                cmd_task_status(&id, &status, force)
            }
            TaskAction::Remove { id } => {
                cmd_task_remove(&id)
            }
            TaskAction::Assign { id, to } => {
                cmd_task_assign(&id, &to)
            }
            TaskAction::Unassign { id } => {
                cmd_task_unassign(&id)
            }
            TaskAction::Update { id, title, desc } => {
                cmd_task_update(
                    &id,
                    title.as_deref(),
                    desc.as_deref(),
                )
            }
            TaskAction::List { available } => {
                cmd_task_list(available)
            }
            TaskAction::Depend { from, to } => {
                cmd_depend(&from, &to)
            }
            TaskAction::Undepend { from, to } => {
                cmd_undepend(&from, &to)
            }
        },
        Commands::Batch { file } => {
            cmd_batch(file.as_deref())
        }
        Commands::Gantt => cmd_gantt(),
        Commands::Effort { action } => match action {
            EffortAction::Log {
                id,
                amount,
                dev,
                note,
            } => cmd_effort_log(
                &id,
                &amount,
                &dev,
                note.as_deref(),
            ),
            EffortAction::Estimate { id, amount } => {
                cmd_effort_estimate(&id, &amount)
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustwerk::domain::project::Project;

    fn test_project() -> Project {
        Project::new("Test").unwrap()
    }

    // --- parse_status ---

    #[test]
    fn parse_status_all_variants() {
        assert!(matches!(
            parse_status("todo").unwrap(),
            Status::Todo
        ));
        assert!(matches!(
            parse_status("in-progress").unwrap(),
            Status::InProgress
        ));
        assert!(matches!(
            parse_status("in_progress").unwrap(),
            Status::InProgress
        ));
        assert!(matches!(
            parse_status("inprogress").unwrap(),
            Status::InProgress
        ));
        assert!(matches!(
            parse_status("blocked").unwrap(),
            Status::Blocked
        ));
        assert!(matches!(
            parse_status("done").unwrap(),
            Status::Done
        ));
        assert!(matches!(
            parse_status("TODO").unwrap(),
            Status::Todo
        ));
    }

    #[test]
    fn parse_status_unknown() {
        assert!(parse_status("invalid").is_err());
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
        p.add_task(
            TaskId::new("A").unwrap(),
            Task::new("X").unwrap(),
        )
        .unwrap();
        p.assign(&TaskId::new("A").unwrap(), "bob")
            .unwrap();
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

use std::env;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use rustwerk::domain::project::Project;
use rustwerk::domain::task::{Effort, Status, Task, TaskId};
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
    println!("Tasks:   {}", project.task_count());
    println!(
        "Created: {}",
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
    task.complexity = complexity;
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
    }
}

mod batch;
mod commands;
mod gantt;
mod render;
mod tree;

use std::env;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use rustwerk::domain::project::Project;
use rustwerk::domain::task::Status;
use rustwerk::persistence::file_store;

use batch::cmd_batch;

/// Resolve a developer ID from an explicit argument or
/// the `RUSTWERK_USER` environment variable.
fn resolve_developer(explicit: Option<String>) -> Result<String> {
    explicit
        .or_else(|| env::var("RUSTWERK_USER").ok())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no developer specified and \
                 RUSTWERK_USER is not set"
            )
        })
}

use commands::{
    cmd_depend, cmd_dev_add, cmd_dev_list, cmd_dev_remove, cmd_effort_estimate,
    cmd_effort_log, cmd_init, cmd_report_bottlenecks, cmd_report_complete,
    cmd_report_effort, cmd_show, cmd_status, cmd_task_add, cmd_task_assign,
    cmd_task_describe, cmd_task_list, cmd_task_remove, cmd_task_rename,
    cmd_task_status, cmd_task_unassign, cmd_task_update, cmd_undepend,
};
use gantt::cmd_gantt;

#[derive(Parser)]
#[command(
    name = "rustwerk",
    version,
    about = "Git-native, AI-agent-friendly project \
             orchestration CLI"
)]
struct Cli {
    /// Emit machine-readable JSON to stdout instead of
    /// human-readable text. Available on every command.
    #[arg(long, global = true)]
    json: bool,
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
    /// Developer management.
    Dev {
        #[command(subcommand)]
        action: DevAction,
    },
    /// Project reports.
    Report {
        #[command(subcommand)]
        action: ReportAction,
    },
    /// Show ASCII Gantt chart of task schedule.
    Gantt {
        /// Show only tasks that are not done.
        #[arg(long)]
        remaining: bool,
    },
    /// Show compact project status dashboard.
    Status,
    /// Show ASCII dependency tree.
    Tree {
        /// Show only remaining (not done/on-hold) tasks.
        #[arg(long)]
        remaining: bool,
    },
}

#[derive(Subcommand)]
enum DevAction {
    /// Add a developer to the project.
    Add {
        /// Developer ID (short username, lowercase).
        id: String,
        /// Full name.
        name: String,
        /// Email address.
        #[arg(long)]
        email: Option<String>,
        /// Role (e.g. "lead", "developer").
        #[arg(long)]
        role: Option<String>,
    },
    /// Remove a developer from the project.
    Remove {
        /// Developer ID to remove.
        id: String,
    },
    /// List all developers in the project.
    List,
}

#[derive(Subcommand)]
enum ReportAction {
    /// PM completion summary (counts, %, estimated vs
    /// actual effort, critical path).
    Complete,
    /// Effort breakdown per developer.
    Effort,
    /// Bottleneck tasks (most downstream dependents).
    Bottlenecks,
}

#[derive(Subcommand)]
enum EffortAction {
    /// Log effort on a task (must be `IN_PROGRESS`).
    Log {
        /// Task ID.
        id: String,
        /// Effort amount (e.g. "2.5H", "1D").
        amount: String,
        /// Developer ID (defaults to `RUSTWERK_USER`).
        #[arg(long)]
        dev: Option<String>,
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
        /// Comma-separated tags (slug-like: lowercase
        /// alphanumeric and hyphens).
        #[arg(long)]
        tags: Option<String>,
    },
    /// Set task status.
    Status {
        /// Task ID.
        id: String,
        /// New status: todo, in-progress, blocked, done,
        /// on-hold.
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
        /// Show only tasks currently in progress.
        #[arg(long, conflicts_with = "available")]
        active: bool,
        /// Filter by status (todo, in-progress, blocked,
        /// done, on-hold).
        #[arg(long, conflicts_with_all = ["available", "active"])]
        status: Option<String>,
        /// Filter by assignee developer ID.
        #[arg(long)]
        assignee: Option<String>,
        /// Show dependency chain for a task (the task
        /// and all its transitive dependencies).
        #[arg(long)]
        chain: Option<String>,
        /// Filter by tag (show only tasks with this tag).
        #[arg(long)]
        tag: Option<String>,
    },
    /// Remove a task.
    Remove {
        /// Task ID to remove.
        id: String,
    },
    /// Rename a task (change its ID). Updates all
    /// dependency references and renames the description
    /// file.
    Rename {
        /// Current task ID.
        old_id: String,
        /// New task ID.
        new_id: String,
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
        /// Comma-separated tags (replaces all existing
        /// tags; use "" to clear).
        #[arg(long)]
        tags: Option<String>,
    },
    /// Assign a developer to a task. Falls back to the
    /// `RUSTWERK_USER` environment variable when no
    /// developer is specified.
    Assign {
        /// Task ID.
        id: String,
        /// Developer ID (defaults to `RUSTWERK_USER`).
        to: Option<String>,
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
    /// Show the description file for a task
    /// (.rustwerk/tasks/<ID>.md).
    Describe {
        /// Task ID.
        id: String,
    },
}

/// Find the project root by looking for `.rustwerk/`
/// starting from the current directory and walking up.
fn find_project_root() -> Result<PathBuf> {
    let mut dir =
        env::current_dir().context("failed to get current directory")?;
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
pub(crate) fn load_project() -> Result<(PathBuf, Project)> {
    let root = find_project_root()?;
    let project = file_store::load(&root).context("failed to load project")?;
    Ok((root, project))
}

/// Save the project back to disk.
pub(crate) fn save_project(
    root: &std::path::Path,
    project: &Project,
) -> Result<()> {
    file_store::save(root, project).context("failed to save project")
}

/// Parse a status string into a `Status` enum.
pub(crate) fn parse_status(s: &str) -> Result<Status> {
    match s.to_lowercase().as_str() {
        "todo" => Ok(Status::Todo),
        "in-progress" | "in_progress" | "inprogress" => Ok(Status::InProgress),
        "blocked" => Ok(Status::Blocked),
        "done" => Ok(Status::Done),
        "on-hold" | "on_hold" | "onhold" => Ok(Status::OnHold),
        _ => bail!(
            "unknown status: {s} (expected: todo, \
             in-progress, blocked, done, on-hold)"
        ),
    }
}

fn dispatch_task(action: TaskAction, fmt: render::OutputFormat) -> Result<()> {
    match action {
        TaskAction::Add {
            title,
            id,
            desc,
            complexity,
            effort,
            tags,
        } => render::emit(
            &cmd_task_add(
                &title,
                id.as_deref(),
                desc.as_deref(),
                complexity,
                effort.as_deref(),
                tags.as_deref(),
            )?,
            fmt,
        ),
        TaskAction::Status { id, status, force } => {
            render::emit(&cmd_task_status(&id, &status, force)?, fmt)
        }
        TaskAction::Remove { id } => {
            render::emit(&cmd_task_remove(&id)?, fmt)
        }
        TaskAction::Rename { old_id, new_id } => {
            render::emit(&cmd_task_rename(&old_id, &new_id)?, fmt)
        }
        TaskAction::Assign { id, to } => {
            let dev = resolve_developer(to)?;
            render::emit(&cmd_task_assign(&id, &dev)?, fmt)
        }
        TaskAction::Unassign { id } => {
            render::emit(&cmd_task_unassign(&id)?, fmt)
        }
        TaskAction::Update {
            id,
            title,
            desc,
            tags,
        } => render::emit(
            &cmd_task_update(
                &id,
                title.as_deref(),
                desc.as_deref(),
                tags.as_deref(),
            )?,
            fmt,
        ),
        TaskAction::List {
            available,
            active,
            status,
            assignee,
            chain,
            tag,
        } => render::emit(
            &cmd_task_list(
                available,
                active,
                status.as_deref(),
                assignee.as_deref(),
                chain.as_deref(),
                tag.as_deref(),
            )?,
            fmt,
        ),
        TaskAction::Depend { from, to } => {
            render::emit(&cmd_depend(&from, &to)?, fmt)
        }
        TaskAction::Undepend { from, to } => {
            render::emit(&cmd_undepend(&from, &to)?, fmt)
        }
        TaskAction::Describe { id } => {
            render::emit(&cmd_task_describe(&id)?, fmt)
        }
    }
}

fn dispatch_dev(action: DevAction, fmt: render::OutputFormat) -> Result<()> {
    match action {
        DevAction::Add {
            id,
            name,
            email,
            role,
        } => render::emit(
            &cmd_dev_add(&id, &name, email.as_deref(), role.as_deref())?,
            fmt,
        ),
        DevAction::Remove { id } => render::emit(&cmd_dev_remove(&id)?, fmt),
        DevAction::List => render::emit(&cmd_dev_list()?, fmt),
    }
}

fn dispatch_effort(
    action: EffortAction,
    fmt: render::OutputFormat,
) -> Result<()> {
    match action {
        EffortAction::Log {
            id,
            amount,
            dev,
            note,
        } => {
            let dev = resolve_developer(dev)?;
            render::emit(
                &cmd_effort_log(&id, &amount, &dev, note.as_deref())?,
                fmt,
            )
        }
        EffortAction::Estimate { id, amount } => {
            render::emit(&cmd_effort_estimate(&id, &amount)?, fmt)
        }
    }
}

fn dispatch_report(
    action: &ReportAction,
    fmt: render::OutputFormat,
) -> Result<()> {
    match action {
        ReportAction::Complete => render::emit(&cmd_report_complete()?, fmt),
        ReportAction::Effort => render::emit(&cmd_report_effort()?, fmt),
        ReportAction::Bottlenecks => {
            render::emit(&cmd_report_bottlenecks()?, fmt)
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let fmt = render::OutputFormat::from_json_flag(cli.json);
    match cli.command {
        Commands::Init { name } => render::emit(&cmd_init(&name)?, fmt),
        Commands::Show => render::emit(&cmd_show()?, fmt),
        Commands::Task { action } => dispatch_task(action, fmt),
        Commands::Dev { action } => dispatch_dev(action, fmt),
        Commands::Report { action } => dispatch_report(&action, fmt),
        Commands::Batch { file } => {
            if cli.json {
                anyhow::bail!(
                    "`batch` always emits JSON; --json is redundant"
                );
            }
            cmd_batch(file.as_deref())
        }
        Commands::Gantt { remaining } => {
            render::emit(&cmd_gantt(remaining)?, fmt)
        }
        Commands::Status => render::emit(&cmd_status()?, fmt),
        Commands::Tree { remaining } => {
            render::emit(&tree::cmd_tree(remaining)?, fmt)
        }
        Commands::Effort { action } => dispatch_effort(action, fmt),
    }
}

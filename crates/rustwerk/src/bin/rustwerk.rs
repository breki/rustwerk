use std::env;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use rustwerk::domain::project::Project;
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
    let root = find_project_root()?;
    let project = file_store::load(&root)
        .context("failed to load project")?;

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

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init { name } => cmd_init(&name),
        Commands::Show => cmd_show(),
    }
}

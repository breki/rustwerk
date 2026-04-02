use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "rustwerk",
    about = "Git-native, AI-agent-friendly project orchestration CLI"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new project
    Init {
        /// Project name
        name: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { name } => {
            println!("Initialized project: {name}");
        }
    }
}

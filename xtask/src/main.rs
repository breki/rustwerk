use clap::{Parser, Subcommand};
use std::process::Command;

#[derive(Parser)]
#[command(name = "xtask")]
struct Cli {
    #[command(subcommand)]
    command: XCommand,
}

#[derive(Subcommand)]
enum XCommand {
    /// Run clippy (deny warnings)
    Clippy,
    /// Run all tests
    Test {
        /// Optional test filter
        filter: Option<String>,
    },
    /// Run clippy + tests
    Validate,
    /// Format code
    Fmt,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        XCommand::Clippy => run_clippy(),
        XCommand::Test { filter } => run_test(filter),
        XCommand::Validate => {
            run_clippy().and_then(|_| run_test(None))
        }
        XCommand::Fmt => run_fmt(),
    };

    if let Err(e) = result {
        eprintln!("xtask error: {e}");
        std::process::exit(1);
    }
}

fn run_clippy() -> Result<(), String> {
    run_cmd(
        &cargo_bin(),
        &["clippy", "--workspace", "--", "-D", "warnings"],
    )
}

fn run_test(filter: Option<String>) -> Result<(), String> {
    let mut args = vec!["test", "--workspace"];
    let filter_owned;
    if let Some(f) = &filter {
        if f.is_empty() {
            return Err("test filter must not be empty".into());
        }
        filter_owned = f.clone();
        args.push("--");
        args.push(&filter_owned);
    }
    run_cmd(&cargo_bin(), &args)
}

fn run_fmt() -> Result<(), String> {
    run_cmd(&cargo_bin(), &["fmt", "--all"])
}

/// Resolve the cargo binary path. Prefers the `CARGO` env var
/// (set by cargo when running xtask) over a PATH lookup.
fn cargo_bin() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".into())
}

fn run_cmd(cmd: &str, args: &[&str]) -> Result<(), String> {
    println!("→ {cmd} {}", args.join(" "));
    let status = Command::new(cmd)
        .args(args)
        .status()
        .map_err(|e| format!("failed to run {cmd}: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        match status.code() {
            Some(code) => Err(format!("{cmd} exited with {code}")),
            None => Err(format!("{cmd} terminated by signal")),
        }
    }
}

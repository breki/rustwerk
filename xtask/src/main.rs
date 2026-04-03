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
    /// Run clippy + tests + coverage check
    Validate,
    /// Format code
    Fmt,
    /// Run coverage check (requires cargo-llvm-cov)
    Coverage,
}

/// Minimum line coverage percentage.
const COVERAGE_THRESHOLD: f64 = 90.0;

/// Maximum allowed exact duplication percentage
/// (production code only, tests excluded).
const DUPLICATION_THRESHOLD: f64 = 6.0;

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        XCommand::Clippy => run_clippy(),
        XCommand::Test { filter } => run_test(filter),
        XCommand::Validate => run_clippy()
            .and_then(|_| run_test(None))
            .and_then(|_| run_coverage())
            .and_then(|_| run_dupes()),
        XCommand::Fmt => run_fmt(),
        XCommand::Coverage => run_coverage(),
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
            return Err(
                "test filter must not be empty".into(),
            );
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

fn run_coverage() -> Result<(), String> {
    println!("→ checking coverage (threshold: {COVERAGE_THRESHOLD}%)");
    let output = Command::new(cargo_bin())
        .args([
            "llvm-cov",
            "--package",
            "rustwerk",
            "--json",
            "--summary-only",
        ])
        .output()
        .map_err(|e| {
            format!(
                "failed to run cargo llvm-cov: {e}. \
                 Install with: cargo install cargo-llvm-cov"
            )
        })?;

    if !output.status.success() {
        let stderr =
            String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "cargo llvm-cov failed:\n{stderr}"
        ));
    }

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(
            |e| format!("failed to parse coverage JSON: {e}"),
        )?;

    // Extract total line coverage percentage.
    let line_pct = json["data"][0]["totals"]["lines"]
        ["percent"]
        .as_f64()
        .ok_or("missing lines.percent in coverage JSON")?;

    let covered = json["data"][0]["totals"]["lines"]
        ["covered"]
        .as_u64()
        .ok_or(
            "missing lines.covered in coverage JSON",
        )?;
    let total = json["data"][0]["totals"]["lines"]["count"]
        .as_u64()
        .ok_or(
            "missing lines.count in coverage JSON",
        )?;

    println!(
        "  lines: {covered}/{total} ({line_pct:.1}%)"
    );

    // Per-file summary.
    if let Some(files) =
        json["data"][0]["files"].as_array()
    {
        for file in files {
            let name = file["filename"]
                .as_str()
                .unwrap_or("?");
            let pct = file["summary"]["lines"]["percent"]
                .as_f64()
                .unwrap_or(0.0);
            // Show only the relative path from src/.
            let short = name
                .rsplit_once("src\\")
                .or_else(|| name.rsplit_once("src/"))
                .map_or(name, |(_, rest)| rest);
            let marker =
                if pct < COVERAGE_THRESHOLD { "!" } else { " " };
            println!("  {marker} {short:<50} {pct:>5.1}%");
        }
    }

    if line_pct < COVERAGE_THRESHOLD {
        Err(format!(
            "coverage {line_pct:.1}% is below \
             {COVERAGE_THRESHOLD}% threshold"
        ))
    } else {
        println!(
            "  coverage OK ({line_pct:.1}% >= \
             {COVERAGE_THRESHOLD}%)"
        );
        Ok(())
    }
}

fn run_dupes() -> Result<(), String> {
    println!(
        "→ checking code duplication \
         (threshold: {DUPLICATION_THRESHOLD}%)"
    );
    let threshold = format!("{DUPLICATION_THRESHOLD}");
    run_cmd(
        "code-dupes",
        &[
            "-p",
            "crates/rustwerk/src",
            "--exclude-tests",
            "check",
            "--max-exact-percent",
            &threshold,
        ],
    )
    .map_err(|e| {
        format!(
            "{e}\n  Install with: cargo install code-dupes"
        )
    })
}

/// Resolve the cargo binary path. Prefers the `CARGO`
/// env var (set by cargo when running xtask) over a
/// PATH lookup.
fn cargo_bin() -> String {
    std::env::var("CARGO")
        .unwrap_or_else(|_| "cargo".into())
}

fn run_cmd(
    cmd: &str,
    args: &[&str],
) -> Result<(), String> {
    println!("→ {cmd} {}", args.join(" "));
    let status = Command::new(cmd)
        .args(args)
        .status()
        .map_err(|e| format!("failed to run {cmd}: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        match status.code() {
            Some(code) => {
                Err(format!("{cmd} exited with {code}"))
            }
            None => {
                Err(format!("{cmd} terminated by signal"))
            }
        }
    }
}

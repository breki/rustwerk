use std::fmt::Write as _;
use std::process::Command;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask")]
struct Cli {
    #[command(subcommand)]
    command: XCommand,
}

#[derive(Subcommand)]
enum XCommand {
    /// Fast compile check (no tests, concise output)
    Check,
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

/// Minimum line coverage percentage (overall and per-module).
const COVERAGE_THRESHOLD: f64 = 90.0;

/// Per-module coverage floor. Small CLI modules with
/// error-propagation branches that require filesystem
/// failures to trigger may fall below the main threshold.
const MODULE_COVERAGE_THRESHOLD: f64 = 85.0;

/// Maximum allowed exact duplication percentage
/// (production code only, tests excluded).
const DUPLICATION_THRESHOLD: f64 = 6.0;

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        XCommand::Check => run_check(),
        XCommand::Clippy => run_clippy(),
        XCommand::Test { filter } => run_test(filter.as_deref()),
        XCommand::Validate => run_clippy()
            .and_then(|()| run_test(None))
            .and_then(|()| run_coverage())
            .and_then(|()| run_dupes()),
        XCommand::Fmt => run_fmt(),
        XCommand::Coverage => run_coverage(),
    };

    if let Err(e) = result {
        eprintln!("xtask error: {e}");
        std::process::exit(1);
    }
}

/// Maximum number of error lines to display for `check`.
const CHECK_MAX_ERROR_LINES: usize = 10;

fn run_check() -> Result<(), String> {
    println!("→ {} check --workspace --message-format=short", cargo_bin());
    let output = Command::new(cargo_bin())
        .args(["check", "--workspace", "--message-format=short"])
        .output()
        .map_err(|e| format!("failed to run cargo check: {e}"))?;

    if output.status.success() {
        println!("Check OK");
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let errors = extract_check_errors(&stderr);
    let count = errors.len();

    eprintln!("FAILED: {count} compilation error(s)\n");
    for line in errors.iter().take(CHECK_MAX_ERROR_LINES) {
        eprintln!("  {line}");
    }
    if count > CHECK_MAX_ERROR_LINES {
        eprintln!("  ... and {} more", count - CHECK_MAX_ERROR_LINES);
    }
    Err(format!("{count} compilation error(s)"))
}

fn extract_check_errors(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .filter(|l| l.starts_with("error[") || l.starts_with("error:"))
        .filter(|l| !l.starts_with("error: aborting due to"))
        .collect()
}

fn run_clippy() -> Result<(), String> {
    run_cmd(
        &cargo_bin(),
        &["clippy", "--workspace", "--", "-D", "warnings"],
    )
}

fn run_test(filter: Option<&str>) -> Result<(), String> {
    let mut args = vec!["test", "--workspace"];
    if let Some(f) = filter {
        if f.is_empty() {
            return Err("test filter must not be empty".into());
        }
        args.push("--");
        args.push(f);
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
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("cargo llvm-cov failed:\n{stderr}"));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("failed to parse coverage JSON: {e}"))?;

    // Extract total line coverage percentage.
    let line_pct = json["data"][0]["totals"]["lines"]["percent"]
        .as_f64()
        .ok_or("missing lines.percent in coverage JSON")?;

    let covered = json["data"][0]["totals"]["lines"]["covered"]
        .as_u64()
        .ok_or("missing lines.covered in coverage JSON")?;
    let total = json["data"][0]["totals"]["lines"]["count"]
        .as_u64()
        .ok_or("missing lines.count in coverage JSON")?;

    println!("  lines: {covered}/{total} ({line_pct:.1}%)");

    // Per-file summary.
    let mut below_threshold = Vec::new();
    if let Some(files) = json["data"][0]["files"].as_array() {
        for file in files {
            let name = file["filename"].as_str().unwrap_or("?");
            let pct =
                file["summary"]["lines"]["percent"].as_f64().unwrap_or(0.0);
            // Show only the relative path from src/.
            let short = name
                .rsplit_once("src\\")
                .or_else(|| name.rsplit_once("src/"))
                .map_or(name, |(_, rest)| rest);
            let marker = if pct < MODULE_COVERAGE_THRESHOLD {
                below_threshold.push((short.to_string(), pct));
                "!"
            } else {
                " "
            };
            println!("  {marker} {short:<50} {pct:>5.1}%");
        }
    }

    if line_pct < COVERAGE_THRESHOLD {
        Err(format!(
            "coverage {line_pct:.1}% is below \
             {COVERAGE_THRESHOLD}% threshold"
        ))
    } else if !below_threshold.is_empty() {
        let mut msg = String::from("modules below coverage threshold:");
        for (name, pct) in &below_threshold {
            let _ = write!(msg, "\n    {name}: {pct:.1}%");
        }
        Err(msg)
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
    .map_err(|e| format!("{e}\n  Install with: cargo install code-dupes"))
}

/// Resolve the cargo binary path. Prefers the `CARGO`
/// env var (set by cargo when running xtask) over a
/// PATH lookup.
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

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_STDERR: &str = "\
error[E0425]: cannot find value `foo` in this scope
 --> crates/rustwerk/src/lib.rs:45:12
error[E0308]: mismatched types
 --> crates/rustwerk/src/cli.rs:123:5
warning: unused variable: `x`
 --> xtask/src/main.rs:10:9
error: aborting due to 2 previous errors";

    #[test]
    fn extracts_only_error_bracket_lines() {
        let errors = extract_check_errors(SAMPLE_STDERR);
        assert_eq!(errors.len(), 2);
        assert!(errors[0].contains("E0425"));
        assert!(errors[1].contains("E0308"));
    }

    #[test]
    fn empty_input_gives_empty_result() {
        let errors = extract_check_errors("");
        assert!(errors.is_empty());
    }

    #[test]
    fn warnings_only_gives_empty_result() {
        let stderr = "warning: unused variable: `x`";
        let errors = extract_check_errors(stderr);
        assert!(errors.is_empty());
    }

    #[test]
    fn keeps_user_errors_that_mention_aborting() {
        let stderr = "\
error: aborting build: feature flag missing
error: aborting due to 1 previous error";
        let errors = extract_check_errors(stderr);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("feature flag missing"));
    }

    #[test]
    fn includes_plain_error_lines() {
        let stderr = "\
error[E0425]: cannot find value `foo`
error: could not compile `rustwerk`
error: aborting due to 1 previous error";
        let errors = extract_check_errors(stderr);
        assert_eq!(errors.len(), 2);
        assert!(errors[0].contains("E0425"));
        assert!(errors[1].contains("could not compile"));
    }
}

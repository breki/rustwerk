use std::ffi::OsStr;
use std::fmt::Write as _;
use std::process::Command;

use clap::{Parser, Subcommand};

mod kg;

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
    /// Knowledge graph site — build or serve with live reload.
    #[command(subcommand)]
    Kg(KgCommand),
}

#[derive(Subcommand)]
enum KgCommand {
    /// One-shot build into tools/kg/site/public/.
    Build,
    /// Live-reload dev server. Extra args (after `--`) pass through
    /// to `zola serve`, e.g. `--port 8080 --open`.
    Serve {
        /// Arguments forwarded verbatim to `zola serve`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        zola_args: Vec<String>,
    },
}

/// Minimum line coverage percentage (overall and per-module).
const COVERAGE_THRESHOLD: f64 = 90.0;

/// Per-module coverage floor. Small CLI modules with
/// error-propagation branches that require filesystem
/// failures to trigger may fall below the main threshold.
const MODULE_COVERAGE_THRESHOLD: f64 = 85.0;

/// Path suffixes excluded from the per-module coverage
/// floor. FFI host code that requires a loaded dynamic
/// library to exercise, and doc-comment-only crate
/// stubs awaiting implementation, are reported but do
/// not fail the check. Entries use forward slashes;
/// the match site normalises file paths before
/// comparison so the same list works on Windows and
/// Unix.
const MODULE_COVERAGE_EXEMPT: &[&str] = &[
    // Plugin host: most unsafe FFI paths need a real
    // cdylib to exercise.
    "bin/rustwerk/plugin_host.rs",
    // CLI dispatch glue: clap-wired command arms can
    // only be exercised by spawning the binary, which
    // is integration-test territory (covered by
    // cli_integration.rs and upcoming PLG-HOST-E2E).
    "bin/rustwerk/main.rs",
    // Main rustwerk lib.rs is module re-exports only.
    "rustwerk/src/lib.rs",
    // Plugin API is covered by its own package's unit
    // tests; the `rustwerk` binary only uses a handful
    // of constants from it.
    "rustwerk-plugin-api/src/lib.rs",
    // Jira plugin HTTP client: the ureq-backed
    // transport paths (timeouts, TLS failures, network
    // errors) are hard to drive from unit tests. The
    // gateway-fallback logic is covered via the
    // HttpClient fake; the leftover uncovered lines
    // are the ureq-wiring layer.
    "rustwerk-jira-plugin/src/jira_client.rs",
];

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
        XCommand::Kg(KgCommand::Build) => kg::run_build(),
        XCommand::Kg(KgCommand::Serve { zola_args }) => kg::run_serve(&zola_args),
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

    // Non-compilation failures (manifest parse, lockfile
    // corruption, registry network problems) exit
    // non-zero without emitting lines that start with
    // `error[` or `error:`, so fall back to printing the
    // tail of stderr instead of a silent "0 error(s)"
    // message.
    if count == 0 {
        eprintln!("FAILED: cargo check exited non-zero (no rustc errors parsed)\n");
        for line in tail_lines(&stderr, CHECK_STDERR_TAIL_LINES) {
            eprintln!("  {line}");
        }
        return Err("cargo check failed (see stderr tail above)".into());
    }

    eprintln!("FAILED: {count} compilation error(s)\n");
    for line in errors.iter().take(CHECK_MAX_ERROR_LINES) {
        eprintln!("  {line}");
    }
    if count > CHECK_MAX_ERROR_LINES {
        eprintln!("  ... and {} more", count - CHECK_MAX_ERROR_LINES);
    }
    Err(format!("{count} compilation error(s)"))
}

/// Number of trailing stderr lines to show when
/// `cargo check` exits non-zero with no recognised
/// rustc error lines.
const CHECK_STDERR_TAIL_LINES: usize = 20;

/// Last `n` non-empty lines of `text`, preserving
/// original order.
fn tail_lines(text: &str, n: usize) -> Vec<&str> {
    let all: Vec<&str> =
        text.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = all.len().saturating_sub(n);
    all[start..].to_vec()
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
        cargo_bin(),
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
    run_cmd(cargo_bin(), &args)
}

fn run_fmt() -> Result<(), String> {
    run_cmd(cargo_bin(), &["fmt", "--all"])
}

fn run_coverage() -> Result<(), String> {
    println!("→ checking coverage (threshold: {COVERAGE_THRESHOLD}%)");
    let output = Command::new(cargo_bin())
        .args([
            "llvm-cov",
            "--workspace",
            "--ignore-filename-regex",
            "xtask",
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
            // Normalize path separators once so the
            // forward-slash exemption list matches on
            // Windows too.
            let norm = name.replace('\\', "/");
            let exempt = MODULE_COVERAGE_EXEMPT
                .iter()
                .any(|suffix| norm.ends_with(suffix));
            let marker = if exempt {
                "~"
            } else if pct < MODULE_COVERAGE_THRESHOLD {
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
            "crates",
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

/// Launch a subprocess, echoing an unambiguous banner before the call
/// and converting non-zero exits into a `String` error. Accepts anything
/// that coerces to `&OsStr` so callers do not have to pre-convert
/// `PathBuf` / `String` arguments.
//
// `clippy::unnecessary_debug_formatting` is allowed deliberately:
// `{:?}` on `OsStr` quotes each arg, which keeps the echo
// copy-pasteable as a real command line when args contain spaces.
// `Display` would drop the quoting and make spaces ambiguous.
#[allow(clippy::unnecessary_debug_formatting)]
pub(crate) fn run_cmd<C, S>(cmd: C, args: &[S]) -> Result<(), String>
where
    C: AsRef<OsStr>,
    S: AsRef<OsStr>,
{
    let cmd_ref = cmd.as_ref();
    print!("→ {cmd_ref:?}");
    for arg in args {
        print!(" {:?}", arg.as_ref());
    }
    println!();

    let status = Command::new(cmd_ref)
        .args(args)
        .status()
        .map_err(|e| format!("failed to run {}: {e}", cmd_ref.to_string_lossy()))?;

    if status.success() {
        Ok(())
    } else {
        let name = cmd_ref.to_string_lossy();
        match status.code() {
            Some(code) => Err(format!("{name} exited with {code}")),
            None => Err(format!("{name} terminated by signal")),
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

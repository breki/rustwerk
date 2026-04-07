use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Path to the rustwerk binary built by cargo.
fn rustwerk_bin() -> PathBuf {
    // Use the env var set by cargo for workspace binaries.
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_rustwerk") {
        return PathBuf::from(path);
    }
    // Fallback: navigate from test binary location.
    let mut path =
        std::env::current_exe().expect("failed to get current exe path");
    path.pop(); // remove test binary name
    path.pop(); // remove deps/
    path.push("rustwerk");
    if cfg!(windows) {
        path.set_extension("exe");
    }
    path
}

/// Create a temp directory with a unique name for each
/// test.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("rustwerk-cli-test-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Run a rustwerk command in the given directory.
fn run(dir: &PathBuf, args: &[&str]) -> (String, String, bool) {
    let output = Command::new(rustwerk_bin())
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to run rustwerk");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (stdout, stderr, output.status.success())
}

// --- init ---

#[test]
fn init_creates_project_file() {
    let dir = temp_dir("init-creates");
    let (stdout, _, ok) = run(&dir, &["init", "TestProject"]);
    assert!(ok, "init should succeed");
    assert!(stdout.contains("Initialized project"));
    assert!(dir.join(".rustwerk/project.json").exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn init_refuses_existing_project() {
    let dir = temp_dir("init-refuses");
    run(&dir, &["init", "First"]);
    let (_, stderr, ok) = run(&dir, &["init", "Second"]);
    assert!(!ok, "second init should fail");
    assert!(stderr.contains("already exists"), "stderr: {stderr}");
    let _ = fs::remove_dir_all(&dir);
}

// --- show ---

#[test]
fn show_displays_project() {
    let dir = temp_dir("show");
    run(&dir, &["init", "ShowTest"]);
    let (stdout, _, ok) = run(&dir, &["show"]);
    assert!(ok);
    assert!(stdout.contains("ShowTest"));
    assert!(stdout.contains("Tasks:"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn show_displays_complexity_and_effort() {
    let dir = temp_dir("show-full");
    run(&dir, &["init", "P"]);
    run(
        &dir,
        &[
            "task",
            "add",
            "T",
            "--id",
            "A",
            "--complexity",
            "5",
            "--effort",
            "2D",
        ],
    );
    let (stdout, _, ok) = run(&dir, &["show"]);
    assert!(ok);
    assert!(stdout.contains("Complexity:"));
    assert!(stdout.contains("Effort:"));
    let _ = fs::remove_dir_all(&dir);
}

// --- task add ---

#[test]
fn task_add_with_id() {
    let dir = temp_dir("add-id");
    run(&dir, &["init", "P"]);
    let (stdout, _, ok) = run(&dir, &["task", "add", "My Task", "--id", "MT"]);
    assert!(ok);
    assert!(stdout.contains("Created task MT"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_add_auto_id() {
    let dir = temp_dir("add-auto");
    run(&dir, &["init", "P"]);
    let (stdout, _, ok) = run(&dir, &["task", "add", "Auto Task"]);
    assert!(ok);
    assert!(stdout.contains("T0001"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_add_with_all_options() {
    let dir = temp_dir("add-opts");
    run(&dir, &["init", "P"]);
    let (stdout, _, ok) = run(
        &dir,
        &[
            "task",
            "add",
            "Complex",
            "--id",
            "CX",
            "--desc",
            "A description",
            "--complexity",
            "5",
            "--effort",
            "8H",
        ],
    );
    assert!(ok);
    assert!(stdout.contains("CX"));
    let _ = fs::remove_dir_all(&dir);
}

// --- task status ---

#[test]
fn task_status_valid_transition() {
    let dir = temp_dir("status-valid");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "T", "--id", "A"]);
    let (stdout, _, ok) = run(&dir, &["task", "status", "A", "in-progress"]);
    assert!(ok);
    assert!(stdout.contains("IN_PROGRESS"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_status_invalid_transition() {
    let dir = temp_dir("status-invalid");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "T", "--id", "A"]);
    let (_, _, ok) = run(&dir, &["task", "status", "A", "done"]);
    assert!(!ok, "TODO->DONE should fail");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_status_force() {
    let dir = temp_dir("status-force");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "T", "--id", "A"]);
    run(&dir, &["task", "status", "A", "in-progress"]);
    run(&dir, &["task", "status", "A", "done"]);
    let (stdout, _, ok) =
        run(&dir, &["task", "status", "A", "todo", "--force"]);
    assert!(ok, "force should bypass validation");
    assert!(stdout.contains("TODO"));
    let _ = fs::remove_dir_all(&dir);
}

// --- task remove ---

#[test]
fn task_remove() {
    let dir = temp_dir("remove");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "T", "--id", "A"]);
    let (stdout, _, ok) = run(&dir, &["task", "remove", "A"]);
    assert!(ok);
    assert!(stdout.contains("Removed"));
    let _ = fs::remove_dir_all(&dir);
}

// --- task update ---

#[test]
fn task_update_title() {
    let dir = temp_dir("update");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "Old", "--id", "A"]);
    let (stdout, _, ok) = run(&dir, &["task", "update", "A", "--title", "New"]);
    assert!(ok);
    assert!(stdout.contains("New"));
    let _ = fs::remove_dir_all(&dir);
}

// --- task assign / unassign ---

#[test]
fn task_assign_and_unassign() {
    let dir = temp_dir("assign");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "T", "--id", "A"]);
    // Register developer via batch (no `dev add` CLI yet).
    let output = Command::new(rustwerk_bin())
        .args(["batch"])
        .current_dir(&dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            let stdin = child.stdin.as_mut().unwrap();
            // Use a dummy "dev.add" — but batch doesn't
            // support that yet. Instead, hand-edit the
            // project.json to add a developer.
            stdin.write_all(b"[]").unwrap();
            child.wait_with_output()
        })
        .expect("batch failed");
    assert!(output.status.success());
    // Hand-add the developer to the project file.
    let proj_path = dir.join(".rustwerk").join("project.json");
    let json = fs::read_to_string(&proj_path).unwrap();
    let mut proj: serde_json::Value = serde_json::from_str(&json).unwrap();
    proj["developers"] = serde_json::json!({
        "alice": {"name": "Alice"}
    });
    fs::write(&proj_path, serde_json::to_string_pretty(&proj).unwrap())
        .unwrap();
    let (stdout, _, ok) = run(&dir, &["task", "assign", "A", "alice"]);
    assert!(ok, "assign should succeed");
    assert!(stdout.contains("alice"));
    let (stdout, _, ok) = run(&dir, &["task", "unassign", "A"]);
    assert!(ok);
    assert!(stdout.contains("unassigned"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_assign_from_env_var() {
    let dir = temp_dir("assign-env");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "T", "--id", "A"]);
    run(&dir, &["dev", "add", "alice", "Alice"]);
    // Assign using RUSTWERK_USER env var instead of
    // positional argument.
    let output = Command::new(rustwerk_bin())
        .args(["task", "assign", "A"])
        .current_dir(&dir)
        .env("RUSTWERK_USER", "alice")
        .output()
        .expect("failed to run rustwerk");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "assign via env should succeed");
    assert!(stdout.contains("alice"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_assign_no_dev_fails() {
    let dir = temp_dir("assign-nodev");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "T", "--id", "A"]);
    // No positional arg and no RUSTWERK_USER — should fail.
    let output = Command::new(rustwerk_bin())
        .args(["task", "assign", "A"])
        .current_dir(&dir)
        .env_remove("RUSTWERK_USER")
        .output()
        .expect("failed to run rustwerk");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no developer specified"),
        "expected env-var error, got: {stderr}"
    );
    let _ = fs::remove_dir_all(&dir);
}

// --- task list ---

#[test]
fn task_list_all() {
    let dir = temp_dir("list-all");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "A", "--id", "A"]);
    run(&dir, &["task", "add", "B", "--id", "B"]);
    let (stdout, _, ok) = run(&dir, &["task", "list"]);
    assert!(ok);
    assert!(stdout.contains("A"));
    assert!(stdout.contains("B"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_list_available() {
    let dir = temp_dir("list-avail");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "A", "--id", "A"]);
    run(&dir, &["task", "add", "B", "--id", "B"]);
    run(&dir, &["task", "depend", "B", "A"]);
    let (stdout, _, ok) = run(&dir, &["task", "list", "--available"]);
    assert!(ok);
    // A is available (no deps), B is not (depends on A).
    assert!(stdout.contains("A"));
    assert!(!stdout.contains(" B "));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_list_filter_by_status() {
    let dir = temp_dir("list-status");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "Alpha", "--id", "A"]);
    run(&dir, &["task", "add", "Beta", "--id", "B"]);
    run(&dir, &["task", "status", "A", "in-progress"]);
    // Filter for in-progress only.
    let (stdout, _, ok) =
        run(&dir, &["task", "list", "--status", "in-progress"]);
    assert!(ok);
    assert!(stdout.contains("A"));
    assert!(!stdout.contains("B"));
    // Filter for todo only.
    let (stdout, _, ok) = run(&dir, &["task", "list", "--status", "todo"]);
    assert!(ok);
    assert!(!stdout.contains(" A "));
    assert!(stdout.contains("B"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_list_filter_by_assignee() {
    let dir = temp_dir("list-assignee");
    run(&dir, &["init", "P"]);
    run(&dir, &["dev", "add", "alice", "Alice"]);
    run(&dir, &["dev", "add", "bob", "Bob"]);
    run(&dir, &["task", "add", "Alpha", "--id", "A"]);
    run(&dir, &["task", "add", "Beta", "--id", "B"]);
    run(&dir, &["task", "assign", "A", "alice"]);
    run(&dir, &["task", "assign", "B", "bob"]);
    let (stdout, _, ok) = run(&dir, &["task", "list", "--assignee", "alice"]);
    assert!(ok);
    assert!(stdout.contains("Alpha"));
    assert!(!stdout.contains("Beta"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_list_filter_by_chain() {
    let dir = temp_dir("list-chain");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "Root", "--id", "R"]);
    run(&dir, &["task", "add", "Mid", "--id", "M"]);
    run(&dir, &["task", "add", "Leaf", "--id", "L"]);
    run(&dir, &["task", "add", "Xtra", "--id", "X"]);
    run(&dir, &["task", "depend", "M", "R"]);
    run(&dir, &["task", "depend", "L", "M"]);
    // Chain of L should include R, M, L (transitive deps + self).
    let (stdout, _, ok) = run(&dir, &["task", "list", "--chain", "L"]);
    assert!(ok);
    assert!(stdout.contains("Root"), "stdout: {stdout}");
    assert!(stdout.contains("Mid"), "stdout: {stdout}");
    assert!(stdout.contains("Leaf"), "stdout: {stdout}");
    assert!(!stdout.contains("Xtra"), "stdout: {stdout}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_list_combined_filters() {
    let dir = temp_dir("list-combined");
    run(&dir, &["init", "P"]);
    run(&dir, &["dev", "add", "alice", "Alice"]);
    run(&dir, &["task", "add", "A1", "--id", "A1"]);
    run(&dir, &["task", "add", "A2", "--id", "A2"]);
    run(&dir, &["task", "assign", "A1", "alice"]);
    run(&dir, &["task", "assign", "A2", "alice"]);
    run(&dir, &["task", "status", "A1", "in-progress"]);
    // Filter: assignee=alice AND status=in-progress.
    let (stdout, _, ok) = run(
        &dir,
        &[
            "task",
            "list",
            "--assignee",
            "alice",
            "--status",
            "in-progress",
        ],
    );
    assert!(ok);
    assert!(stdout.contains("A1"));
    assert!(!stdout.contains("A2"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_list_chain_unknown_task_fails() {
    let dir = temp_dir("list-chain-unknown");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "A", "--id", "A"]);
    let (_, stderr, ok) = run(&dir, &["task", "list", "--chain", "NOPE"]);
    assert!(!ok, "should fail for unknown task");
    assert!(stderr.contains("not found"), "stderr: {stderr}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_list_no_matching_tasks() {
    let dir = temp_dir("list-no-match");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "A", "--id", "A"]);
    let (stdout, _, ok) = run(&dir, &["task", "list", "--status", "done"]);
    assert!(ok);
    assert!(stdout.contains("No matching tasks"), "stdout: {stdout}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_list_status_conflicts_with_available() {
    let dir = temp_dir("list-conflict");
    run(&dir, &["init", "P"]);
    let (_, _, ok) =
        run(&dir, &["task", "list", "--available", "--status", "done"]);
    assert!(!ok, "--available and --status should conflict");
    let _ = fs::remove_dir_all(&dir);
}

// --- depend / undepend ---

#[test]
fn depend_and_undepend() {
    let dir = temp_dir("depend");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "A", "--id", "A"]);
    run(&dir, &["task", "add", "B", "--id", "B"]);
    let (stdout, _, ok) = run(&dir, &["task", "depend", "B", "A"]);
    assert!(ok);
    assert!(stdout.contains("depends on"));
    let (stdout, _, ok) = run(&dir, &["task", "undepend", "B", "A"]);
    assert!(ok);
    assert!(stdout.contains("Removed"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn depend_cycle_rejected() {
    let dir = temp_dir("cycle");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "A", "--id", "A"]);
    run(&dir, &["task", "add", "B", "--id", "B"]);
    run(&dir, &["task", "depend", "A", "B"]);
    let (_, _, ok) = run(&dir, &["task", "depend", "B", "A"]);
    assert!(!ok, "cycle should be rejected");
    let _ = fs::remove_dir_all(&dir);
}

// --- effort ---

#[test]
fn effort_log_and_estimate() {
    let dir = temp_dir("effort");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "T", "--id", "A"]);
    run(&dir, &["task", "status", "A", "in-progress"]);
    let (stdout, _, ok) =
        run(&dir, &["effort", "log", "A", "2.5H", "--dev", "alice"]);
    assert!(ok);
    assert!(stdout.contains("2.5H"));
    let (stdout, _, ok) = run(&dir, &["effort", "estimate", "A", "8H"]);
    assert!(ok);
    assert!(stdout.contains("8H"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn effort_log_requires_in_progress() {
    let dir = temp_dir("effort-fail");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "T", "--id", "A"]);
    let (_, _, ok) = run(&dir, &["effort", "log", "A", "1H", "--dev", "bob"]);
    assert!(!ok, "should fail on TODO task");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn effort_log_from_env_var() {
    let dir = temp_dir("effort-env");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "T", "--id", "A"]);
    run(&dir, &["dev", "add", "alice", "Alice"]);
    run(&dir, &["task", "status", "A", "in-progress"]);
    // Log effort using RUSTWERK_USER instead of --dev.
    let output = Command::new(rustwerk_bin())
        .args(["effort", "log", "A", "2H"])
        .current_dir(&dir)
        .env("RUSTWERK_USER", "alice")
        .output()
        .expect("failed to run rustwerk");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "effort log via env should succeed");
    assert!(stdout.contains("2H"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn effort_log_invalid_task_fails() {
    let dir = temp_dir("effort-bad-id");
    run(&dir, &["init", "P"]);
    let (_, _, ok) = run(&dir, &["effort", "log", "!!!", "1H", "--dev", "x"]);
    assert!(!ok, "invalid task ID should fail");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn effort_log_invalid_amount_fails() {
    let dir = temp_dir("effort-bad-amt");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "T", "--id", "A"]);
    run(&dir, &["task", "status", "A", "in-progress"]);
    let (_, _, ok) = run(&dir, &["effort", "log", "A", "abc", "--dev", "x"]);
    assert!(!ok, "invalid effort amount should fail");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn effort_estimate_invalid_task_fails() {
    let dir = temp_dir("est-bad-id");
    run(&dir, &["init", "P"]);
    let (_, _, ok) = run(&dir, &["effort", "estimate", "!!!", "1H"]);
    assert!(!ok, "invalid task ID should fail");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn effort_estimate_invalid_amount_fails() {
    let dir = temp_dir("est-bad-amt");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "T", "--id", "A"]);
    let (_, _, ok) = run(&dir, &["effort", "estimate", "A", "xyz"]);
    assert!(!ok, "invalid effort amount should fail");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn effort_estimate_nonexistent_task_fails() {
    let dir = temp_dir("est-noent");
    run(&dir, &["init", "P"]);
    let (_, _, ok) = run(&dir, &["effort", "estimate", "NOPE", "1H"]);
    assert!(!ok, "nonexistent task should fail");
    let _ = fs::remove_dir_all(&dir);
}

// --- batch ---

#[test]
fn batch_from_stdin() {
    let dir = temp_dir("batch-stdin");
    run(&dir, &["init", "P"]);
    let output = Command::new(rustwerk_bin())
        .args(["batch"])
        .current_dir(&dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            let stdin = child.stdin.as_mut().unwrap();
            stdin
                .write_all(
                    b"[{\"command\":\"task.add\",\
                       \"args\":{\"title\":\"Batch\",\
                       \"id\":\"BT\"}}]",
                )
                .unwrap();
            child.wait_with_output()
        })
        .expect("batch failed");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("BT"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn batch_rollback_on_error() {
    let dir = temp_dir("batch-rollback");
    run(&dir, &["init", "P"]);
    let output = Command::new(rustwerk_bin())
        .args(["batch"])
        .current_dir(&dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            let stdin = child.stdin.as_mut().unwrap();
            stdin
                .write_all(
                    b"[{\"command\":\"task.add\",\
                       \"args\":{\"title\":\"OK\",\
                       \"id\":\"OK\"}},\
                      {\"command\":\"task.status\",\
                       \"args\":{\"id\":\"NOPE\",\
                       \"status\":\"done\"}}]",
                )
                .unwrap();
            child.wait_with_output()
        })
        .expect("batch failed");
    assert!(!output.status.success());
    // Verify OK task was not persisted.
    let (stdout, _, _) = run(&dir, &["task", "list"]);
    assert!(!stdout.contains("OK"), "rolled-back task should not exist");
    let _ = fs::remove_dir_all(&dir);
}

// --- error cases ---

#[test]
fn show_without_project_fails() {
    let dir = temp_dir("no-project");
    let (_, _, ok) = run(&dir, &["show"]);
    assert!(!ok, "show without project should fail");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_add_without_project_fails() {
    let dir = temp_dir("no-project-add");
    let (_, _, ok) = run(&dir, &["task", "add", "X", "--id", "X"]);
    assert!(!ok);
    let _ = fs::remove_dir_all(&dir);
}

// --- gantt visual alignment ---

/// Find the character (display) column of the first
/// occurrence of `needle` in `s`. Returns `None` if not
/// found.
fn char_col(s: &str, needle: char) -> Option<usize> {
    s.chars()
        .enumerate()
        .find(|(_, c)| *c == needle)
        .map(|(i, _)| i)
}

/// Find all character columns where `needle` appears.
fn char_cols(s: &str, needle: char) -> Vec<usize> {
    s.chars()
        .enumerate()
        .filter(|(_, c)| *c == needle)
        .map(|(i, _)| i)
        .collect()
}

/// Find the last character column of `needle` in `s`.
fn last_char_col(s: &str, needle: char) -> Option<usize> {
    s.chars()
        .enumerate()
        .filter(|(_, c)| *c == needle)
        .map(|(i, _)| i)
        .last()
}

/// Set up a project with sequential tasks A(5) -> B(5)
/// -> C(5), mark A done. Returns the temp dir.
fn gantt_project_abc(name: &str) -> PathBuf {
    let dir = temp_dir(name);
    let bin = rustwerk_bin();
    let r = |args: &[&str]| {
        Command::new(&bin)
            .args(args)
            .current_dir(&dir)
            .output()
            .expect("failed to run rustwerk");
    };
    r(&["init", "P"]);
    r(&["task", "add", "A", "--id", "A", "--complexity", "5"]);
    r(&["task", "add", "B", "--id", "B", "--complexity", "5"]);
    r(&["task", "add", "C", "--id", "C", "--complexity", "5"]);
    r(&["task", "depend", "B", "A"]);
    r(&["task", "depend", "C", "B"]);
    r(&["task", "status", "A", "in-progress"]);
    r(&["task", "status", "A", "done"]);
    dir
}

#[test]
fn gantt_header_ticks_align_with_bars() {
    let dir = gantt_project_abc("gantt-align");
    let (stdout, _, ok) = run(&dir, &["gantt"]);
    assert!(ok, "gantt should succeed");

    let lines: Vec<&str> = stdout.lines().collect();
    assert!(
        lines.len() >= 5,
        "expected header + 3 task rows, got {} lines",
        lines.len()
    );

    // The header has tick numbers (line 0) and tick marks
    // (line 1). Task bars follow. Tick positions in the
    // header must align with the bar start positions.
    //
    // Task A starts at time 0, B at time 5, C at time 10.
    // The tick mark `|` at time 5 must be in the same
    // column as the left cap `▐` of task B's bar.
    let tick_line = lines[1];
    let bar_b_line = lines[3]; // A=line 2, B=line 3

    // Find column of the second `┬` (time 5 tick).
    let tick_positions = char_cols(tick_line, '\u{252C}');
    assert!(
        tick_positions.len() >= 2,
        "expected at least 2 tick marks (┬), got {:?}",
        tick_positions
    );
    let tick5_col = tick_positions[1];

    // Find column of B's left cap.
    let b_cap_col = char_col(bar_b_line, '\u{2590}')
        .expect("B's bar should have a left cap ▐");

    assert_eq!(
        tick5_col, b_cap_col,
        "tick at time 5 (col {}) must align with B's \
         bar start (col {})",
        tick5_col, b_cap_col
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn gantt_axis_uses_box_drawing_chars() {
    let dir = gantt_project_abc("gantt-axis");
    let (stdout, _, ok) = run(&dir, &["gantt"]);
    assert!(ok);

    let lines: Vec<&str> = stdout.lines().collect();
    let axis_line = lines[1];

    // Axis should contain ┬ (tick marks) and ─ (lines).
    assert!(
        axis_line.contains('\u{252C}'),
        "axis should contain ┬: {axis_line}"
    );
    assert!(
        axis_line.contains('\u{2500}'),
        "axis should contain ─: {axis_line}"
    );
    // Should NOT contain plain | or spaces in the axis
    // area (after the label prefix).
    assert!(
        !axis_line.contains('|'),
        "axis should not use plain |: {axis_line}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn gantt_bars_use_unicode_blocks() {
    let dir = gantt_project_abc("gantt-unicode");
    let (stdout, _, ok) = run(&dir, &["gantt"]);
    assert!(ok);

    let lines: Vec<&str> = stdout.lines().collect();
    let bar_a = lines[2]; // A is done
    let bar_b = lines[3]; // B is todo

    // Done bars use full block █.
    assert!(
        bar_a.contains('\u{2588}'),
        "done bar should contain █, got: {bar_a}"
    );
    // Todo bars use light shade ░.
    assert!(
        bar_b.contains('\u{2591}'),
        "todo bar should contain ░, got: {bar_b}"
    );
    // All bars have left cap ▐ and right cap ▌.
    assert!(
        bar_a.contains('\u{2590}') && bar_a.contains('\u{258C}'),
        "bar should have caps ▐▌, got: {bar_a}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn gantt_consecutive_bars_dont_overlap() {
    let dir = gantt_project_abc("gantt-no-overlap");
    let (stdout, _, ok) = run(&dir, &["gantt"]);
    assert!(ok);

    let lines: Vec<&str> = stdout.lines().collect();
    let bar_a = lines[2];
    let bar_b = lines[3];

    // A's right cap ▌ must be at a column strictly before
    // B's left cap ▐ (bars must not overlap or touch with
    // no gap).
    let a_end_col =
        last_char_col(bar_a, '\u{258C}').expect("A should have right cap ▌");

    let b_start_col =
        char_col(bar_b, '\u{2590}').expect("B should have left cap ▐");

    assert!(
        a_end_col < b_start_col,
        "A's right cap (col {a_end_col}) must be before \
         B's left cap (col {b_start_col})"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn gantt_id_column_aligned() {
    let dir = temp_dir("gantt-id-align");
    let bin = rustwerk_bin();
    let r = |args: &[&str]| {
        Command::new(&bin)
            .args(args)
            .current_dir(&dir)
            .output()
            .expect("failed to run rustwerk");
    };
    r(&["init", "P"]);
    r(&["task", "add", "Short", "--id", "AB", "--complexity", "3"]);
    r(&[
        "task",
        "add",
        "Longer name",
        "--id",
        "ABCDEFGHIJ",
        "--complexity",
        "3",
    ]);
    let (stdout, _, ok) = run(&dir, &["gantt"]);
    assert!(ok);

    let lines: Vec<&str> = stdout.lines().collect();
    // Skip header (2 lines), check task rows.
    let row1 = lines[2];
    let row2 = lines[3];

    // Both bars' left caps should be in the same column
    // (IDs are padded to the same width).
    let cap1_col =
        char_col(row1, '\u{2590}').expect("row1 should have left cap");
    let cap2_col =
        char_col(row2, '\u{2590}').expect("row2 should have left cap");

    assert_eq!(
        cap1_col, cap2_col,
        "bars should start at the same column regardless \
         of ID length (got {} vs {})",
        cap1_col, cap2_col
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn gantt_remaining_excludes_done_tasks() {
    let dir = gantt_project_abc("gantt-remaining");
    // A is done, B and C are not.
    let (stdout, _, ok) = run(&dir, &["gantt", "--remaining"]);
    assert!(ok, "gantt --remaining should succeed");

    // A should not appear (it's done).
    assert!(
        !stdout.contains(" A "),
        "done task A should be excluded: {stdout}"
    );

    let lines: Vec<&str> = stdout.lines().collect();
    // B's done dependency (A) is satisfied, so B starts
    // at 0 — its left cap should be at the same column
    // as the header's time-0 tick.
    let tick_line = lines[1];
    let tick0_col = char_col(tick_line, '\u{252C}')
        .expect("header should have tick ┬ at 0");

    let b_line = lines
        .iter()
        .find(|l| l.contains("B"))
        .expect("B should appear");
    let b_cap = char_col(b_line, '\u{2590}').expect("B should have left cap");
    assert_eq!(
        tick0_col, b_cap,
        "B should start at time 0 (done deps don't \
         block): tick0={tick0_col}, B cap={b_cap}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn gantt_remaining_recalculates_critical_path() {
    // Setup: A(done,10)->D(todo,1) is the full critical
    // path. B(todo,3)->C(todo,5) is a separate chain.
    // Full crit path: A->D (11). Remaining crit path:
    // B->C (8), not D (1).
    let dir = temp_dir("gantt-remaining-crit");
    let bin = rustwerk_bin();
    let r = |args: &[&str]| {
        Command::new(&bin)
            .args(args)
            .current_dir(&dir)
            .output()
            .expect("failed to run rustwerk");
    };
    r(&["init", "P"]);
    r(&["task", "add", "X", "--id", "A", "--complexity", "10"]);
    r(&["task", "add", "X", "--id", "B", "--complexity", "3"]);
    r(&["task", "add", "X", "--id", "C", "--complexity", "5"]);
    r(&["task", "add", "X", "--id", "D", "--complexity", "1"]);
    r(&["task", "depend", "D", "A"]);
    r(&["task", "depend", "C", "B"]);
    r(&["task", "status", "A", "in-progress"]);
    r(&["task", "status", "A", "done"]);

    let (stdout, _, ok) = run(&dir, &["gantt", "--remaining"]);
    assert!(ok);

    let lines: Vec<&str> = stdout.lines().collect();
    // B and C should be on remaining critical path (*).
    let b_line = lines
        .iter()
        .find(|l| l.contains("B"))
        .expect("B should appear");
    let c_line = lines
        .iter()
        .find(|l| l.contains("C"))
        .expect("C should appear");
    assert!(
        b_line.starts_with('*'),
        "B should be on remaining critical path: {b_line}"
    );
    assert!(
        c_line.starts_with('*'),
        "C should be on remaining critical path: {c_line}"
    );

    // D should NOT be on remaining critical path.
    let d_line = lines
        .iter()
        .find(|l| l.contains("D"))
        .expect("D should appear");
    assert!(
        d_line.starts_with(' '),
        "D should NOT be on remaining critical path: \
         {d_line}"
    );

    let _ = fs::remove_dir_all(&dir);
}

// --- report ---

#[test]
fn report_complete_shows_summary() {
    let dir = temp_dir("report-complete");
    let bin = rustwerk_bin();
    let r = |args: &[&str]| {
        Command::new(&bin)
            .args(args)
            .current_dir(&dir)
            .output()
            .expect("failed to run rustwerk");
    };
    r(&["init", "P"]);
    r(&[
        "task",
        "add",
        "A",
        "--id",
        "A",
        "--complexity",
        "3",
        "--effort",
        "8H",
    ]);
    r(&[
        "task",
        "add",
        "B",
        "--id",
        "B",
        "--complexity",
        "5",
        "--effort",
        "2D",
    ]);
    r(&["task", "depend", "B", "A"]);
    r(&["task", "status", "A", "in-progress"]);
    r(&["task", "status", "A", "done"]);

    let (stdout, _, ok) = run(&dir, &["report", "complete"]);
    assert!(ok, "report complete should succeed");

    // Should contain key summary fields.
    assert!(
        stdout.contains("Completion"),
        "should show completion: {stdout}"
    );
    assert!(
        stdout.contains("50%"),
        "should show 50% complete (1/2): {stdout}"
    );
    assert!(
        stdout.contains("Effort"),
        "should show effort section: {stdout}"
    );
    assert!(
        stdout.contains("Critical"),
        "should show critical path info: {stdout}"
    );

    let _ = fs::remove_dir_all(&dir);
}

// --- dev list ---

#[test]
fn dev_list_empty() {
    let dir = temp_dir("dev-list-empty");
    run(&dir, &["init", "P"]);
    let (stdout, _, ok) = run(&dir, &["dev", "list"]);
    assert!(ok, "dev list should succeed");
    assert!(
        stdout.contains("No developers"),
        "should say no developers: {stdout}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn dev_list_shows_developers() {
    let dir = temp_dir("dev-list-show");
    run(&dir, &["init", "P"]);
    // Inject developers directly into the project JSON.
    let path = dir.join(".rustwerk/project.json");
    let json = fs::read_to_string(&path).unwrap();
    let mut project: serde_json::Value = serde_json::from_str(&json).unwrap();
    project["developers"] = serde_json::json!({
        "alice": {
            "name": "Alice Smith",
            "email": "alice@example.com",
            "role": "lead"
        },
        "bob": {
            "name": "Bob Jones"
        }
    });
    fs::write(&path, serde_json::to_string_pretty(&project).unwrap()).unwrap();

    let (stdout, _, ok) = run(&dir, &["dev", "list"]);
    assert!(ok, "dev list should succeed");
    assert!(stdout.contains("alice"), "should list alice: {stdout}");
    assert!(stdout.contains("bob"), "should list bob: {stdout}");
    assert!(stdout.contains("Alice Smith"), "should show name: {stdout}");

    let _ = fs::remove_dir_all(&dir);
}

// --- report effort ---

#[test]
fn report_effort_shows_per_developer() {
    let dir = temp_dir("report-effort");
    let bin = rustwerk_bin();
    let r = |args: &[&str]| {
        Command::new(&bin)
            .args(args)
            .current_dir(&dir)
            .output()
            .expect("failed to run rustwerk");
    };
    r(&["init", "P"]);
    r(&["task", "add", "A", "--id", "A"]);
    r(&["task", "status", "A", "in-progress"]);
    r(&["effort", "log", "A", "3H", "--dev", "alice"]);
    r(&["effort", "log", "A", "2H", "--dev", "bob"]);
    r(&["effort", "log", "A", "1.5H", "--dev", "alice"]);

    let (stdout, _, ok) = run(&dir, &["report", "effort"]);
    assert!(ok, "report effort should succeed");

    // Should show per-developer totals.
    assert!(stdout.contains("alice"), "should list alice: {stdout}");
    assert!(stdout.contains("bob"), "should list bob: {stdout}");
    // alice: 3 + 1.5 = 4.5H
    assert!(stdout.contains("4.5"), "alice should have 4.5H: {stdout}");
    // bob: 2H
    assert!(stdout.contains("2.0"), "bob should have 2.0H: {stdout}");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn report_effort_empty() {
    let dir = temp_dir("report-effort-empty");
    run(&dir, &["init", "P"]);
    let (stdout, _, ok) = run(&dir, &["report", "effort"]);
    assert!(ok);
    assert!(
        stdout.contains("No effort"),
        "should say no effort logged: {stdout}"
    );
    let _ = fs::remove_dir_all(&dir);
}

// --- report bottlenecks ---

#[test]
fn report_bottlenecks_shows_blocking_tasks() {
    let dir = temp_dir("report-bottlenecks");
    let bin = rustwerk_bin();
    let r = |args: &[&str]| {
        Command::new(&bin)
            .args(args)
            .current_dir(&dir)
            .output()
            .expect("failed to run rustwerk");
    };
    r(&["init", "P"]);
    r(&["task", "add", "Foundation", "--id", "A"]);
    r(&["task", "add", "Middle", "--id", "B"]);
    r(&["task", "add", "Leaf", "--id", "C"]);
    r(&["dev", "add", "alice", "Alice Smith"]);
    r(&["task", "assign", "A", "alice"]);
    r(&["task", "depend", "B", "A"]); // B depends on A
    r(&["task", "depend", "C", "B"]); // C depends on B

    let (stdout, _, ok) = run(&dir, &["report", "bottlenecks"]);
    assert!(ok, "report bottlenecks should succeed");

    // A blocks B and C (2 downstream).
    assert!(stdout.contains("A"), "should list task A: {stdout}");
    assert!(
        stdout.contains("alice"),
        "should show assignee alice: {stdout}"
    );
    assert!(stdout.contains("2"), "A should block 2 tasks: {stdout}");
    // A has no deps → ready.
    assert!(stdout.contains("ready"), "A should be ready: {stdout}");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn report_bottlenecks_empty() {
    let dir = temp_dir("report-bottlenecks-empty");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "Solo", "--id", "A"]);
    let (stdout, _, ok) = run(&dir, &["report", "bottlenecks"]);
    assert!(ok);
    assert!(
        stdout.contains("No bottlenecks"),
        "should say no bottlenecks: {stdout}"
    );
    let _ = fs::remove_dir_all(&dir);
}

// --- dev add / dev remove ---

#[test]
fn dev_add_and_list() {
    let dir = temp_dir("dev-add");
    run(&dir, &["init", "P"]);
    let (stdout, _, ok) = run(
        &dir,
        &[
            "dev",
            "add",
            "alice",
            "Alice Smith",
            "--email",
            "alice@example.com",
            "--role",
            "lead",
        ],
    );
    assert!(ok, "dev add should succeed: {stdout}");
    assert!(stdout.contains("alice"));

    // Should appear in dev list.
    let (stdout, _, _) = run(&dir, &["dev", "list"]);
    assert!(stdout.contains("alice"));
    assert!(stdout.contains("Alice Smith"));
    assert!(stdout.contains("alice@example.com"));
    assert!(stdout.contains("lead"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn dev_add_duplicate_rejected() {
    let dir = temp_dir("dev-add-dup");
    run(&dir, &["init", "P"]);
    run(&dir, &["dev", "add", "alice", "Alice"]);
    let (_, _, ok) = run(&dir, &["dev", "add", "alice", "Alice 2"]);
    assert!(!ok, "duplicate dev add should fail");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn dev_remove() {
    let dir = temp_dir("dev-remove");
    run(&dir, &["init", "P"]);
    run(&dir, &["dev", "add", "alice", "Alice"]);
    let (stdout, _, ok) = run(&dir, &["dev", "remove", "alice"]);
    assert!(ok, "dev remove should succeed: {stdout}");

    // Should no longer appear in dev list.
    let (stdout, _, _) = run(&dir, &["dev", "list"]);
    assert!(
        stdout.contains("No developers"),
        "should be empty: {stdout}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn dev_remove_nonexistent_fails() {
    let dir = temp_dir("dev-remove-nope");
    run(&dir, &["init", "P"]);
    let (_, _, ok) = run(&dir, &["dev", "remove", "ghost"]);
    assert!(!ok, "removing nonexistent dev should fail");
    let _ = fs::remove_dir_all(&dir);
}

// --- tree ---

#[test]
fn tree_basic() {
    let dir = temp_dir("tree-basic");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "Root", "--id", "R"]);
    run(&dir, &["task", "add", "Child", "--id", "C"]);
    run(&dir, &["task", "depend", "C", "R"]);
    let (stdout, _, ok) = run(&dir, &["tree"]);
    assert!(ok);
    assert!(stdout.contains("P"), "project name: {stdout}");
    assert!(stdout.contains("R"), "root task: {stdout}");
    assert!(stdout.contains("C"), "child task: {stdout}");
    // Box-drawing chars present.
    assert!(
        stdout.contains('\u{2514}') || stdout.contains('\u{251C}'),
        "box-drawing: {stdout}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn tree_remaining_excludes_done() {
    let dir = temp_dir("tree-remaining");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "Root", "--id", "R"]);
    run(&dir, &["task", "add", "Child", "--id", "C"]);
    run(&dir, &["task", "depend", "C", "R"]);
    run(&dir, &["task", "status", "R", "in-progress"]);
    run(&dir, &["task", "status", "R", "done"]);
    let (stdout, _, ok) = run(&dir, &["tree", "--remaining"]);
    assert!(ok);
    // R is done, only C should appear.
    assert!(!stdout.contains(" R "), "R gone: {stdout}");
    assert!(stdout.contains("C"), "C present: {stdout}");
    let _ = fs::remove_dir_all(&dir);
}

// --- status ---

#[test]
fn status_shows_dashboard() {
    let dir = temp_dir("status-dashboard");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "A", "--id", "A"]);
    run(&dir, &["task", "add", "B", "--id", "B"]);
    run(&dir, &["task", "add", "C", "--id", "C"]);
    run(&dir, &["task", "depend", "B", "A"]);
    run(&dir, &["task", "status", "A", "in-progress"]);
    let (stdout, _, ok) = run(&dir, &["status"]);
    assert!(ok, "status should succeed");
    // Should contain completion percentage.
    assert!(stdout.contains('%'), "pct: {stdout}");
    // Should contain task counts.
    assert!(stdout.contains("done"), "done: {stdout}");
    assert!(stdout.contains("in-progress"), "in-progress: {stdout}");
    // Should mention active tasks.
    assert!(stdout.contains("A"), "active task: {stdout}");
    // Should mention bottleneck count.
    assert!(stdout.contains("bottleneck"), "bottleneck: {stdout}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn status_empty_project() {
    let dir = temp_dir("status-empty");
    run(&dir, &["init", "P"]);
    let (stdout, _, ok) = run(&dir, &["status"]);
    assert!(ok);
    assert!(stdout.contains("0%"), "zero pct: {stdout}");
    let _ = fs::remove_dir_all(&dir);
}

// --- tags ---

#[test]
fn task_add_with_tags() {
    let dir = temp_dir("add-tags");
    run(&dir, &["init", "P"]);
    let (stdout, _, ok) = run(
        &dir,
        &[
            "task",
            "add",
            "Tagged task",
            "--id",
            "T1",
            "--tags",
            "backend,urgent",
        ],
    );
    assert!(ok, "task add --tags failed: {stdout}");

    // Verify tags persisted in project.json.
    let path = dir.join(".rustwerk/project.json");
    let json = fs::read_to_string(&path).unwrap();
    let proj: serde_json::Value = serde_json::from_str(&json).unwrap();
    let tags = &proj["tasks"]["T1"]["tags"];
    assert_eq!(
        tags,
        &serde_json::json!(["backend", "urgent"]),
        "tags should be sorted: {tags}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_update_sets_tags() {
    let dir = temp_dir("update-tags");
    run(&dir, &["init", "P"]);
    run(
        &dir,
        &["task", "add", "T", "--id", "A", "--tags", "old-tag"],
    );
    let (_, _, ok) =
        run(&dir, &["task", "update", "A", "--tags", "new-a,new-b"]);
    assert!(ok, "task update --tags failed");

    let path = dir.join(".rustwerk/project.json");
    let json = fs::read_to_string(&path).unwrap();
    let proj: serde_json::Value = serde_json::from_str(&json).unwrap();
    let tags = &proj["tasks"]["A"]["tags"];
    assert_eq!(
        tags,
        &serde_json::json!(["new-a", "new-b"]),
        "tags should be replaced: {tags}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_update_clears_tags() {
    let dir = temp_dir("update-clear-tags");
    run(&dir, &["init", "P"]);
    run(
        &dir,
        &["task", "add", "T", "--id", "A", "--tags", "backend"],
    );
    let (_, _, ok) = run(&dir, &["task", "update", "A", "--tags", ""]);
    assert!(ok, "clear tags failed");

    let path = dir.join(".rustwerk/project.json");
    let json = fs::read_to_string(&path).unwrap();
    let proj: serde_json::Value = serde_json::from_str(&json).unwrap();
    // Empty tags should be omitted from JSON.
    assert!(
        proj["tasks"]["A"].get("tags").is_none(),
        "empty tags should be omitted"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_add_invalid_tag_fails() {
    let dir = temp_dir("add-bad-tag");
    run(&dir, &["init", "P"]);
    let (_, _, ok) = run(
        &dir,
        &["task", "add", "T", "--id", "A", "--tags", "has spaces"],
    );
    assert!(!ok, "invalid tag should fail");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_list_filter_by_tag() {
    let dir = temp_dir("list-tag");
    run(&dir, &["init", "P"]);
    run(
        &dir,
        &[
            "task",
            "add",
            "Backend work",
            "--id",
            "A",
            "--tags",
            "backend",
        ],
    );
    run(
        &dir,
        &[
            "task",
            "add",
            "Frontend work",
            "--id",
            "B",
            "--tags",
            "frontend",
        ],
    );
    run(
        &dir,
        &[
            "task",
            "add",
            "Both",
            "--id",
            "C",
            "--tags",
            "backend,frontend",
        ],
    );
    run(&dir, &["task", "add", "No tags", "--id", "D"]);

    let (stdout, _, ok) = run(&dir, &["task", "list", "--tag", "backend"]);
    assert!(ok, "list --tag failed");
    assert!(stdout.contains("A"), "should include A: {stdout}");
    assert!(
        !stdout.contains("B ") && !stdout.contains("B\n"),
        "should exclude B (frontend only): {stdout}"
    );
    assert!(
        stdout.contains("C"),
        "should include C (has backend): {stdout}"
    );
    assert!(
        !stdout.contains("D ") && !stdout.contains("D\n"),
        "should exclude D (no tags): {stdout}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_list_tag_combined_with_status() {
    let dir = temp_dir("list-tag-status");
    run(&dir, &["init", "P"]);
    run(
        &dir,
        &["task", "add", "A", "--id", "A", "--tags", "backend"],
    );
    run(
        &dir,
        &["task", "add", "B", "--id", "B", "--tags", "backend"],
    );
    run(&dir, &["task", "status", "A", "in-progress"]);

    // Filter by tag + status should intersect.
    let (stdout, _, ok) = run(
        &dir,
        &[
            "task",
            "list",
            "--tag",
            "backend",
            "--status",
            "in-progress",
        ],
    );
    assert!(ok);
    assert!(stdout.contains("A"), "A matches: {stdout}");
    assert!(
        !stdout.contains(" B"),
        "B is todo, should be excluded: {stdout}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_list_tag_invalid_fails() {
    let dir = temp_dir("list-tag-invalid");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "T", "--id", "A"]);
    let (_, _, ok) = run(&dir, &["task", "list", "--tag", "not valid!"]);
    assert!(!ok, "invalid tag should fail early");
    let _ = fs::remove_dir_all(&dir);
}

// --- task describe ---

#[test]
fn task_describe_shows_file_contents() {
    let dir = temp_dir("describe-shows");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "My Task", "--id", "A"]);

    let tasks_dir = dir.join(".rustwerk/tasks");
    fs::create_dir_all(&tasks_dir).unwrap();
    fs::write(tasks_dir.join("A.md"), "# Task A\n\nDetails here.\n").unwrap();

    let (stdout, _, ok) = run(&dir, &["task", "describe", "A"]);
    assert!(ok, "describe should succeed");
    assert!(stdout.contains("# Task A"));
    assert!(stdout.contains("Details here."));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_describe_no_file_shows_hint() {
    let dir = temp_dir("describe-no-file");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "My Task", "--id", "B"]);

    let (stdout, _, ok) = run(&dir, &["task", "describe", "B"]);
    assert!(ok, "describe should succeed even without file");
    assert!(stdout.contains("No description file for B"));
    assert!(stdout.contains(".rustwerk"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_describe_nonexistent_task_fails() {
    let dir = temp_dir("describe-nonexistent");
    run(&dir, &["init", "P"]);

    let (_, _, ok) = run(&dir, &["task", "describe", "NOPE"]);
    assert!(!ok, "describe should fail for unknown task");
    let _ = fs::remove_dir_all(&dir);
}

// --- version ---

#[test]
fn version_flag_prints_version() {
    let dir = temp_dir("version-flag");
    let (stdout, _, ok) = run(&dir, &["--version"]);
    assert!(ok, "--version should succeed");
    // Output format: "rustwerk X.Y.Z\n"
    let trimmed = stdout.trim();
    let parts: Vec<&str> = trimmed.split(' ').collect();
    assert_eq!(parts[0], "rustwerk", "should start with binary name");
    assert_eq!(
        parts[1].split('.').count(),
        3,
        "version should have 3 dot-separated components"
    );
    let _ = fs::remove_dir_all(&dir);
}

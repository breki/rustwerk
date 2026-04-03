use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Path to the rustwerk binary built by cargo.
fn rustwerk_bin() -> PathBuf {
    // Use the env var set by cargo for workspace binaries.
    if let Ok(path) =
        std::env::var("CARGO_BIN_EXE_rustwerk")
    {
        return PathBuf::from(path);
    }
    // Fallback: navigate from test binary location.
    let mut path = std::env::current_exe()
        .expect("failed to get current exe path");
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
    let dir = std::env::temp_dir().join(format!(
        "rustwerk-cli-test-{}-{name}",
        std::process::id()
    ));
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
    let stdout =
        String::from_utf8_lossy(&output.stdout).to_string();
    let stderr =
        String::from_utf8_lossy(&output.stderr).to_string();
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
    assert!(
        stderr.contains("already exists"),
        "stderr: {stderr}"
    );
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

// --- task add ---

#[test]
fn task_add_with_id() {
    let dir = temp_dir("add-id");
    run(&dir, &["init", "P"]);
    let (stdout, _, ok) = run(
        &dir,
        &["task", "add", "My Task", "--id", "MT"],
    );
    assert!(ok);
    assert!(stdout.contains("Created task MT"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_add_auto_id() {
    let dir = temp_dir("add-auto");
    run(&dir, &["init", "P"]);
    let (stdout, _, ok) =
        run(&dir, &["task", "add", "Auto Task"]);
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
    let (stdout, _, ok) =
        run(&dir, &["task", "status", "A", "in-progress"]);
    assert!(ok);
    assert!(stdout.contains("IN_PROGRESS"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn task_status_invalid_transition() {
    let dir = temp_dir("status-invalid");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "T", "--id", "A"]);
    let (_, _, ok) =
        run(&dir, &["task", "status", "A", "done"]);
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
    let (stdout, _, ok) = run(
        &dir,
        &["task", "status", "A", "todo", "--force"],
    );
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
    let (stdout, _, ok) =
        run(&dir, &["task", "remove", "A"]);
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
    let (stdout, _, ok) = run(
        &dir,
        &["task", "update", "A", "--title", "New"],
    );
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
    let proj_path =
        dir.join(".rustwerk").join("project.json");
    let json = fs::read_to_string(&proj_path).unwrap();
    let mut proj: serde_json::Value =
        serde_json::from_str(&json).unwrap();
    proj["developers"] = serde_json::json!({
        "alice": {"name": "Alice"}
    });
    fs::write(
        &proj_path,
        serde_json::to_string_pretty(&proj).unwrap(),
    )
    .unwrap();
    let (stdout, _, ok) =
        run(&dir, &["task", "assign", "A", "alice"]);
    assert!(ok, "assign should succeed");
    assert!(stdout.contains("alice"));
    let (stdout, _, ok) =
        run(&dir, &["task", "unassign", "A"]);
    assert!(ok);
    assert!(stdout.contains("unassigned"));
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
    let (stdout, _, ok) =
        run(&dir, &["task", "list", "--available"]);
    assert!(ok);
    // A is available (no deps), B is not (depends on A).
    assert!(stdout.contains("A"));
    assert!(!stdout.contains(" B "));
    let _ = fs::remove_dir_all(&dir);
}

// --- depend / undepend ---

#[test]
fn depend_and_undepend() {
    let dir = temp_dir("depend");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "A", "--id", "A"]);
    run(&dir, &["task", "add", "B", "--id", "B"]);
    let (stdout, _, ok) =
        run(&dir, &["task", "depend", "B", "A"]);
    assert!(ok);
    assert!(stdout.contains("depends on"));
    let (stdout, _, ok) =
        run(&dir, &["task", "undepend", "B", "A"]);
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
    let (_, _, ok) =
        run(&dir, &["task", "depend", "B", "A"]);
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
    let (stdout, _, ok) = run(
        &dir,
        &["effort", "log", "A", "2.5H", "--dev", "alice"],
    );
    assert!(ok);
    assert!(stdout.contains("2.5H"));
    let (stdout, _, ok) =
        run(&dir, &["effort", "estimate", "A", "8H"]);
    assert!(ok);
    assert!(stdout.contains("8H"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn effort_log_requires_in_progress() {
    let dir = temp_dir("effort-fail");
    run(&dir, &["init", "P"]);
    run(&dir, &["task", "add", "T", "--id", "A"]);
    let (_, _, ok) = run(
        &dir,
        &["effort", "log", "A", "1H", "--dev", "bob"],
    );
    assert!(!ok, "should fail on TODO task");
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
    let stdout =
        String::from_utf8_lossy(&output.stdout);
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
    assert!(
        !stdout.contains("OK"),
        "rolled-back task should not exist"
    );
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
    let (_, _, ok) =
        run(&dir, &["task", "add", "X", "--id", "X"]);
    assert!(!ok);
    let _ = fs::remove_dir_all(&dir);
}

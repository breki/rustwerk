//! `xtask kg` — knowledge graph build / serve.
//!
//! Thin wrapper around Zola that owns the responsibilities the shell
//! wrappers used to duplicate: resolving the `zola` binary, verifying
//! its integrity, staging `knowledge/` into the site's `content/`
//! directory, and running the build / serve command.
//!
//! # Security posture
//!
//! - **Explicit PATH resolution.** We never invoke `zola` as a bare
//!   name; we resolve to an absolute path by walking `PATH`
//!   ourselves (avoiding Windows `CreateProcess` CWD-first search).
//! - **SHA-256 pinning.** When a pin exists for the current target,
//!   we hash the extracted binary and refuse to execute on mismatch —
//!   both on first download and on every subsequent cache hit.
//! - **Safe extraction.** Archives are extracted into a temp sibling
//!   directory; only the expected `zola[.exe]` file is moved into the
//!   final location, neutering zip-slip / tar-slip entries.
//! - **Marker-guarded staging.** `stage_content` refuses to wipe
//!   `tools/kg/site/content/` unless the directory carries an
//!   `.kg-staged` marker the tool wrote itself, so user-placed files
//!   are preserved.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::run_cmd;

/// Pinned zola release.
const ZOLA_VERSION: &str = "0.20.0";

/// SHA-256 of the *extracted* `zola` binary for the current target
/// triple. `None` means we have not pinned this platform yet; the
/// download path will print a prominent warning and proceed without
/// verification. Add a pin by computing
/// `sha256sum tools/kg/bin/zola[.exe]` after a trusted install.
///
/// The `Option` wrapper is deliberate: it lets unpinned platforms
/// compile today and be tightened later without changing callers.
#[allow(clippy::unnecessary_wraps)]
fn expected_zola_sha256() -> Option<&'static str> {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        Some("3554c43585c4314ce4e59f9562982ebb228560f51d9c3e996cbc42b70decfcc5")
    }
    #[cfg(not(all(target_os = "windows", target_arch = "x86_64")))]
    {
        None
    }
}

/// Marker file written into the staging directory so `stage_content`
/// can recognise a directory it owns before wiping it. The content is
/// a fixed magic string.
const STAGE_MARKER: &str = ".kg-staged";
const STAGE_MARKER_MAGIC: &str = "rustwerk-kg-staging-dir\n";

// ----- zola resolution ---------------------------------------------------

/// Vendored fallback binary path (relative to repo root).
fn vendored_zola_path() -> PathBuf {
    let name = if cfg!(target_os = "windows") {
        "zola.exe"
    } else {
        "zola"
    };
    PathBuf::from("tools").join("kg").join("bin").join(name)
}

/// Resolve zola to an absolute path. Prefer a binary on `PATH` (by
/// explicit directory walk — never the bare-name `Command::new` lookup,
/// which searches CWD first on Windows). Fall back to the vendored
/// copy, downloading on cache miss. Verify the SHA-256 pin on the
/// result before returning.
fn ensure_zola() -> Result<PathBuf, String> {
    let resolved = if let Some(on_path) = find_on_path("zola") {
        on_path
    } else {
        let vendored = vendored_zola_path();
        if !vendored.exists() {
            download_zola(&vendored)?;
        }
        vendored
    };

    verify_binary(&resolved)?;
    Ok(resolved)
}

/// Walk the `PATH` environment variable looking for an executable.
/// Returns the absolute path to the first hit. On Windows we append
/// each `PATHEXT` entry to the candidate name and reject the CWD
/// entirely (empty PATH components, "." etc.) so a CWD-local binary
/// can never win.
fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let exe_candidates = executable_candidates(name);

    for dir in std::env::split_paths(&path) {
        // Reject empty / CWD entries — PATH is allowed to contain them
        // on both Unix and Windows, and we never want to search CWD.
        if dir.as_os_str().is_empty() || dir == Path::new(".") {
            continue;
        }
        for candidate in &exe_candidates {
            let full = dir.join(candidate);
            if full.is_file() {
                return Some(full);
            }
        }
    }
    None
}

/// Executable filenames to try for `name` on this platform. On Windows
/// we honour `PATHEXT`; everywhere else we just look for the bare name.
fn executable_candidates(name: &str) -> Vec<String> {
    if cfg!(target_os = "windows") {
        let pathext = std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
        pathext
            .split(';')
            .filter(|ext| !ext.is_empty())
            .map(|ext| format!("{name}{ext}"))
            .collect()
    } else {
        vec![name.to_string()]
    }
}

// ----- download + verify -------------------------------------------------

/// Download and extract zola into the vendored location. Extraction
/// happens in a sibling temp directory; only the expected executable
/// is moved into place, so archive entries with `../` traversal or
/// absolute paths cannot land outside `tools/kg/bin/`.
fn download_zola(target_bin: &Path) -> Result<(), String> {
    let asset = zola_asset_name();
    let url = format!(
        "https://github.com/getzola/zola/releases/download/v{ZOLA_VERSION}/{asset}"
    );
    let bin_dir = target_bin
        .parent()
        .ok_or("vendored zola path has no parent")?;
    fs::create_dir_all(bin_dir)
        .map_err(|e| format!("mkdir {}: {e}", bin_dir.display()))?;

    // Extract into a sibling scratch dir, not directly into bin_dir,
    // so a malicious archive can only pollute the temp area.
    let scratch = bin_dir.join(".download");
    if scratch.exists() {
        fs::remove_dir_all(&scratch)
            .map_err(|e| format!("clean {}: {e}", scratch.display()))?;
    }
    fs::create_dir_all(&scratch)
        .map_err(|e| format!("mkdir {}: {e}", scratch.display()))?;
    let archive = scratch.join(&asset);

    println!("Downloading zola v{ZOLA_VERSION} from {url}");

    #[cfg(target_os = "windows")]
    download_windows(&url, &archive, &scratch)?;
    #[cfg(not(target_os = "windows"))]
    download_unix(&url, &archive, &scratch)?;

    // Find exactly one `zola[.exe]` file anywhere under the scratch
    // tree and move it into place. Anything else (README, LICENSE,
    // traversal entries) is discarded with the scratch directory.
    let expected_name = target_bin
        .file_name()
        .ok_or("vendored zola path has no filename")?;
    let extracted = locate_extracted_binary(&scratch, expected_name)?;
    fs::rename(&extracted, target_bin)
        .map_err(|e| format!(
            "move {} -> {}: {e}",
            extracted.display(),
            target_bin.display()
        ))?;

    // Drop the entire scratch directory — the archive, any loose
    // companion files, and any traversal debris go with it.
    let _ = fs::remove_dir_all(&scratch);

    Ok(())
}

#[cfg(target_os = "windows")]
fn download_windows(url: &str, archive: &Path, scratch: &Path) -> Result<(), String> {
    // Paths and the URL are threaded through environment variables so
    // no user-controlled string is interpolated into the PowerShell
    // command text. The PowerShell script itself is a compile-time
    // constant.
    run_pwsh(
        "[Net.ServicePointManager]::SecurityProtocol = 'Tls12'; \
         Invoke-WebRequest -UseBasicParsing -Uri $env:KG_URL -OutFile $env:KG_OUT",
        &[("KG_URL", OsStr::new(url)), ("KG_OUT", archive.as_os_str())],
    )?;
    run_pwsh(
        "Expand-Archive -Path $env:KG_ARCHIVE -DestinationPath $env:KG_DEST -Force",
        &[
            ("KG_ARCHIVE", archive.as_os_str()),
            ("KG_DEST", scratch.as_os_str()),
        ],
    )
}

/// Run a `PowerShell` script with a curated environment. Tries
/// `pwsh` (`PowerShell` 7+) first and falls back to `powershell`
/// (Windows `PowerShell` 5.1) for machines that have not installed
/// the former. The script text is a caller-controlled compile-time
/// constant; caller-controlled *data* is passed exclusively via the
/// env map — no interpolation into the script.
#[cfg(target_os = "windows")]
fn run_pwsh(script: &str, env: &[(&str, &OsStr)]) -> Result<(), String> {
    let mut last_err: Option<String> = None;
    for shell in ["pwsh", "powershell"] {
        let mut cmd = std::process::Command::new(shell);
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.args(["-NoProfile", "-Command", script]);
        match cmd.status() {
            Ok(status) if status.success() => return Ok(()),
            Ok(status) => {
                last_err = Some(format!("{shell} exited with {status}"));
            }
            Err(e) => {
                last_err = Some(format!("failed to launch {shell}: {e}"));
            }
        }
    }
    Err(last_err.unwrap_or_else(|| "no PowerShell available".into()))
}

#[cfg(not(target_os = "windows"))]
fn download_unix(url: &str, archive: &Path, scratch: &Path) -> Result<(), String> {
    run_cmd("curl", &[
        OsStr::new("-fsSL"),
        OsStr::new("-o"),
        archive.as_os_str(),
        OsStr::new(url),
    ])?;
    run_cmd("tar", &[
        OsStr::new("xzf"),
        archive.as_os_str(),
        OsStr::new("-C"),
        scratch.as_os_str(),
    ])?;
    Ok(())
}

/// Find the expected zola executable file somewhere under `scratch`.
/// The official release archives drop a single `zola[.exe]` at the
/// top level; this walk tolerates future archive repackaging as long
/// as the filename is preserved.
fn locate_extracted_binary(scratch: &Path, expected_name: &OsStr) -> Result<PathBuf, String> {
    for entry in walk(scratch) {
        if entry.file_name() == Some(expected_name) {
            return Ok(entry);
        }
    }
    Err(format!(
        "no {} found under {}",
        expected_name.to_string_lossy(),
        scratch.display()
    ))
}

/// Minimal recursive file walk — we avoid the `walkdir` dep because
/// xtask keeps its dependency surface small. Symlinks are skipped so
/// a hostile archive cannot trick us into following one out of the
/// scratch directory.
fn walk(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(ty) = entry.file_type() else { continue };
            if ty.is_dir() {
                stack.push(path);
            } else if ty.is_file() {
                out.push(path);
            }
        }
    }
    out
}

/// Verify the binary's SHA-256 against the pinned hash for this
/// target. Unpinned platforms produce a one-line warning and skip
/// verification (strictly an improvement over no pin anywhere).
fn verify_binary(path: &Path) -> Result<(), String> {
    let Some(expected) = expected_zola_sha256() else {
        eprintln!(
            "WARNING: no zola SHA-256 pin for this target; skipping \
             integrity check for {}",
            path.display()
        );
        return Ok(());
    };
    let actual = file_sha256(path)?;
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        // Make the failure loud and helpful; a pin mismatch is
        // either a supply-chain event or a version bump.
        Err(format!(
            "zola binary SHA-256 mismatch at {}\n  expected: {expected}\n  actual:   {actual}\n\
             This may mean the pinned ZOLA_VERSION was updated but the \
             pinned SHA was not, or the binary has been tampered with. \
             Delete {} and re-run to force a fresh download.",
            path.display(),
            path.display()
        ))
    }
}

fn file_sha256(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    let digest = Sha256::digest(&bytes);
    Ok(format!("{digest:x}"))
}

/// The asset name Zola publishes per target tuple.
fn zola_asset_name() -> String {
    let (arch, os, ext) = if cfg!(target_os = "windows") {
        ("x86_64", "pc-windows-msvc", "zip")
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            ("aarch64", "apple-darwin", "tar.gz")
        } else {
            ("x86_64", "apple-darwin", "tar.gz")
        }
    } else if cfg!(target_arch = "aarch64") {
        ("aarch64", "unknown-linux-gnu", "tar.gz")
    } else {
        ("x86_64", "unknown-linux-gnu", "tar.gz")
    };
    format!("zola-v{ZOLA_VERSION}-{arch}-{os}.{ext}")
}

// ----- staging -----------------------------------------------------------

/// Mirror `knowledge/` into `tools/kg/site/content/`. We refuse to
/// wipe the destination unless it carries our marker file so a
/// user-placed directory (hand-authored notes, a symlink) is never
/// silently destroyed.
fn stage_content() -> Result<(), String> {
    let src = PathBuf::from("knowledge");
    let dst = PathBuf::from("tools").join("kg").join("site").join("content");
    if !src.is_dir() {
        return Err(format!(
            "knowledge/ does not exist at {} — run from repo root",
            src.display()
        ));
    }
    if dst.exists() {
        let marker = dst.join(STAGE_MARKER);
        let owned = marker.is_file()
            && fs::read_to_string(&marker)
                .map(|s| s == STAGE_MARKER_MAGIC)
                .unwrap_or(false);
        if !owned {
            return Err(format!(
                "refusing to wipe {} — it exists but has no \
                 {STAGE_MARKER} marker, so it may contain \
                 hand-authored files. Move its contents aside or \
                 delete the directory manually to proceed.",
                dst.display()
            ));
        }
        fs::remove_dir_all(&dst)
            .map_err(|e| format!("rm {}: {e}", dst.display()))?;
    }
    fs::create_dir_all(&dst)
        .map_err(|e| format!("mkdir {}: {e}", dst.display()))?;
    // Write the marker first so a crash mid-copy still leaves the
    // directory recognisably ours for the next run.
    fs::write(dst.join(STAGE_MARKER), STAGE_MARKER_MAGIC)
        .map_err(|e| format!("write marker: {e}"))?;
    copy_dir_recursive(&src, &dst)?;
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    for entry in fs::read_dir(src).map_err(|e| format!("read {}: {e}", src.display()))? {
        let entry = entry.map_err(|e| format!("read entry: {e}"))?;
        let target = dst.join(entry.file_name());
        let ty = entry
            .file_type()
            .map_err(|e| format!("file_type: {e}"))?;
        if ty.is_dir() {
            fs::create_dir_all(&target)
                .map_err(|e| format!("mkdir {}: {e}", target.display()))?;
            copy_dir_recursive(&entry.path(), &target)?;
        } else if ty.is_file() {
            fs::copy(entry.path(), &target)
                .map_err(|e| format!("copy {}: {e}", target.display()))?;
        }
        // Symlinks are intentionally ignored.
    }
    Ok(())
}

// ----- commands ----------------------------------------------------------

fn site_root() -> PathBuf {
    PathBuf::from("tools").join("kg").join("site")
}

/// `cargo xtask kg build`: one-shot build into `tools/kg/site/public/`.
pub fn run_build() -> Result<(), String> {
    let zola = ensure_zola()?;
    stage_content()?;
    let site = site_root();
    run_cmd(&zola, &[OsStr::new("--root"), site.as_os_str(), OsStr::new("build")])?;
    println!("built: {}", site.join("public").display());
    Ok(())
}

/// `cargo xtask kg serve [-- <zola-serve args>]`: live-reload dev
/// server. Extra args after a leading `--` flow through to zola.
pub fn run_serve(extra: &[String]) -> Result<(), String> {
    let zola = ensure_zola()?;
    stage_content()?;
    let site = site_root();

    let mut args: Vec<&OsStr> = vec![
        OsStr::new("--root"),
        site.as_os_str(),
        OsStr::new("serve"),
    ];
    args.extend(extra.iter().map(OsStr::new));
    run_cmd(&zola, &args)
}

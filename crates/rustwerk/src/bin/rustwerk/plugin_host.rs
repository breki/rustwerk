//! Dynamic plugin discovery and loading.
//!
//! This module is the only place in the `rustwerk`
//! binary that uses `unsafe` code — confined by
//! `#![allow(unsafe_code)]` below. All FFI interactions
//! with plugin dynamic libraries happen here.
//!
//! # Trust model
//!
//! Calling `Library::new` on a file executes that
//! shared object's initializers (ELF `.init_array`,
//! Windows `DllMain`) *before* the plugin host can run
//! any validation logic. Every file in a discovery
//! directory is therefore implicitly trusted to run
//! arbitrary code as the current user. The
//! `rustwerk_plugin_api_version` check runs only after
//! the library is loaded, so it cannot prevent a
//! malicious library from compromising the host —
//! only reject libraries that will mis-use the API
//! afterwards.
//!
//! To keep the trust boundary narrow, this module
//! scans only:
//!
//! - `<project>/.rustwerk/plugins/` — project-scoped,
//!   committed to git, reviewable.
//! - `$HOME/.rustwerk/plugins/` (or `%USERPROFILE%`) —
//!   user-scoped install location.
//!
//! A target-directory fallback (`target/debug/`,
//! `target/release/`) is **off by default** and gated
//! behind the `RUSTWERK_PLUGIN_DEV=1` environment
//! variable. Cargo drops unrelated cdylibs into
//! `target/*` (build-script artifacts, dep rlibs on
//! some platforms) that must not be auto-loaded in
//! end-user installs.
//!
//! Plugin-returned strings are governed by the FFI
//! contract (see `rustwerk_plugin_api`). This module
//! enforces a [`MAX_PLUGIN_RESPONSE_BYTES`] size cap
//! on deserialized output after `CStr::from_ptr` has
//! walked to the NUL terminator; plugins that return
//! non-NUL-terminated buffers remain UB per the
//! contract regardless of the cap.

#![allow(unsafe_code)]

use std::env;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::{Path, PathBuf};
use std::ptr;

use anyhow::{anyhow, bail, Context, Result};
use libloading::Library;

use rustwerk_plugin_api::{
    PluginApiVersionFn, PluginFreeStringFn, PluginInfo, PluginInfoFn,
    PluginPushTasksFn, PluginResult, API_VERSION, ERR_OK,
};

/// Maximum size, in bytes, of a plugin-returned JSON
/// string. Prevents a misbehaving plugin from causing
/// unbounded allocation on the host side of the parse.
const MAX_PLUGIN_RESPONSE_BYTES: usize = 10 * 1024 * 1024;

/// Environment variable that, when set to a non-empty
/// value, enables discovery of plugins from the local
/// `target/debug` and `target/release` build
/// directories. Off by default because Cargo deposits
/// unrelated cdylibs there.
const DEV_DIRS_ENV: &str = "RUSTWERK_PLUGIN_DEV";

/// Platform-specific dynamic library extension.
pub(crate) const DYLIB_EXT: &str = if cfg!(target_os = "windows") {
    "dll"
} else if cfg!(target_os = "macos") {
    "dylib"
} else {
    "so"
};

/// A plugin successfully loaded from disk.
///
/// # Invariant
///
/// `push_tasks` and `free_string` **must** point at
/// functions exported from `_library`. Construct only
/// via [`load_plugin`]; do not synthesize fields from
/// outside this module.
///
/// Field declaration order is load-readability, not a
/// safety property — the `Library` holds the
/// reference-counted OS handle, and the cached fn
/// pointers are `Copy` with no `Drop` impl, so they
/// require no particular drop ordering relative to the
/// library. `_library` remains alive for the lifetime
/// of the struct, which is what keeps the fn pointers
/// valid.
pub(crate) struct LoadedPlugin {
    info: PluginInfo,
    source_path: PathBuf,
    push_tasks: PluginPushTasksFn,
    free_string: PluginFreeStringFn,
    _library: Library,
}

impl LoadedPlugin {
    /// Plugin metadata reported at load time.
    pub(crate) fn info(&self) -> &PluginInfo {
        &self.info
    }

    /// Path the plugin was loaded from — useful for
    /// diagnostics.
    pub(crate) fn source_path(&self) -> &Path {
        &self.source_path
    }

    /// Invoke `rustwerk_plugin_push_tasks` with two JSON
    /// payloads (plugin config + tasks array). Returns
    /// the deserialized [`PluginResult`].
    pub(crate) fn push_tasks(
        &self,
        config_json: &str,
        tasks_json: &str,
    ) -> Result<PluginResult> {
        let config = CString::new(config_json)
            .context("plugin config JSON contains an interior NUL")?;
        let tasks = CString::new(tasks_json)
            .context("tasks JSON contains an interior NUL")?;
        let mut out: *mut c_char = ptr::null_mut();
        // Safety: the plugin API contract requires the
        // plugin to either leave `*out` null on error or
        // write a heap-allocated C string whose lifetime
        // runs until `free_string` is invoked. `config`
        // and `tasks` are valid NUL-terminated C strings
        // for the duration of the call.
        let code = unsafe {
            (self.push_tasks)(
                config.as_ptr(),
                tasks.as_ptr(),
                &raw mut out,
            )
        };
        if code != ERR_OK {
            if !out.is_null() {
                // Safety: `out` was written by the
                // plugin in this call; ownership now
                // belongs to us until we call
                // `free_string`.
                unsafe { (self.free_string)(out) };
            }
            bail!(
                "plugin '{}' push_tasks failed with code {code}",
                self.info.name
            );
        }
        if out.is_null() {
            bail!(
                "plugin '{}' returned ERR_OK but a null out-pointer",
                self.info.name
            );
        }
        // Parse first into an owned value so the CStr
        // borrow is dropped before we hand the pointer
        // back to the plugin's allocator.
        let parsed = parse_plugin_response::<PluginResult>(out, &self.info.name);
        // Safety: `out` is still the plugin-allocated
        // pointer, unmodified by us.
        unsafe { (self.free_string)(out) };
        parsed
    }
}

/// Read the plugin-owned C string at `ptr` as JSON of
/// type `T`. Scopes the `CStr` borrow so the caller can
/// free the pointer afterwards without any aliasing
/// concern.
fn parse_plugin_response<T: for<'de> serde::Deserialize<'de>>(
    ptr: *mut c_char,
    plugin_name: &str,
) -> Result<T> {
    // Safety: callers pass a plugin-allocated
    // NUL-terminated C string; see the module-level
    // trust-model notes.
    let len = unsafe { CStr::from_ptr(ptr) }.to_bytes().len();
    if len > MAX_PLUGIN_RESPONSE_BYTES {
        bail!(
            "plugin '{plugin_name}' response exceeds {MAX_PLUGIN_RESPONSE_BYTES} bytes ({len})"
        );
    }
    // Re-borrow to materialise the bytes for
    // deserialization; scope ends before the caller
    // frees.
    let json = unsafe { CStr::from_ptr(ptr) }.to_bytes().to_vec();
    serde_json::from_slice(&json).context("failed to parse plugin response JSON")
}

/// Resolve the directories to scan for plugins. Missing
/// directories are filtered out at scan time. Order
/// determines precedence when multiple copies share a
/// `name`: earlier entries win.
///
/// Order: project (most trusted) → user → optional
/// dev `target/*` (least trusted, gated behind
/// [`DEV_DIRS_ENV`]).
pub(crate) fn discovery_dirs(project_root: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![project_root.join(".rustwerk").join("plugins")];
    if let Some(home) = home_dir() {
        dirs.push(home.join(".rustwerk").join("plugins"));
    }
    if dev_dirs_enabled() {
        dirs.push(project_root.join("target").join("debug"));
        dirs.push(project_root.join("target").join("release"));
    }
    dirs
}

/// True when the operator has opted in to dev-path
/// discovery via [`DEV_DIRS_ENV`]. Empty or missing
/// means disabled.
fn dev_dirs_enabled() -> bool {
    env::var_os(DEV_DIRS_ENV)
        .is_some_and(|v| !v.is_empty() && v != "0")
}

/// Return the user's home directory. Treats empty env
/// values the same as absent ones so that
/// `HOME=` / `USERPROFILE=` cannot cause a
/// CWD-relative plugin scan.
pub(crate) fn home_dir() -> Option<PathBuf> {
    let raw = if cfg!(target_os = "windows") {
        env::var_os("USERPROFILE")
    } else {
        env::var_os("HOME")
    };
    raw.filter(|s| !s.is_empty()).map(PathBuf::from)
}

/// Enumerate candidate plugin files in `dir`. Non-
/// matching extensions are skipped; directory read
/// errors return an empty list rather than propagate —
/// missing plugin dirs are normal.
fn list_plugin_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case(DYLIB_EXT))
        })
        .collect()
}

/// Scan every configured directory and return the set
/// of successfully loaded plugins. Load failures are
/// reported on stderr and skipped so a broken plugin
/// does not break the host. When multiple plugins
/// share a `name`, the first (most trusted directory)
/// wins and later copies are logged as shadowed.
pub(crate) fn discover_plugins(project_root: &Path) -> Vec<LoadedPlugin> {
    let mut loaded: Vec<LoadedPlugin> = Vec::new();
    for dir in discovery_dirs(project_root) {
        for path in list_plugin_files(&dir) {
            match load_plugin(&path) {
                Ok(plugin) => {
                    if let Some(shadowing) =
                        loaded.iter().find(|p| p.info.name == plugin.info.name)
                    {
                        eprintln!(
                            "rustwerk: plugin '{}' at {} shadowed by {} (higher-precedence directory)",
                            plugin.info.name,
                            path.display(),
                            shadowing.source_path.display(),
                        );
                        continue;
                    }
                    loaded.push(plugin);
                }
                Err(e) => {
                    eprintln!(
                        "rustwerk: skipping plugin {}: {e:#}",
                        path.display()
                    );
                }
            }
        }
    }
    loaded
}

/// Load a single plugin from `path`. Rejects anything
/// that does not export all four FFI entry points or
/// reports a different API version.
pub(crate) fn load_plugin(path: &Path) -> Result<LoadedPlugin> {
    // Safety: `Library::new` executes the shared
    // object's constructors. See the module trust-model
    // notes — every file in a discovery directory is
    // implicitly trusted to run arbitrary code.
    let library = unsafe { Library::new(path) }
        .with_context(|| format!("load failed: {}", path.display()))?;

    // Every symbol lookup below is `unsafe` because
    // `Library::get` cannot verify the type we claim
    // matches the exported symbol's signature. We're
    // matching the documented FFI contract from
    // `rustwerk_plugin_api`.
    let version_fn: PluginApiVersionFn = unsafe {
        *library
            .get::<PluginApiVersionFn>(b"rustwerk_plugin_api_version\0")
            .context("missing rustwerk_plugin_api_version symbol")?
    };
    let version = version_fn();
    if version != API_VERSION {
        bail!(
            "API version mismatch: plugin reports {version}, host expects {API_VERSION}; \
             rebuild the plugin against the current rustwerk-plugin-api crate"
        );
    }

    let free_string: PluginFreeStringFn = unsafe {
        *library
            .get::<PluginFreeStringFn>(b"rustwerk_plugin_free_string\0")
            .context("missing rustwerk_plugin_free_string symbol")?
    };
    let info_fn: PluginInfoFn = unsafe {
        *library
            .get::<PluginInfoFn>(b"rustwerk_plugin_info\0")
            .context("missing rustwerk_plugin_info symbol")?
    };
    let push_tasks: PluginPushTasksFn = unsafe {
        *library
            .get::<PluginPushTasksFn>(b"rustwerk_plugin_push_tasks\0")
            .context("missing rustwerk_plugin_push_tasks symbol")?
    };

    // Safety: `info_fn` and `free_string` originate
    // from the library we just loaded and loaded
    // successfully; they implement the API contract.
    let info = unsafe { call_info(info_fn, free_string) }?;
    validate_plugin_name(&info.name)?;

    Ok(LoadedPlugin {
        info,
        source_path: path.to_path_buf(),
        push_tasks,
        free_string,
        _library: library,
    })
}

/// Enforce a narrow character set on plugin-reported
/// names so they can never smuggle ANSI escapes,
/// newlines, or shell-confusing whitespace into host
/// output. Matches the well-known identifier shape used
/// by the built-in jira plugin (`[a-z0-9_-]+`, case-
/// insensitive). Keeps trust-boundary rendering
/// one-line-per-plugin even if a discovery directory
/// ever gains a hostile entry.
fn validate_plugin_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("plugin reported an empty name");
    }
    if name.len() > 64 {
        bail!("plugin name exceeds 64 characters: {} chars", name.len());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!(
            "plugin name contains characters outside [A-Za-z0-9_-]: {name:?}"
        );
    }
    Ok(())
}

/// Invoke the plugin's info function, parse the
/// returned JSON, and free the plugin-owned string.
///
/// # Safety
///
/// `info_fn` and `free_string` must originate from the
/// same dynamic library, implementing the FFI contract.
unsafe fn call_info(
    info_fn: PluginInfoFn,
    free_string: PluginFreeStringFn,
) -> Result<PluginInfo> {
    let mut out: *mut c_char = ptr::null_mut();
    let code = info_fn(&raw mut out);
    if code != ERR_OK {
        if !out.is_null() {
            free_string(out);
        }
        return Err(anyhow!("plugin info returned error code {code}"));
    }
    if out.is_null() {
        return Err(anyhow!("plugin info returned null pointer"));
    }
    let parsed = parse_plugin_response::<PluginInfo>(out, "<unknown>");
    free_string(out);
    parsed
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn dylib_ext_matches_target_os() {
        if cfg!(target_os = "windows") {
            assert_eq!(DYLIB_EXT, "dll");
        } else if cfg!(target_os = "macos") {
            assert_eq!(DYLIB_EXT, "dylib");
        } else {
            assert_eq!(DYLIB_EXT, "so");
        }
    }

    #[test]
    fn discovery_dirs_excludes_target_without_env() {
        // Clear the env var so the test is
        // deterministic even in CI where it may leak in.
        // SAFETY: tests in this crate are single-
        // threaded per binary and `remove_var` only
        // races with other threads reading the
        // environment.
        unsafe { env::remove_var(DEV_DIRS_ENV) };
        let root = PathBuf::from("/tmp/fake-project");
        let dirs = discovery_dirs(&root);
        assert!(dirs.contains(&root.join(".rustwerk").join("plugins")));
        assert!(
            !dirs.iter().any(|p| p.ends_with("target/debug")
                || p.ends_with("target\\debug")),
            "target/debug should be gated behind {DEV_DIRS_ENV}: {dirs:?}"
        );
    }

    #[test]
    fn discovery_dirs_includes_target_with_env() {
        // SAFETY: see note above.
        unsafe { env::set_var(DEV_DIRS_ENV, "1") };
        let root = PathBuf::from("/tmp/fake-project");
        let dirs = discovery_dirs(&root);
        assert!(dirs.contains(&root.join("target").join("debug")));
        assert!(dirs.contains(&root.join("target").join("release")));
        // SAFETY: see note above.
        unsafe { env::remove_var(DEV_DIRS_ENV) };
    }

    #[test]
    fn home_dir_treats_empty_as_absent() {
        let var = if cfg!(target_os = "windows") {
            "USERPROFILE"
        } else {
            "HOME"
        };
        // SAFETY: single-threaded test env.
        let original = env::var_os(var);
        unsafe { env::set_var(var, "") };
        assert!(
            home_dir().is_none(),
            "empty {var} must not produce a home path"
        );
        // Restore.
        match original {
            Some(v) => unsafe { env::set_var(var, v) },
            None => unsafe { env::remove_var(var) },
        }
    }

    #[test]
    fn list_plugin_files_empty_for_missing_dir() {
        let files = list_plugin_files(Path::new("/definitely/not/a/dir"));
        assert!(files.is_empty());
    }

    #[test]
    fn discover_plugins_empty_when_nothing_installed() {
        let dir = std::env::temp_dir().join(format!(
            "rustwerk-plugin-host-empty-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let plugins = discover_plugins(&dir);
        assert!(plugins.is_empty(), "expected none, got {}", plugins.len());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_plugin_name_accepts_simple_id() {
        assert!(validate_plugin_name("jira").is_ok());
        assert!(validate_plugin_name("my_plugin").is_ok());
        assert!(validate_plugin_name("Plugin-42").is_ok());
    }

    #[test]
    fn validate_plugin_name_rejects_empty() {
        assert!(validate_plugin_name("").is_err());
    }

    #[test]
    fn validate_plugin_name_rejects_control_chars() {
        assert!(validate_plugin_name("jira\x1b[2J").is_err());
        assert!(validate_plugin_name("line\nbreak").is_err());
    }

    #[test]
    fn validate_plugin_name_rejects_spaces_and_punctuation() {
        assert!(validate_plugin_name("my plugin").is_err());
        assert!(validate_plugin_name("plugin.name").is_err());
        assert!(validate_plugin_name("../evil").is_err());
    }

    #[test]
    fn validate_plugin_name_rejects_overly_long() {
        let long = "a".repeat(65);
        assert!(validate_plugin_name(&long).is_err());
    }

    #[test]
    fn load_plugin_fails_on_nonexistent_file() {
        let err = load_plugin(Path::new("/definitely/not/a/plugin.so"))
            .err()
            .expect("load should fail for missing file");
        assert!(
            format!("{err:#}").contains("load failed"),
            "unexpected error: {err:#}"
        );
    }
}

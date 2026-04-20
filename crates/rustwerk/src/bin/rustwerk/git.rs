//! Thin shell-outs to the local `git` binary.
//!
//! Kept deliberately small — rustwerk leans on git's
//! behaviour rather than re-implementing it, but full
//! integration lives in Phase 5. This module is where
//! new read-only queries belong so callers do not spawn
//! `git` ad-hoc.

use std::process::Command;

/// Return the user's configured git email, if any.
///
/// Returns `None` when `git` is not on PATH, the command
/// fails (no user.email configured, not a git repo, …),
/// or the output is empty. Never panics.
pub(crate) fn user_email() -> Option<String> {
    let out = Command::new("git")
        .args(["config", "user.email"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_email_returns_option_without_panicking() {
        // CI and dev environments may or may not have a
        // git identity configured; the only contract is
        // that this helper never panics.
        let _: Option<String> = user_email();
    }
}

//! Error type returned by every `gh`-backed fetch.
//!
//! Replaces the previous mix of `expect("gh not installed")` (which panicked
//! inside raw mode and trashed the terminal) and silent `return Vec::new()`
//! parse-failure paths (which left the UI showing "no PRs" with no clue
//! something went wrong).

use std::fmt;

#[derive(Debug)]
pub enum FetchError {
    /// `gh` binary couldn't be spawned at all — e.g. uninstalled mid-session.
    GhMissing,
    /// `gh` ran but returned a non-zero exit code. Stderr is captured so we
    /// can show the user what gh complained about.
    GhFailed {
        code: Option<i32>,
        stderr: String,
    },
    /// `gh`'s JSON output didn't match the schema we expect (different gh
    /// version, server change, etc.).
    ParseFailed(String),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GhMissing => write!(f, "gh CLI not available"),
            Self::GhFailed { code, stderr } => {
                let stderr = stderr.trim();
                match (code, stderr.is_empty()) {
                    (Some(c), false) => write!(f, "gh exited with code {c}: {stderr}"),
                    (Some(c), true) => write!(f, "gh exited with code {c}"),
                    (None, false) => write!(f, "gh failed: {stderr}"),
                    (None, true) => write!(f, "gh failed"),
                }
            }
            Self::ParseFailed(msg) => write!(f, "couldn't parse gh response: {msg}"),
        }
    }
}

/// Run a `gh` subprocess and return its stdout, or a typed error.
///
/// Centralises the "spawn failed" vs "ran but errored" branches so each
/// provider only deals with parsing afterwards.
pub fn run_gh(args: &[&str]) -> Result<Vec<u8>, FetchError> {
    let output = std::process::Command::new("gh")
        .args(args)
        .output()
        .map_err(|_| FetchError::GhMissing)?;
    if !output.status.success() {
        return Err(FetchError::GhFailed {
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(output.stdout)
}

/// Same as `run_gh` but returns `Option`, for best-effort calls where a
/// failure is non-fatal (e.g. per-commit stats).
pub fn try_gh(args: &[&str]) -> Option<Vec<u8>> {
    run_gh(args).ok()
}

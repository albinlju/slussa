//! Error type and shared `gh` helpers for the GitHub provider.

use std::fmt;

#[derive(Debug)]
pub enum FetchError {
    /// `gh` binary couldn't be spawned at all.
    GhMissing,
    /// `gh` ran but returned a non-zero exit code. Stderr is captured so we
    /// can show the user what gh complained about.
    GhFailed { code: Option<i32>, stderr: String },
    /// `gh`'s JSON output didn't match the schema we expect.
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

/// Run a `gh` subprocess and return its stdout.
pub(super) fn run_gh(args: &[&str]) -> Result<Vec<u8>, FetchError> {
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

/// Run a `gh` subprocess and deserialize its stdout as JSON into `T`.
pub(super) fn run_gh_json<T: serde::de::DeserializeOwned>(args: &[&str]) -> Result<T, FetchError> {
    let stdout = run_gh(args)?;
    serde_json::from_slice(&stdout).map_err(|e| FetchError::ParseFailed(e.to_string()))
}

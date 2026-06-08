use std::fmt;

#[derive(Debug)]
pub enum FetchError {
    GhMissing,
    GhFailed { code: Option<i32>, stderr: String },
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

pub(super) fn run_gh(args: &[&str]) -> Result<Vec<u8>, FetchError> {
    tracing::debug!("gh {}", args.join(" "));
    let output = std::process::Command::new("gh")
        .args(args)
        .output()
        .map_err(|err| {
            tracing::warn!("gh failed to spawn: {err}");
            FetchError::GhMissing
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        tracing::warn!("gh exited {:?}: {}", output.status.code(), stderr.trim());
        return Err(FetchError::GhFailed {
            code: output.status.code(),
            stderr,
        });
    }
    Ok(output.stdout)
}

pub(super) fn run_gh_json<T: serde::de::DeserializeOwned>(args: &[&str]) -> Result<T, FetchError> {
    let stdout = run_gh(args)?;
    serde_json::from_slice(&stdout).map_err(|e| {
        let sample: String = String::from_utf8_lossy(&stdout).chars().take(200).collect();
        tracing::warn!("gh json parse failed: {e} (first 200B: {sample})");
        FetchError::ParseFailed(e.to_string())
    })
}

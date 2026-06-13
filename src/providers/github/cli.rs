use crate::providers::error::FetchError;

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

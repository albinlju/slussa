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
        // gh writes its short "(HTTP nnn)" line to stderr but the detailed error
        // body (e.g. 422 validation fields) to stdout — capture both.
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let body = String::from_utf8_lossy(&output.stdout).into_owned();
        let detail = format!("{} {}", stderr.trim(), body.trim());
        let detail = detail.trim().to_string();
        tracing::warn!("gh exited {:?}: {detail}", output.status.code());
        return Err(FetchError::GhFailed {
            code: output.status.code(),
            stderr: detail,
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

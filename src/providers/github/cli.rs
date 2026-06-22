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

/// Like `run_gh` but feeds `stdin` to the process — for `gh api --input -`,
/// where the JSON body (e.g. a review's `comments` array) can't be expressed
/// with `-f` flags.
pub(super) fn run_gh_stdin(args: &[&str], stdin: &[u8]) -> Result<Vec<u8>, FetchError> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    tracing::debug!("gh {} (stdin {}B)", args.join(" "), stdin.len());
    let mut child = Command::new("gh")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| {
            tracing::warn!("gh failed to spawn: {err}");
            FetchError::GhMissing
        })?;
    // Drop the handle after writing so gh sees EOF and proceeds.
    child
        .stdin
        .take()
        .expect("stdin was piped")
        .write_all(stdin)
        .map_err(|err| {
            tracing::warn!("gh stdin write failed: {err}");
            FetchError::GhMissing
        })?;
    let output = child.wait_with_output().map_err(|err| {
        tracing::warn!("gh wait failed: {err}");
        FetchError::GhMissing
    })?;
    if !output.status.success() {
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

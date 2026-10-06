use std::process::Command;

use super::preflight::PreflightError;

pub(crate) fn origin_url() -> Result<String, PreflightError> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                PreflightError::GitMissing
            } else {
                PreflightError::GitFailed(e)
            }
        })?;
    if !output.status.success() {
        return Err(PreflightError::NotAGitRepo);
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// The top of the repository the working directory is in, where its rules files
/// are kept.
pub(crate) fn repo_root() -> Option<std::path::PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!path.is_empty()).then(|| std::path::PathBuf::from(path))
}

pub(crate) fn parse_host(url: &str) -> Option<String> {
    let (authority, _) = crate::git_url::split(url)?;
    let host_port = authority.rsplit('@').next().unwrap_or(authority);
    // The port of an http(s) remote is the web port, so it is part of the
    // host. The port of an ssh remote is the git port and is dropped.
    if url.starts_with("http://") || url.starts_with("https://") {
        return Some(host_port.to_string());
    }
    let host = host_port.split(':').next().unwrap_or(host_port);
    Some(host.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ssh_host() {
        assert_eq!(
            parse_host("git@github.com:albinljung/slussa.git").as_deref(),
            Some("github.com")
        );
    }

    #[test]
    fn parses_https_host() {
        assert_eq!(
            parse_host("https://github.com/albinljung/slussa.git").as_deref(),
            Some("github.com")
        );
    }

    #[test]
    fn parses_https_host_without_dot_git() {
        assert_eq!(
            parse_host("https://github.com/albinljung/slussa").as_deref(),
            Some("github.com")
        );
    }

    #[test]
    fn parses_ssh_url_with_port() {
        assert_eq!(
            parse_host("ssh://git@bitbucket.customer.com:7999/PLAT/payments.git").as_deref(),
            Some("bitbucket.customer.com")
        );
    }

    #[test]
    fn keeps_the_port_of_an_http_remote() {
        assert_eq!(
            parse_host("http://localhost:7990/scm/P/r.git").as_deref(),
            Some("localhost:7990")
        );
    }

    #[test]
    fn rejects_garbage_url() {
        assert!(parse_host("not-a-url").is_none());
    }
}

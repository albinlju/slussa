use std::process::Command;

use super::preflight::PreflightError;

pub(crate) fn origin_host() -> Result<String, PreflightError> {
    let url = origin_url()?;
    parse_host(&url).ok_or(PreflightError::UnparseableRemote { remote: url })
}

pub(super) fn origin_url() -> Result<String, PreflightError> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .map_err(|_| PreflightError::GitMissing)?;
    if !output.status.success() {
        return Err(PreflightError::NotAGitRepo);
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(super) fn parse_host(url: &str) -> Option<String> {
    let (authority, _) = crate::git_url::split(url)?;
    let host_port = authority.rsplit('@').next().unwrap_or(authority);
    let host = host_port.split(':').next().unwrap_or(host_port);
    Some(host.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ssh_host() {
        assert_eq!(
            parse_host("git@github.com:albinljung/tuipr.git").as_deref(),
            Some("github.com")
        );
    }

    #[test]
    fn parses_https_host() {
        assert_eq!(
            parse_host("https://github.com/albinljung/tuipr.git").as_deref(),
            Some("github.com")
        );
    }

    #[test]
    fn parses_https_host_without_dot_git() {
        assert_eq!(
            parse_host("https://github.com/albinljung/tuipr").as_deref(),
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
    fn rejects_garbage_url() {
        assert!(parse_host("not-a-url").is_none());
    }
}

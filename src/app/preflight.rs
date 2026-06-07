//! Startup checks that must pass before the TUI is allowed to come up.
//!
//! Run synchronously in `main` before raw mode so a failure can print a
//! friendly stderr message and exit, instead of panicking inside the
//! alternate screen and leaving the user's terminal broken.

use std::fmt;
use std::process::Command;

#[derive(Debug)]
pub enum PreflightError {
    /// `git remote get-url origin` failed — not a git repo, or no `origin`.
    NotAGitRepo,
    /// Remote URL parsed but the host isn't one we support yet.
    UnsupportedHost { host: String },
    /// `gh` binary is not on PATH.
    GhMissing,
    /// `gh auth status` reported the user is not logged in.
    GhNotAuthenticated { host: String },
}

impl fmt::Display for PreflightError {
    // Messages don't repeat the `tuipr:` prefix — `main` adds it once.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAGitRepo => write!(
                f,
                "must be run inside a git repository with an `origin` remote.",
            ),
            Self::UnsupportedHost { host } => write!(
                f,
                "only GitHub repos are supported for now — this repo's remote is on `{host}`.\n\
                 Bitbucket support is on the roadmap — open an issue at \
                 https://github.com/albinljung/tuipr/issues if you'd like to help.",
            ),
            Self::GhMissing => write!(
                f,
                "the GitHub CLI (`gh`) is not installed.\n\
                 Install it from https://cli.github.com and re-run tuipr.",
            ),
            Self::GhNotAuthenticated { host } => write!(
                f,
                "the GitHub CLI (`gh`) is installed but not authenticated for {host}.\n\
                 Run `gh auth login` then re-run tuipr.",
            ),
        }
    }
}

/// Run all startup checks.
pub fn preflight() -> Result<(), PreflightError> {
    let host = detect_repo_host()?;
    check_gh_installed()?;
    check_gh_auth(&host)?;
    Ok(())
}

/// Resolve the `origin` remote URL and return its host. Only `github.com`
/// is accepted right now; anything else returns `UnsupportedHost` so we
/// can degrade gracefully when Bitbucket/GitLab repos show up.
fn detect_repo_host() -> Result<String, PreflightError> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .map_err(|_| PreflightError::NotAGitRepo)?;
    if !output.status.success() {
        return Err(PreflightError::NotAGitRepo);
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let host = parse_remote_host(&url).ok_or(PreflightError::NotAGitRepo)?;
    if host != "github.com" {
        return Err(PreflightError::UnsupportedHost { host });
    }
    Ok(host)
}

/// Pull the host out of a git remote URL. Supports both SSH
/// (`git@host:owner/repo.git`) and HTTPS (`https://host/owner/repo.git`).
fn parse_remote_host(url: &str) -> Option<String> {
    if let Some(rest) = url.strip_prefix("git@") {
        let (host, _) = rest.split_once(':')?;
        return Some(host.to_string());
    }
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let (host, _) = rest.split_once('/')?;
    Some(host.to_string())
}

fn check_gh_installed() -> Result<(), PreflightError> {
    Command::new("gh")
        .arg("--version")
        .output()
        .map(|_| ())
        .map_err(|_| PreflightError::GhMissing)
}

fn check_gh_auth(host: &str) -> Result<(), PreflightError> {
    let output = Command::new("gh")
        .args(["auth", "status", "-h", host])
        .output()
        .map_err(|_| PreflightError::GhMissing)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(PreflightError::GhNotAuthenticated {
            host: host.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ssh_host() {
        assert_eq!(
            parse_remote_host("git@github.com:albinljung/tuipr.git").as_deref(),
            Some("github.com")
        );
    }

    #[test]
    fn parses_https_host() {
        assert_eq!(
            parse_remote_host("https://github.com/albinljung/tuipr.git").as_deref(),
            Some("github.com")
        );
    }

    #[test]
    fn parses_https_host_without_dot_git() {
        assert_eq!(
            parse_remote_host("https://github.com/albinljung/tuipr").as_deref(),
            Some("github.com")
        );
    }

    #[test]
    fn rejects_garbage_url() {
        assert!(parse_remote_host("not-a-url").is_none());
    }
}

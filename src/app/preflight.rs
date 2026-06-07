//! Startup checks that must pass before the TUI is allowed to come up.
//!
//! We run these synchronously in `main` (before raw mode is entered) so a
//! failure can print a friendly stderr message and exit cleanly instead of
//! panicking inside the alternate screen and leaving the user's terminal
//! in a broken state.

use std::fmt;
use std::process::Command;

/// Where the current repo's remote lives and which owner/repo it's under.
/// We don't currently thread this into provider calls (gh substitutes
/// `{owner}/{repo}` from the cwd) but it lets us verify the repo is on a
/// supported host before launching the TUI.
#[derive(Debug, Clone)]
pub struct RepoContext {
    pub host: String,
    // Reserved for the upcoming layer 2/3 where providers stop relying on
    // gh's implicit `{owner}/{repo}` substitution.
    #[allow(dead_code)]
    pub owner: String,
    #[allow(dead_code)]
    pub repo: String,
}

#[derive(Debug)]
pub enum PreflightError {
    /// `git remote get-url origin` failed — not a git repo, or no `origin`.
    NotAGitRepo,
    /// The remote URL parsed but the host isn't one we support yet.
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
                "only GitHub repos are supported for now — this repo's remote is on `{}`.\n\
                 Bitbucket support is on the roadmap — open an issue at \
                 https://github.com/albinljung/tuipr/issues if you'd like to help.",
                host
            ),
            Self::GhMissing => write!(
                f,
                "the GitHub CLI (`gh`) is not installed.\n\
                 Install it from https://cli.github.com and re-run tuipr.",
            ),
            Self::GhNotAuthenticated { host } => write!(
                f,
                "the GitHub CLI (`gh`) is installed but not authenticated for {}.\n\
                 Run `gh auth login` then re-run tuipr.",
                host
            ),
        }
    }
}

/// Run all startup checks. Returns the resolved repo context on success.
pub fn preflight() -> Result<RepoContext, PreflightError> {
    let ctx = detect_repo()?;
    check_gh_installed()?;
    check_gh_auth(&ctx.host)?;
    Ok(ctx)
}

/// Resolve the `origin` remote URL and parse out (host, owner, repo).
///
/// Supports both SSH (`git@github.com:owner/repo.git`) and HTTPS
/// (`https://github.com/owner/repo[.git]`) URLs. Only `github.com` is
/// accepted for now; anything else returns `UnsupportedHost` so we can
/// degrade gracefully when Bitbucket/GitLab repos show up.
fn detect_repo() -> Result<RepoContext, PreflightError> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .map_err(|_| PreflightError::NotAGitRepo)?;

    if !output.status.success() {
        return Err(PreflightError::NotAGitRepo);
    }

    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let (host, owner, repo) = parse_remote_url(&url).ok_or(PreflightError::NotAGitRepo)?;

    if host != "github.com" {
        return Err(PreflightError::UnsupportedHost { host });
    }
    Ok(RepoContext { host, owner, repo })
}

fn parse_remote_url(url: &str) -> Option<(String, String, String)> {
    // SSH form: git@host:owner/repo(.git)
    if let Some(rest) = url.strip_prefix("git@") {
        let (host, path) = rest.split_once(':')?;
        let (owner, repo) = split_owner_repo(path)?;
        return Some((host.to_string(), owner, repo));
    }
    // HTTPS form: https://host/owner/repo(.git)
    if let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    {
        let (host, path) = rest.split_once('/')?;
        let (owner, repo) = split_owner_repo(path)?;
        return Some((host.to_string(), owner, repo));
    }
    None
}

fn split_owner_repo(path: &str) -> Option<(String, String)> {
    let trimmed = path.trim_end_matches('/').trim_end_matches(".git");
    let (owner, repo) = trimmed.split_once('/')?;
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner.to_string(), repo.to_string()))
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
    fn parses_ssh_url() {
        let parsed = parse_remote_url("git@github.com:albinljung/tuipr.git").unwrap();
        assert_eq!(parsed.0, "github.com");
        assert_eq!(parsed.1, "albinljung");
        assert_eq!(parsed.2, "tuipr");
    }

    #[test]
    fn parses_https_url_with_dot_git() {
        let parsed = parse_remote_url("https://github.com/albinljung/tuipr.git").unwrap();
        assert_eq!(parsed.0, "github.com");
        assert_eq!(parsed.1, "albinljung");
        assert_eq!(parsed.2, "tuipr");
    }

    #[test]
    fn parses_https_url_without_dot_git() {
        let parsed = parse_remote_url("https://github.com/albinljung/tuipr").unwrap();
        assert_eq!(parsed.2, "tuipr");
    }

    #[test]
    fn rejects_garbage_url() {
        assert!(parse_remote_url("not-a-url").is_none());
    }
}

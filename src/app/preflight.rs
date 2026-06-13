use std::fmt;
use std::process::Command;

use crate::providers::{Provider, bitbucket_dc, github};

#[derive(Debug)]
pub enum PreflightError {
    GitMissing,
    NotAGitRepo,
    UnparseableRemote {
        remote: String,
    },
    UnsupportedHost {
        host: String,
    },
    GhMissing,
    GhNotAuthenticated {
        host: String,
    },
    UnknownHost {
        host: String,
        reason: String,
    },
    DcNotAuthenticated {
        host: String,
    },
    DcUnparseableRemote {
        host: String,
        remote: String,
    },
}

impl fmt::Display for PreflightError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GitMissing => write!(f, "git is not installed (or not on PATH)."),
            Self::NotAGitRepo => write!(
                f,
                "must be run inside a git repository with an `origin` remote.",
            ),
            Self::UnparseableRemote { remote } => write!(
                f,
                "couldn't parse a host out of the `origin` remote `{remote}`.",
            ),
            Self::UnsupportedHost { host } => write!(
                f,
                "host `{host}` isn't a recognized GitHub or Bitbucket instance.\n\
                 Bitbucket Cloud (bitbucket.org) support is on the roadmap — open an issue at \
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
            Self::UnknownHost { host, reason } => write!(
                f,
                "couldn't reach `{host}` to detect the provider: {reason}.\n\
                 Check the host is reachable and re-run tuipr.",
            ),
            Self::DcNotAuthenticated { host } => write!(
                f,
                "not logged in to {host}.\n\
                 Run: tuipr auth login",
            ),
            Self::DcUnparseableRemote { host, remote } => write!(
                f,
                "couldn't parse the `origin` remote `{remote}` into a Bitbucket project/repo \
                 on `{host}`.\n\
                 Expected shapes: ssh://git@host/PROJ/repo.git, git@host:PROJ/repo.git, \
                 https://host/scm/PROJ/repo.git.",
            ),
        }
    }
}

pub fn run() -> Result<Provider, PreflightError> {
    let remote = read_origin_remote()?;
    let host = parse_remote_host(&remote).ok_or_else(|| PreflightError::UnparseableRemote {
        remote: remote.clone(),
    })?;
    tracing::info!("detected git remote host: {host}");

    match classify_host(&host)? {
        HostKind::GitHub => {
            if !github::auth::is_installed() {
                return Err(PreflightError::GhMissing);
            }
            if !github::auth::is_authenticated(&host) {
                return Err(PreflightError::GhNotAuthenticated { host });
            }
            tracing::info!("gh auth ok for {host}");
            Ok(Provider::GitHub)
        }
        HostKind::BitbucketDc => {
            let coords = bitbucket_dc::remote::parse(&remote, &host).ok_or_else(|| {
                PreflightError::DcUnparseableRemote {
                    host: host.clone(),
                    remote: remote.clone(),
                }
            })?;
            let pat = crate::app::auth::load_pat(&host)
                .ok_or_else(|| PreflightError::DcNotAuthenticated { host: host.clone() })?;
            tracing::info!(
                "bitbucket dc preflight ok for {}/{}/{}",
                host,
                coords.project_key,
                coords.repo_slug
            );
            Ok(Provider::BitbucketDc(bitbucket_dc::Config {
                repo: coords,
                pat,
            }))
        }
    }
}

enum HostKind {
    GitHub,
    BitbucketDc,
}

fn classify_host(host: &str) -> Result<HostKind, PreflightError> {
    if host == "github.com" {
        return Ok(HostKind::GitHub);
    }
    if host == "bitbucket.org" {
        return Err(PreflightError::UnsupportedHost {
            host: host.to_string(),
        });
    }
    match bitbucket_dc::probe(host) {
        Ok(true) => Ok(HostKind::BitbucketDc),
        Ok(false) => Err(PreflightError::UnsupportedHost {
            host: host.to_string(),
        }),
        Err(reason) => Err(PreflightError::UnknownHost {
            host: host.to_string(),
            reason,
        }),
    }
}

pub(super) fn read_origin_remote() -> Result<String, PreflightError> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .map_err(|_| PreflightError::GitMissing)?;
    if !output.status.success() {
        return Err(PreflightError::NotAGitRepo);
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(super) fn parse_remote_host(url: &str) -> Option<String> {
    if let Some(rest) = url.strip_prefix("ssh://") {
        let (authority, _) = rest.split_once('/')?;
        let host_port = authority.rsplit('@').next().unwrap_or(authority);
        let host = host_port.split(':').next().unwrap_or(host_port);
        return Some(host.to_string());
    }
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
    fn parses_ssh_url_with_port() {
        assert_eq!(
            parse_remote_host("ssh://git@bitbucket.kunden.se:7999/PLAT/payments.git").as_deref(),
            Some("bitbucket.kunden.se")
        );
    }

    #[test]
    fn rejects_garbage_url() {
        assert!(parse_remote_host("not-a-url").is_none());
    }
}

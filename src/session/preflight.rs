use thiserror::Error;

use super::remote;
use crate::providers::{Provider, bitbucket_dc, github};

#[derive(Debug, Error)]
pub enum PreflightError {
    #[error("couldn't identify the logged-in account: {}", .0.user_message())]
    AccountUnknown(#[source] crate::providers::FetchError),
    #[error("git is not installed (or not on PATH).")]
    GitMissing,
    #[error("couldn't run git: {0}.")]
    GitFailed(#[source] std::io::Error),
    #[error("must be run inside a git repository with an `origin` remote.")]
    NotAGitRepo,
    #[error("couldn't parse a host out of the `origin` remote `{remote}`.")]
    UnparseableRemote { remote: String },
    #[error("host `{host}` isn't a recognized GitHub or Bitbucket instance.")]
    UnsupportedHost { host: String },
    #[error(
        "Bitbucket Cloud (bitbucket.org) isn't supported yet — it's on the roadmap.\n\
         Open an issue at {}/issues if you'd like to help.",
        env!("CARGO_PKG_REPOSITORY")
    )]
    BitbucketCloudUnsupported,
    #[error(
        "the GitHub CLI (`gh`) is not installed.\n\
         Install it from https://cli.github.com and re-run slussa."
    )]
    GhMissing,
    #[error(
        "the GitHub CLI (`gh`) is installed but not authenticated for {host}.\n\
         Run `gh auth login` then re-run slussa."
    )]
    GhNotAuthenticated { host: String },
    #[error(
        "couldn't reach `{host}` to detect the provider: {reason}.\n\
         Check the host is reachable and re-run slussa."
    )]
    UnknownHost { host: String, reason: String },
    #[error(
        "not logged in to {host}.\n\
         Run: slussa auth login"
    )]
    DcNotAuthenticated { host: String },
    #[error(
        "couldn't parse the `origin` remote `{remote}` into a Bitbucket project/repo on `{host}`.\n\
         Expected shapes: ssh://git@host/PROJ/repo.git, git@host:PROJ/repo.git, \
         https://host/scm/PROJ/repo.git."
    )]
    DcUnparseableRemote { host: String, remote: String },
}

/// `auth login` stores a Bitbucket Data Center token. For the hosts known not to
/// be one, this says what to do instead, so nothing is asked for or stored.
pub fn login_redirect(host: &str) -> Option<String> {
    match HostKind::named(host)? {
        HostKind::GitHub => Some(
            "GitHub uses the `gh` CLI, so there is no token to store here.\n\
             Run `gh auth login`."
                .into(),
        ),
        HostKind::BitbucketCloud => Some(PreflightError::BitbucketCloudUnsupported.to_string()),
        // Never known by name: its token is what a login stores.
        HostKind::BitbucketDc => None,
    }
}

pub(super) fn run() -> Result<Provider, PreflightError> {
    let remote = remote::origin_url()?;
    let host = remote::parse_host(&remote).ok_or_else(|| PreflightError::UnparseableRemote {
        remote: remote.clone(),
    })?;
    tracing::info!("detected git remote host: {host}");

    match classify_host(&host, &remote)? {
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
        HostKind::BitbucketCloud => Err(PreflightError::BitbucketCloudUnsupported),
        HostKind::BitbucketDc => {
            let repo = bitbucket_dc::remote::locate(&remote, &host).ok_or_else(|| {
                PreflightError::DcUnparseableRemote {
                    host: host.clone(),
                    remote: remote.clone(),
                }
            })?;
            let pat = bitbucket_dc::auth::load_pat(&host)
                .ok_or_else(|| PreflightError::DcNotAuthenticated { host: host.clone() })?;
            tracing::info!(
                "bitbucket dc preflight ok for {}/{}/{}",
                host,
                repo.project_key,
                repo.repo_slug
            );
            Ok(Provider::BitbucketDc(bitbucket_dc::Config { repo, pat }))
        }
    }
}

enum HostKind {
    GitHub,
    BitbucketCloud,
    BitbucketDc,
}

impl HostKind {
    /// The hosts known by name. Any other host is asked whether it is a
    /// Bitbucket Data Center.
    fn named(host: &str) -> Option<Self> {
        match host {
            "github.com" => Some(Self::GitHub),
            "bitbucket.org" => Some(Self::BitbucketCloud),
            _ => None,
        }
    }
}

fn classify_host(host: &str, remote: &str) -> Result<HostKind, PreflightError> {
    if let Some(kind) = HostKind::named(host) {
        return Ok(kind);
    }
    match bitbucket_dc::is_instance(&bitbucket_dc::remote::base_url(remote, host)) {
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

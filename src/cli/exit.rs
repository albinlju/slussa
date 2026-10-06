//! How a command that prints and exits says it failed: one line of JSON on
//! standard error, and an exit code that tells a call that was wrong from one
//! that failed. Such a command never asks for input, so it connects without the
//! interactive login.

use std::process::ExitCode;

use crate::{
    domain::{pr::PrId, printable::printable},
    providers::FetchError,
    session::{self, Session, preflight::PreflightError},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Usage,
    Invalid,
    NotLoggedIn,
    NotFound,
    Failed,
}

impl Kind {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Usage => "usage",
            Self::Invalid => "invalid",
            Self::NotLoggedIn => "not_logged_in",
            Self::NotFound => "not_found",
            Self::Failed => "failed",
        }
    }

    const fn exit(self) -> u8 {
        match self {
            Self::Usage | Self::Invalid => 2,
            Self::NotLoggedIn | Self::NotFound | Self::Failed => 1,
        }
    }
}

pub(super) struct Failure {
    pub(super) kind: Kind,
    pub(super) message: String,
}

impl Failure {
    pub(super) fn new(kind: Kind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// A PR that could not be read: `not_found` when the server said there is
    /// none, and else a failure, which the caller may get past by trying again.
    pub(super) fn unread_pr(pr: PrId, error: &FetchError) -> Self {
        let none = match error {
            FetchError::GhFailed { stderr, .. } => {
                stderr.contains("Could not resolve to a PullRequest") || stderr.contains("HTTP 404")
            }
            FetchError::HttpFailed { status, .. } => *status == 404,
            _ => false,
        };
        let kind = if none { Kind::NotFound } else { Kind::Failed };
        Self::new(kind, format!("PR #{pr}: {}", error.user_message()))
    }
}

/// Say on standard error why `command` failed, and give its exit code.
pub(super) fn report(command: &str, failure: &Failure) -> ExitCode {
    tracing::warn!("{command} failed: {}", failure.kind.name());
    let error = serde_json::json!({
        "schema": 1,
        "error": {"kind": failure.kind.name(), "message": printable(&failure.message)},
    });
    eprintln!("{error}");
    ExitCode::from(failure.kind.exit())
}

/// Connect as a command that never asks: an account that is not logged in is a
/// failure, where `cli::connect` would start the interactive login.
pub(super) fn connect() -> Result<Session, Failure> {
    session::connect().map_err(|e| match e {
        PreflightError::GhNotAuthenticated { .. } => Failure::new(Kind::NotLoggedIn, e.to_string()),
        _ => Failure::new(Kind::Failed, e.to_string()),
    })
}

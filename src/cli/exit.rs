//! How a command that prints and exits says it failed: one line of JSON on
//! standard error, and an exit code that tells a call that was wrong from one
//! that failed. Such a command never asks for input, so it connects without the
//! interactive login.

use std::{
    io::{self, Write},
    process::ExitCode,
};

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
        const NO_SUCH_PR: &str = "Could not resolve to a PullRequest";
        let none = match error {
            FetchError::GhFailed { stderr, .. } => {
                stderr.contains(NO_SUCH_PR) || stderr.contains("HTTP 404")
            }
            FetchError::GraphQl(messages) => {
                messages.iter().any(|message| message.contains(NO_SUCH_PR))
            }
            FetchError::HttpFailed { status, .. } => *status == 404,
            // What reading a PR by its number answers when the server has an
            // empty place where the PR would be.
            FetchError::InvalidInput(_) => true,
            FetchError::GhMissing
            | FetchError::Unsupported(_)
            | FetchError::Stale(_)
            | FetchError::Truncated(_)
            | FetchError::Timeout
            | FetchError::Network(_)
            | FetchError::NotAuthenticated { .. }
            | FetchError::ParseFailed(_)
            | FetchError::WorkerPanicked(_)
            | FetchError::Partial { .. } => false,
        };
        let kind = if none { Kind::NotFound } else { Kind::Failed };
        Self::new(kind, format!("PR #{pr}: {}", error.user_message()))
    }
}

/// Print what `command` answers, and give its exit code. A reader that has
/// stopped reading, as `head` does, has what it wanted: that is not a failure,
/// and `println!` would panic on it.
pub(super) fn print(command: &str, text: &str) -> ExitCode {
    match written(&mut io::stdout().lock(), text) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => report(
            command,
            &Failure::new(Kind::Failed, format!("cannot write the answer: {error}")),
        ),
    }
}

fn written(out: &mut impl Write, text: &str) -> io::Result<()> {
    match writeln!(out, "{text}").and_then(|()| out.flush()) {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        result => result,
    }
}

/// Say on standard error why `command` failed, and give its exit code.
pub(super) fn report(command: &str, failure: &Failure) -> ExitCode {
    tracing::warn!("{command} failed: {}", failure.kind.name());
    let error = serde_json::json!({
        "schema": 1,
        "error": {"kind": failure.kind.name(), "message": printable(&failure.message)},
    });
    // Nobody may be reading this either, and that is not a reason to panic.
    let _ = writeln!(io::stderr().lock(), "{error}");
    ExitCode::from(failure.kind.exit())
}

/// Connect as a command that never asks: an account that is not logged in is a
/// failure, where `cli::connect` would start the interactive login.
pub(super) fn connect() -> Result<Session, Failure> {
    session::connect().map_err(|e| {
        let kind = match e {
            PreflightError::GhNotAuthenticated { .. }
            | PreflightError::DcNotAuthenticated { .. } => Kind::NotLoggedIn,
            PreflightError::AccountUnknown(_)
            | PreflightError::GitMissing
            | PreflightError::GitFailed(_)
            | PreflightError::NotAGitRepo
            | PreflightError::UnparseableRemote { .. }
            | PreflightError::UnsupportedHost { .. }
            | PreflightError::BitbucketCloudUnsupported
            | PreflightError::GhMissing
            | PreflightError::UnknownHost { .. }
            | PreflightError::DcUnparseableRemote { .. } => Kind::Failed,
        };
        Failure::new(kind, e.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pipe whose reader has gone, or a disk that is full.
    struct Closed(io::ErrorKind);

    impl Write for Closed {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(self.0.into())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_reader_that_stopped_reading_is_not_a_failure_and_another_error_is() {
        assert!(written(&mut Closed(io::ErrorKind::BrokenPipe), "the answer").is_ok());
        assert!(written(&mut Closed(io::ErrorKind::StorageFull), "the answer").is_err());
        let mut out = Vec::new();
        written(&mut out, "the answer").unwrap();
        assert_eq!(out, b"the answer\n");
    }

    #[test]
    fn a_number_the_server_has_no_pr_for_is_not_found_however_it_says_so() {
        let kind = |error: FetchError| Failure::unread_pr(PrId(9), &error).kind;
        assert_eq!(
            kind(FetchError::InvalidInput("There is no PR #9.".into())),
            Kind::NotFound
        );
        assert_eq!(
            kind(FetchError::GraphQl(vec![
                "Could not resolve to a PullRequest with the number of 9.".into()
            ])),
            Kind::NotFound
        );
        assert_eq!(kind(FetchError::Timeout), Kind::Failed);
    }
}

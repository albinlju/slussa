//! What the UI asks the application to do, and what comes back from work that
//! ran off the UI thread.

use crate::{
    domain::{
        activity::Activity,
        ci::Build,
        commit::{Commit, CommitOid},
        diff::Diff,
        pr::{Mergeability, PrBatch, PrGroup, PrId, PrInfo, PullRequest},
    },
    providers::{FetchError, MergeError, ReviewError},
    tui::app::{
        commands::Command,
        desktop::{LinkDone, LinkError},
        store::{FetchKey, PrResource, WriteTicket},
    },
};

/// Work for the application, returned by a component's `update`. Only these
/// reach `App`: a message a component handles itself cannot be one.
#[derive(Debug)]
pub enum Effect {
    Quit,
    Navigate(crate::tui::app::navigation::Screen),
    /// Force a re-fetch of the active view now (`F`).
    Refresh,
    OpenPr(PrId),
    /// Read the next older page of each closed group the view shows.
    LoadOlder,
    /// The list shows another view, which may need PRs not read yet.
    LoadView,
    LoadCommitDiff {
        pr_id: PrId,
        oid: CommitOid,
    },
    PrLink {
        pr_id: PrId,
        kind: LinkAction,
    },
    /// Open an issue the PR closes in the browser.
    IssueLink {
        number: u64,
        url: String,
    },
    Command {
        pr_id: PrId,
        command: Command,
    },
    /// Close the error shown on this PR.
    DismissError {
        pr_id: PrId,
    },
    /// Say why something the reader asked for was not done, as an error on
    /// this PR.
    Report {
        pr_id: PrId,
        message: String,
    },
}

/// What work that ran off the UI thread sends back.
#[derive(Debug)]
pub enum TaskResult {
    Read(Read),
    /// A write is over. The ticket names the PR and the operation it was.
    Written {
        ticket: WriteTicket,
        result: Result<(), WriteError>,
    },
    /// Opening or copying a link is over.
    LinkFinished {
        target: LinkTarget,
        result: Result<LinkDone, LinkError>,
    },
}

/// What a read brought back, and of which resource.
#[derive(Debug)]
pub enum Read {
    /// A group of PRs, or with `after` the page of a closed group that follows it.
    Prs {
        group: PrGroup,
        after: Option<String>,
        result: Result<PrBatch, FetchError>,
    },
    /// One PR the reader asked for by its number, which the list may not hold.
    Pr(PrId, Result<PullRequest, FetchError>),
    Commits(PrId, Result<Vec<Commit>, FetchError>),
    Diff(PrId, Result<Diff, FetchError>),
    Builds(PrId, Result<Vec<Build>, FetchError>),
    Activity(PrId, Result<Activity, FetchError>),
    Mergeability(PrId, Result<Mergeability, FetchError>),
    Info(PrId, Result<PrInfo, FetchError>),
    CommitDiff(PrId, CommitOid, Result<Diff, FetchError>),
}

impl Read {
    /// The resource this is a read of: the key its fetch was registered under.
    pub fn key(&self) -> FetchKey {
        match self {
            Self::Prs { group, .. } => FetchKey::Prs(*group),
            Self::Pr(id, _) => FetchKey::One(*id),
            Self::Commits(id, _) => FetchKey::Pr(PrResource::Commits, *id),
            Self::Diff(id, _) => FetchKey::Pr(PrResource::Diff, *id),
            Self::Builds(id, _) => FetchKey::Pr(PrResource::Builds, *id),
            Self::Activity(id, _) => FetchKey::Pr(PrResource::Activity, *id),
            Self::Mergeability(id, _) => FetchKey::Pr(PrResource::Mergeability, *id),
            Self::Info(id, _) => FetchKey::Pr(PrResource::Info, *id),
            Self::CommitDiff(id, oid, _) => FetchKey::Pr(PrResource::CommitDiff(oid.clone()), *id),
        }
    }

    /// Why it failed, if it did.
    pub fn failure(&self) -> Option<&FetchError> {
        match self {
            Self::Prs { result, .. } => result.as_ref().err(),
            Self::Pr(_, result) => result.as_ref().err(),
            Self::Commits(_, result) => result.as_ref().err(),
            Self::Diff(_, result) | Self::CommitDiff(_, _, result) => result.as_ref().err(),
            Self::Builds(_, result) => result.as_ref().err(),
            Self::Activity(_, result) => result.as_ref().err(),
            Self::Mergeability(_, result) => result.as_ref().err(),
            Self::Info(_, result) => result.as_ref().err(),
        }
    }
}

/// Why a write did not go through, or not all of it.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error(transparent)]
    Failed(#[from] FetchError),
    /// The PR was merged; the branch it came from was not deleted.
    #[error("merged, but deleting the branch failed: {0}")]
    BranchDeleteFailed(FetchError),
    /// A review sent as several requests, some of which arrived.
    #[error("review partially sent ({posted_comments} comments): {source}")]
    PartialReview {
        posted_comments: usize,
        /// The summary, when it was among what arrived.
        submitted_summary: Option<String>,
        source: FetchError,
    },
}

impl From<MergeError> for WriteError {
    fn from(error: MergeError) -> Self {
        match error {
            MergeError::Failed(error) => Self::Failed(error),
            MergeError::BranchDeleteFailed(error) => Self::BranchDeleteFailed(error),
        }
    }
}

impl WriteError {
    /// A review's failure, with `summary` kept if it was among what arrived.
    pub fn from_review(error: ReviewError, summary: String) -> Self {
        match error {
            ReviewError::Failed(error) => Self::Failed(error),
            ReviewError::Partial {
                posted_comments,
                summary_posted,
                source,
            } => Self::PartialReview {
                posted_comments,
                submitted_summary: summary_posted.then_some(summary),
                source,
            },
        }
    }

    pub fn user_message(&self) -> String {
        match self {
            Self::Failed(error) => error.user_message(),
            Self::BranchDeleteFailed(error) => format!(
                "Merged, but deleting the branch failed: {}",
                error.user_message()
            ),
            Self::PartialReview {
                posted_comments,
                source,
                ..
            } => format!(
                "{}\n{posted_comments} line comments were sent; confirmed posts will be skipped on retry. Check the last attempted post before retrying.",
                source.user_message()
            ),
        }
    }

    /// Whether the server may have applied the write, or part of it.
    pub const fn may_have_reached_server(&self) -> bool {
        match self {
            Self::Failed(error) => error.may_have_reached_server(),
            Self::BranchDeleteFailed(_) | Self::PartialReview { .. } => true,
        }
    }
}

/// What a link is to, for the notice that says it was opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkTarget {
    Pr(PrId),
    Issue(u64),
}

impl std::fmt::Display for LinkTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pr(pr_id) => write!(f, "PR #{pr_id}"),
            Self::Issue(number) => write!(f, "issue #{number}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkAction {
    Open,
    Copy,
}

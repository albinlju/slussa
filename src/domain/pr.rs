use super::{ci::CiSummary, review::Reviewer, user::User};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrStatus {
    Open,
    Draft,
    Merged,
    Declined,
}

impl PrStatus {
    pub const fn label(&self) -> &str {
        match self {
            Self::Open => "Open",
            Self::Draft => "Draft",
            Self::Merged => "Merged",
            Self::Declined => "Declined",
        }
    }
}

/// What a provider leaves out of the list because it is costly to read for
/// every PR, and gives for one PR when it is opened.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrInfo {
    pub description: Option<String>,
    pub labels: Vec<String>,
}

/// A slice of a repository's PRs that can be read on its own. `Open` holds open
/// and draft PRs together, since a provider does not separate them when asked
/// for the open ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrGroup {
    Open,
    Merged,
    Declined,
}

impl PrGroup {
    pub const ALL: [Self; 3] = [Self::Open, Self::Merged, Self::Declined];

    /// The group a PR belongs to, by its status.
    pub const fn of(status: &PrStatus) -> Self {
        match status {
            PrStatus::Open | PrStatus::Draft => Self::Open,
            PrStatus::Merged => Self::Merged,
            PrStatus::Declined => Self::Declined,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Merged => "merged",
            Self::Declined => "declined",
        }
    }

    /// Open PRs are always read in full. Closed ones are history, so they are
    /// read a page at a time, newest first.
    pub const fn is_paged(self) -> bool {
        !matches!(self, Self::Open)
    }
}

/// PRs read in one go, plus where to continue for older merged and declined
/// ones. `more` is opaque to everything but the provider that made it, and is
/// `None` when nothing older is left to read.
#[derive(Debug, Clone, Default)]
pub struct PrBatch {
    pub prs: Vec<PullRequest>,
    pub more: Option<String>,
}

/// Whether a PR can be merged into its target. What stands in the way is held
/// by the state it belongs to, in words the provider gave or that were derived
/// from its status fields, so a mergeable PR has no reasons to carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mergeability {
    Mergeable,
    Conflicts(Vec<String>),
    /// A provider rule stops the merge for a reason other than a conflict:
    /// missing approvals, required checks, a draft, a branch behind its base.
    Blocked(Vec<String>),
    Unknown,
}

impl Mergeability {
    /// Why it cannot be merged yet; empty when nothing is known to stand in
    /// the way.
    pub fn blockers(&self) -> &[String] {
        match self {
            Self::Conflicts(reasons) | Self::Blocked(reasons) => reasons,
            Self::Mergeable | Self::Unknown => &[],
        }
    }
}

/// How to integrate a PR. Which ones are offered depends on the provider — see
/// `Capabilities::merge_strategies` (GitHub allows all three; Bitbucket DC just merges).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    Merge,
    Squash,
    Rebase,
}

impl MergeStrategy {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Merge => "Merge commit",
            Self::Squash => "Squash and merge",
            Self::Rebase => "Rebase and merge",
        }
    }
}

/// A pull request's number in its repository. A type of its own, so that it
/// cannot be passed where a comment's id is expected, or the other way round.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct PrId(pub u64);

impl std::fmt::Display for PrId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Whether an AI agent's bot account has reviewed the PR, and how that review
/// stands, as the list can tell. When several have, the one that needs the
/// reader most counts: the order is the order of attention.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum AiReview {
    /// None has. An agent that posts under a person's account is not told apart
    /// here, since that takes the text of its comments.
    #[default]
    None,
    /// It reviewed the head the PR has now.
    Current,
    /// Its latest review was of an older head.
    Stale,
    /// It asked for changes.
    ChangesRequested,
}

#[derive(Debug, Clone)]
pub struct PullRequest {
    pub url: Option<String>,
    pub id: PrId,
    pub title: String,
    pub description: Option<String>,
    pub author: User,
    pub ci: CiSummary,
    pub status: PrStatus,
    pub reviewers: Vec<Reviewer>,
    pub labels: Vec<String>,
    pub comment_count: u32,
    pub source_branch: String,
    pub target_branch: String,
    pub additions: u32,
    pub deletions: u32,
    pub changed_files: u32,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub ai_review: AiReview,
    /// The provider says the PR cannot be merged as it is, for a conflict with
    /// the branch it targets. Only an open PR has one; a provider that does not
    /// say, or has not worked it out, leaves it false.
    pub has_conflicts: bool,
}

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

/// A slice of a repository's PRs that can be read on its own. `Open` holds open
/// and draft PRs together, since a provider does not separate them when asked
/// for the open ones.
/// What a provider leaves out of the list because it is costly to read for
/// every PR, and gives for one PR when it is opened.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrInfo {
    pub description: Option<String>,
    pub labels: Vec<String>,
}

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
/// `Provider::merge_strategies` (GitHub allows all three; Bitbucket DC just merges).
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

#[derive(Debug, Clone)]
pub struct PullRequest {
    pub url: Option<String>,
    pub id: u64,
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
}

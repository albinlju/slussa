use super::{
    ci::CiSummary,
    review::{ReviewedHead, Reviewer},
    user::User,
};
use chrono::{DateTime, Utc};

/// Where a PR is in its life. What only an open PR has, being a draft and
/// having a conflict, is in the variant for an open PR, so a merged or declined
/// PR cannot be written with either.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrStatus {
    Open(OpenPr),
    Merged,
    Declined,
}

/// What an open PR can be besides open.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OpenPr {
    /// The author has not asked for a review yet.
    pub draft: bool,
    pub conflicts: Conflicts,
}

/// Whether an open PR conflicts with the branch it targets, as far as the
/// provider says.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Conflicts {
    Yes,
    No,
    /// The provider does not say in the list, or has not worked it out yet.
    #[default]
    Unknown,
}

impl PrStatus {
    /// An open PR that is ready for review, about which nothing more is known.
    pub const fn open() -> Self {
        Self::Open(OpenPr {
            draft: false,
            conflicts: Conflicts::Unknown,
        })
    }

    /// An open PR that is a draft, about which nothing more is known.
    #[cfg(test)]
    pub const fn draft() -> Self {
        Self::Open(OpenPr {
            draft: true,
            conflicts: Conflicts::Unknown,
        })
    }

    /// An open PR that is ready for review and has a conflict.
    #[cfg(test)]
    pub const fn conflicting() -> Self {
        Self::Open(OpenPr {
            draft: false,
            conflicts: Conflicts::Yes,
        })
    }

    pub const fn label(&self) -> &str {
        match self {
            Self::Open(OpenPr { draft: false, .. }) => "Open",
            Self::Open(OpenPr { draft: true, .. }) => "Draft",
            Self::Merged => "Merged",
            Self::Declined => "Declined",
        }
    }

    /// Open and not a draft: the PRs that ask for a reader.
    pub const fn is_ready(&self) -> bool {
        matches!(self, Self::Open(OpenPr { draft: false, .. }))
    }

    /// Whether the provider says an open PR has no conflict. Not the same as
    /// not having one that is known: where it does not say, it is neither.
    pub const fn is_conflict_free(&self) -> bool {
        matches!(
            self,
            Self::Open(OpenPr {
                conflicts: Conflicts::No,
                ..
            })
        )
    }

    pub const fn has_conflicts(&self) -> bool {
        matches!(
            self,
            Self::Open(OpenPr {
                conflicts: Conflicts::Yes,
                ..
            })
        )
    }
}

/// What a provider leaves out of the list because it is costly to read for
/// every PR, and gives for one PR when it is opened.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrInfo {
    pub description: Option<String>,
    pub labels: Vec<String>,
    /// The issues the PR closes when it is merged.
    pub issues: Vec<LinkedIssue>,
}

impl PrInfo {
    /// The issues that can be opened: those the provider gave an address for,
    /// with it.
    pub fn issues_to_open(&self) -> Vec<(&LinkedIssue, &str)> {
        self.issues
            .iter()
            .filter_map(|issue| Some((issue, issue.url.as_deref()?)))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedIssue {
    pub number: u64,
    pub title: String,
    /// Where the issue is, when the provider says; it may be in another repository.
    pub url: Option<String>,
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
            PrStatus::Open(_) => Self::Open,
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
    /// The provider merges it by itself with `strategy` once nothing stands in
    /// the way; `waiting` is what still does.
    AutoMerge {
        strategy: MergeStrategy,
        waiting: Vec<String>,
    },
    Unknown,
}

impl Mergeability {
    /// Why it cannot be merged yet; empty when nothing is known to stand in
    /// the way.
    pub fn blockers(&self) -> &[String] {
        match self {
            Self::Conflicts(reasons)
            | Self::Blocked(reasons)
            | Self::AutoMerge {
                waiting: reasons, ..
            } => reasons,
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

impl PrId {
    /// A PR number as a person writes it, `44` or `#44`: digits and nothing
    /// else, and not zero.
    pub fn parse(text: &str) -> Option<Self> {
        let digits = text.strip_prefix('#').unwrap_or(text);
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        digits.parse().ok().filter(|&number| number != 0).map(Self)
    }
}

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

#[cfg(test)]
impl PullRequest {
    /// An open PR by `alice`, with nothing in it, last updated at `updated`.
    pub fn for_test(id: u64, updated: DateTime<Utc>) -> Self {
        Self {
            url: None,
            id: PrId(id),
            title: String::new(),
            description: None,
            author: User {
                username: "alice".into(),
            },
            ci: CiSummary::Unknown,
            status: PrStatus::open(),
            reviewers: vec![],
            labels: vec![],
            comment_count: 0,
            source_branch: String::new(),
            source_repo: SourceRepo::Unknown,
            head_oid: None,
            target_branch: String::new(),
            additions: 0,
            deletions: 0,
            changed_files: 0,
            created: updated,
            updated,
            ai_review: AiReview::None,
        }
    }
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
    pub source_repo: SourceRepo,
    /// The commit the source branch was at when the list was read.
    pub head_oid: Option<String>,
    pub target_branch: String,
    pub additions: u32,
    pub deletions: u32,
    pub changed_files: u32,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub ai_review: AiReview,
}

/// What is asked of merging a PR by itself once it is ready.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoMerge {
    /// Merge with `strategy` once the checks and reviews allow it, if the
    /// branch is still at `head` now.
    On {
        strategy: MergeStrategy,
        head: ReviewedHead,
    },
    Off,
}

/// Whether the source branch lives in the repository the PR targets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SourceRepo {
    /// The same repository: the branch is the author's own to delete.
    Same,
    /// A fork: a branch of that name in this repository is another branch.
    Fork,
    /// The provider does not say.
    #[default]
    Unknown,
}

/// A source branch that may be deleted once its PR is merged: in the same
/// repository as the PR's target, and not the target itself. Only
/// [`DeletableBranch::of`] makes one, so a delete cannot name a fork's branch
/// or the branch merged into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletableBranch(String);

impl DeletableBranch {
    pub fn of(pr: &PullRequest) -> Option<Self> {
        let named = !pr.source_branch.is_empty() && pr.source_branch != pr.target_branch;
        match pr.source_repo {
            SourceRepo::Same if named => Some(Self(pr.source_branch.clone())),
            SourceRepo::Same | SourceRepo::Fork | SourceRepo::Unknown => None,
        }
    }

    pub fn name(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{DeletableBranch, PrId, PullRequest, SourceRepo};

    #[test]
    fn a_pr_number_is_digits_with_or_without_a_hash() {
        assert_eq!(PrId::parse("44"), Some(PrId(44)));
        assert_eq!(PrId::parse("#44"), Some(PrId(44)));
        assert_eq!(PrId::parse("007"), Some(PrId(7)));
    }

    #[test]
    fn anything_else_is_not_a_pr_number() {
        for text in [
            "", "#", "0", "#0", "-4", "4x", "x4", "4 4", " 4", "4.0", "##4", "+4", "auth",
        ] {
            assert_eq!(PrId::parse(text), None, "{text:?}");
        }
        // Too large for a number: not one.
        assert_eq!(PrId::parse("99999999999999999999999"), None);
    }

    fn pr_from(source: &str, repo: SourceRepo) -> PullRequest {
        PullRequest {
            source_branch: source.into(),
            source_repo: repo,
            target_branch: "main".into(),
            ..PullRequest::for_test(1, chrono::Utc::now())
        }
    }

    #[test]
    fn only_a_branch_of_this_repository_other_than_the_target_may_be_deleted() {
        let ok = DeletableBranch::of(&pr_from("feature/x", SourceRepo::Same));
        assert_eq!(ok.as_ref().map(DeletableBranch::name), Some("feature/x"));
        assert_eq!(
            DeletableBranch::of(&pr_from("feature", SourceRepo::Fork)),
            None
        );
        assert_eq!(
            DeletableBranch::of(&pr_from("feature", SourceRepo::Unknown)),
            None
        );
        assert_eq!(
            DeletableBranch::of(&pr_from("main", SourceRepo::Same)),
            None
        );
        assert_eq!(DeletableBranch::of(&pr_from("", SourceRepo::Same)), None);
    }
}

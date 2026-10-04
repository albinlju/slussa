//! Features implemented by a provider adapter. This describes support, not
//! repository permissions or whether a particular PR is currently actionable.
use super::{pr::MergeStrategy, review::ReviewVerdict};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {
    PrComments,
    InlineComments,
    Replies,
    EditComments,
    DeleteComments,
    ResolveThreads,
    Builds,
    Mergeability,
    ClosePr,
    ReopenPr,
    AutoMerge,
    /// The list omits the description and labels; they are read per PR.
    PrInfo,
}

/// How a review with queued line comments reaches the provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewSubmission {
    /// One request, whose comments are all on one diff revision.
    AtomicSingleRevision,
    /// The comments one by one, then the summary and the verdict.
    Sequential,
}

/// How a provider takes reviews. Present only where it takes them, so there
/// is one way to say that it does not.
#[derive(Debug, Clone)]
pub struct ReviewCaps {
    pub verdicts: Vec<ReviewVerdict>,
    /// The verdicts the author of a PR may give on it.
    pub own_pr_verdicts: Vec<ReviewVerdict>,
    pub submission: ReviewSubmission,
}

#[derive(Debug, Clone, Default)]
pub struct Capabilities {
    pub features: HashSet<Feature>,
    pub review: Option<ReviewCaps>,
    pub merge_strategies: Vec<MergeStrategy>,
}

impl Capabilities {
    pub fn supports(&self, feature: Feature) -> bool {
        self.features.contains(&feature)
    }
    pub const fn reviews(&self) -> bool {
        self.review.is_some()
    }
    /// The verdicts on offer; none where reviews are not taken.
    pub fn review_verdicts(&self) -> &[ReviewVerdict] {
        self.review.as_ref().map_or(&[], |r| &r.verdicts)
    }
    pub fn own_pr_verdicts(&self) -> &[ReviewVerdict] {
        self.review.as_ref().map_or(&[], |r| &r.own_pr_verdicts)
    }
    pub fn can_submit_verdict(&self, verdict: ReviewVerdict, own_pr: bool) -> bool {
        self.review_verdicts().contains(&verdict)
            && (!own_pr || self.own_pr_verdicts().contains(&verdict))
    }
}

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
    /// The list omits the description and labels; they are read per PR.
    PrInfo,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReviewSubmission {
    #[default]
    Unsupported,
    AtomicSingleRevision,
    Sequential,
}

#[derive(Debug, Clone, Default)]
pub struct Capabilities {
    pub features: HashSet<Feature>,
    pub review_verdicts: Vec<ReviewVerdict>,
    pub own_pr_verdicts: Vec<ReviewVerdict>,
    pub review_submission: ReviewSubmission,
    pub merge_strategies: Vec<MergeStrategy>,
}

impl Capabilities {
    pub fn supports(&self, feature: Feature) -> bool {
        self.features.contains(&feature)
    }
    pub fn reviews(&self) -> bool {
        self.review_submission != ReviewSubmission::Unsupported && !self.review_verdicts.is_empty()
    }
    pub fn can_submit_verdict(&self, verdict: ReviewVerdict, own_pr: bool) -> bool {
        self.reviews()
            && self.review_verdicts.contains(&verdict)
            && (!own_pr || self.own_pr_verdicts.contains(&verdict))
    }
}

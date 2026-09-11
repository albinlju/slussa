use crate::domain::review::ReviewVerdict;

#[derive(Debug, Clone)]
pub struct CommentAnchor {
    pub path: String,
    pub line: usize,
    pub removed: bool,
}

#[derive(Debug, Clone)]
pub enum CommentTarget {
    Line(CommentAnchor),
    Pr,
    Reply(u64),
    /// Editing an existing comment; `review` picks the right provider endpoint.
    Edit {
        id: u64,
        review: bool,
    },
    /// The summary body of a review verdict that carries one (request changes /
    /// comment).
    Review {
        verdict: ReviewVerdict,
    },
}

/// A review being assembled before submission. Line comments accumulate here
/// (only locally — nothing is sent) until a verdict flushes them in one go.
#[derive(Debug, Default, Clone)]
pub struct PendingReview {
    pub comments: Vec<PendingComment>,
}

#[derive(Debug, Clone)]
pub struct PendingComment {
    pub anchor: CommentAnchor,
    pub text: String,
}

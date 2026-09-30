//! Comment and review drafts, before anything is sent to a provider.
//!
//! A `CommentTarget` says where a comment goes. `Line` carries a
//! `CommentAnchor`, which includes the `DiffRevision` the user was looking at,
//! so a comment is never re-pointed at a newer commit. `PendingReview`
//! collects line comments locally until a verdict submits them together.
//! Everything here is serializable because `drafts` persists it.

use crate::domain::review::ReviewVerdict;

pub use crate::domain::review::CommentAnchor;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct PendingReview {
    pub submitted_summary: Option<String>,
    pub comments: Vec<PendingComment>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PendingComment {
    pub anchor: CommentAnchor,
    pub text: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommentDraft {
    pub target: CommentTarget,
    pub text: String,
}

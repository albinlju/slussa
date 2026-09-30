use super::user::User;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewerState {
    Approved,
    ChangesRequested,
    Commented,
    /// Asked to review and has not yet; also a re-request after an earlier review.
    Requested,
}

#[derive(Debug, Clone)]
pub struct Reviewer {
    pub author: User,
    pub state: ReviewerState,
}

/// A review submission's verdict. `Unapprove` (withdraw approval) is only offered
/// where a provider supports it — see `Provider::can_unapprove`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ReviewVerdict {
    Approve,
    RequestChanges,
    Comment,
    Unapprove,
}

impl ReviewVerdict {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Approve => "Approve",
            Self::RequestChanges => "Request changes",
            Self::Comment => "Comment",
            Self::Unapprove => "Unapprove",
        }
    }

    /// Verdicts carrying a summary body (required by GitHub for these two).
    pub const fn needs_body(self) -> bool {
        matches!(self, Self::RequestChanges | Self::Comment)
    }
}

/// One inline (diff-line) comment published as part of a batched review
/// submission — see `Provider::submit_full_review`.
#[derive(Debug, Clone)]
pub struct ReviewComment {
    pub revision: Option<super::diff::DiffRevision>,
    pub path: String,
    pub line: usize,
    /// The line sits on the removed (old) side of the diff.
    pub removed: bool,
    pub body: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommentAnchor {
    pub revision: Option<crate::domain::diff::DiffRevision>,
    pub path: String,
    pub line: usize,
    pub removed: bool,
}

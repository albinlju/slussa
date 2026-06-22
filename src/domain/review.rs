use super::user::User;

#[derive(Debug, Clone, PartialEq)]
pub enum ReviewerState {
    Approved,
    ChangesRequested,
    Commented,
}

#[derive(Debug, Clone)]
pub struct Reviewer {
    pub author: User,
    pub state: ReviewerState,
}

/// A review submission's verdict. `Unapprove` (withdraw approval) is only offered
/// where a provider supports it — see `Provider::can_unapprove`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewVerdict {
    Approve,
    RequestChanges,
    Comment,
    Unapprove,
}

impl ReviewVerdict {
    pub fn label(self) -> &'static str {
        match self {
            Self::Approve => "Approve",
            Self::RequestChanges => "Request changes",
            Self::Comment => "Comment",
            Self::Unapprove => "Unapprove",
        }
    }

    /// Verdicts carrying a summary body (required by GitHub for these two).
    pub fn needs_body(self) -> bool {
        matches!(self, Self::RequestChanges | Self::Comment)
    }
}

/// One inline (diff-line) comment published as part of a batched review
/// submission — see `Provider::submit_full_review`.
#[derive(Debug, Clone)]
pub struct ReviewComment {
    pub path: String,
    pub line: usize,
    /// The line sits on the removed (old) side of the diff.
    pub removed: bool,
    pub body: String,
}

//! The writes a PR can have pending, one at a time: what each says when it went
//! through and what it changes at once. `Store::begin_write` records one.

use crate::domain::pr::PrStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Comment,
    Moderation,
    Review,
    Merge,
    AutoMerge,
    CancelAutoMerge,
    RerunBuilds,
    RerequestReview,
    Decline,
    Reopen,
}

impl Operation {
    /// What the notice says once it went through.
    pub const fn done_label(self) -> &'static str {
        match self {
            Self::Merge => "merged",
            Self::AutoMerge => "will merge when ready",
            Self::CancelAutoMerge => "auto-merge off",
            Self::RerunBuilds => "failed builds run again",
            Self::RerequestReview => "asked to review again",
            Self::Decline => "closed / declined",
            Self::Reopen => "reopened",
            Self::Review => "review submitted",
            Self::Comment => "comment saved",
            Self::Moderation => "comment / thread updated",
        }
    }

    /// The status the PR has once it went through, if it changes it.
    pub const fn moves_pr_to(self) -> Option<PrStatus> {
        match self {
            Self::Merge => Some(PrStatus::Merged),
            Self::Decline => Some(PrStatus::Declined),
            // Whether it is a draft or has a conflict is read with the list again.
            Self::Reopen => Some(PrStatus::open()),
            Self::Comment
            | Self::Moderation
            | Self::Review
            | Self::AutoMerge
            | Self::CancelAutoMerge
            | Self::RerunBuilds
            | Self::RerequestReview => None,
        }
    }

    /// Whether it sends what is in the comment editor.
    pub const fn sends_editor_text(self) -> bool {
        match self {
            Self::Comment | Self::Review => true,
            Self::Moderation
            | Self::Merge
            | Self::AutoMerge
            | Self::CancelAutoMerge
            | Self::RerunBuilds
            | Self::RerequestReview
            | Self::Decline
            | Self::Reopen => false,
        }
    }
}

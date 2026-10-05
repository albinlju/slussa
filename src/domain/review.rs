use super::{
    comment::{CommentId, CommentKey},
    user::{User, Username},
};

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

/// The people who asked for changes, to be asked to look again. Never empty:
/// only [`Rerequest::of`] makes one, and it makes none when nobody asked for
/// changes, so a request for no one cannot be sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rerequest(Vec<Username>);

impl Rerequest {
    pub fn of(reviewers: &[Reviewer]) -> Option<Self> {
        let who: Vec<Username> = reviewers
            .iter()
            .filter(|reviewer| reviewer.state == ReviewerState::ChangesRequested)
            .filter_map(|reviewer| Username::parse(&reviewer.author.username))
            .collect();
        (!who.is_empty()).then_some(Self(who))
    }

    pub fn names(&self) -> &[Username] {
        &self.0
    }
}

/// A review submission's verdict. `Unapprove` (withdraw approval) is only offered
/// where a provider lists it — see `ReviewCaps::verdicts`.
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

/// One comment on a line, as a provider posts it. Its revision is known: an
/// anchor made on a diff whose revision was not read cannot become one, so no
/// provider has to ask.
#[derive(Debug, Clone)]
pub struct ReviewComment {
    pub revision: super::diff::DiffRevision,
    pub path: String,
    pub line: super::diff::LineRef,
    pub body: String,
}

impl ReviewComment {
    /// None when the anchor was made on a diff with an unknown revision.
    pub fn new(anchor: CommentAnchor, body: String) -> Option<Self> {
        let line = if anchor.removed {
            super::diff::LineRef::Old(anchor.line)
        } else {
            super::diff::LineRef::New(anchor.line)
        };
        Some(Self {
            revision: anchor.revision?,
            path: anchor.path,
            line,
            body,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommentAnchor {
    pub revision: Option<crate::domain::diff::DiffRevision>,
    pub path: String,
    pub line: usize,
    pub removed: bool,
}

// Drafts: what is written before anything is sent. A `CommentTarget` says where
// a comment goes; `Line` carries a `CommentAnchor`, which includes the
// `DiffRevision` the user was looking at, so a comment is never re-pointed at a
// newer commit. `PendingReview` collects line comments locally until a verdict
// submits them together. The local drafts file serializes all of these; their
// spelling on disk changes only with a new file version.

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CommentTarget {
    Line(CommentAnchor),
    Pr,
    Reply(CommentId),
    /// Editing an existing comment.
    Edit(CommentKey),
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

#[cfg(test)]
mod rerequest_tests {
    use super::*;

    fn reviewer(name: &str, state: ReviewerState) -> Reviewer {
        Reviewer {
            author: User {
                username: name.into(),
            },
            state,
        }
    }

    #[test]
    fn only_those_who_asked_for_changes_are_asked_again() {
        let who = Rerequest::of(&[
            reviewer("alice", ReviewerState::ChangesRequested),
            reviewer("bob", ReviewerState::Approved),
            reviewer("carol", ReviewerState::Commented),
            reviewer("dave", ReviewerState::Requested),
            reviewer("erin", ReviewerState::ChangesRequested),
        ])
        .expect("two asked for changes");
        let names: Vec<&str> = who.names().iter().map(Username::as_str).collect();
        assert_eq!(names, ["alice", "erin"]);
    }

    #[test]
    fn nobody_asking_for_changes_is_no_request() {
        assert_eq!(Rerequest::of(&[]), None);
        assert_eq!(
            Rerequest::of(&[
                reviewer("bob", ReviewerState::Approved),
                reviewer("dave", ReviewerState::Requested),
            ]),
            None
        );
        assert_eq!(
            Rerequest::of(&[reviewer(" ", ReviewerState::ChangesRequested)]),
            None,
            "an account without a name is not someone to ask"
        );
    }
}

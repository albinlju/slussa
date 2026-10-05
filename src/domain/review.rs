use super::{
    comment::{CommentId, CommentKey},
    diff::Diff,
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

/// The head commit of a PR as the reviewer saw it, which a verdict or a merge
/// is tied to: the provider refuses it if the branch has moved since, so what
/// is approved or merged is what was read. Only [`ReviewedHead::of`] makes one,
/// from what was loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedHead(String);

impl ReviewedHead {
    /// What was read: the head of the PR's diff when it has been loaded, and
    /// otherwise the head the list last gave. `None` where neither says.
    pub fn of(diff: Option<&Diff>, listed: Option<&str>) -> Option<Self> {
        let read = diff
            .and_then(|diff| diff.revision.as_ref())
            .filter(|revision| !revision.commit)
            .map(|revision| revision.head.as_str());
        read.or(listed)
            .filter(|head| !head.is_empty())
            .map(|head| Self(head.to_owned()))
    }

    pub fn as_str(&self) -> &str {
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

#[cfg(test)]
mod reviewed_head_tests {
    use super::*;
    use crate::domain::diff::DiffRevision;

    fn diff_at(head: &str, commit: bool) -> Diff {
        Diff {
            revision: Some(DiffRevision {
                head: head.into(),
                base: None,
                commit,
            }),
            files: vec![],
        }
    }

    /// A head known only by its listing.
    fn named(sha: &str) -> Option<ReviewedHead> {
        ReviewedHead::of(None, Some(sha))
    }

    #[test]
    fn what_was_read_is_the_head_of_the_loaded_diff_before_the_listed_one() {
        let read = diff_at("def456", false);
        assert_eq!(
            ReviewedHead::of(Some(&read), Some("abc123")),
            named("def456"),
            "the diff the reader saw, not the list's newer head"
        );
    }

    #[test]
    fn without_a_diff_the_listed_head_is_what_there_is() {
        assert_eq!(ReviewedHead::of(None, Some("abc123")), named("abc123"));
        let unknown = Diff {
            revision: None,
            files: vec![],
        };
        assert_eq!(
            ReviewedHead::of(Some(&unknown), Some("abc123")),
            named("abc123"),
            "a diff that does not say what it is of says nothing"
        );
    }

    #[test]
    fn the_diff_of_one_commit_is_not_the_head_that_was_read() {
        let one_commit = diff_at("def456", true);
        assert_eq!(
            ReviewedHead::of(Some(&one_commit), Some("abc123")),
            named("abc123")
        );
    }

    #[test]
    fn no_head_known_is_no_head() {
        assert_eq!(ReviewedHead::of(None, None), None);
        assert_eq!(ReviewedHead::of(None, Some("")), None);
        assert_eq!(ReviewedHead::of(Some(&diff_at("", false)), None), None);
    }
}

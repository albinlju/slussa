use super::{
    authorship::Authorship,
    user::{AccountKind, User},
};
use chrono::{DateTime, Utc};

/// A comment's id at its provider. A type of its own, so that it cannot be
/// passed where a PR's number is expected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct CommentId(pub u64);

impl std::fmt::Display for CommentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub struct Comment {
    /// The comment's own id (GitHub `databaseId`, Bitbucket `id`), used to edit
    /// or delete it. `None` when the provider didn't supply one.
    pub id: Option<CommentId>,
    pub author: User,
    /// Whether the account is a bot's; what the provider could not tell is a person's.
    pub account: AccountKind,
    /// Whether an AI agent wrote it. A provider leaves this `Human`; the store
    /// judges it when the activity arrives (`AiMarkers::judge`), once, from the
    /// account and the session's markers.
    pub authorship: Authorship,
    pub content: String,
    pub created: DateTime<Utc>,
    pub reactions: Vec<Reaction>,
    /// Id to hang a reply under, when the provider threads this comment (None = no threading).
    pub reply_to: Option<CommentId>,
}

impl Comment {
    pub fn is_ai(&self) -> bool {
        self.authorship == Authorship::Ai
    }
}

/// Text with something in it besides whitespace: what a comment has to be
/// before it is sent or queued. `new` is the only way to make one, so nothing
/// that takes it checks again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonBlank(String);

impl NonBlank {
    pub fn new(text: String) -> Option<Self> {
        (!text.trim().is_empty()).then_some(Self(text))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

#[cfg(test)]
impl From<&str> for NonBlank {
    fn from(text: &str) -> Self {
        Self::new(text.to_owned()).expect("a test comment is not blank")
    }
}

/// Which of the two kinds of comment a provider keeps. GitHub edits and
/// deletes them through different endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommentKind {
    /// A comment on the PR as a whole.
    Conversation,
    /// A comment in a review thread on the code.
    Review,
}

/// A comment that can be edited or deleted: the provider gave it an id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct CommentKey {
    pub id: CommentId,
    /// Saved drafts spell this `review: bool`; the file is older than the enum.
    #[serde(rename = "review", with = "review_flag")]
    pub kind: CommentKind,
}

mod review_flag {
    use super::CommentKind;
    use serde::{Deserialize, Deserializer, Serializer};

    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "the signature is serde's"
    )]
    pub fn serialize<S: Serializer>(kind: &CommentKind, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bool(match kind {
            CommentKind::Review => true,
            CommentKind::Conversation => false,
        })
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(source: D) -> Result<CommentKind, D::Error> {
        Ok(if bool::deserialize(source)? {
            CommentKind::Review
        } else {
            CommentKind::Conversation
        })
    }
}

#[derive(Debug, Clone)]
pub struct Reaction {
    pub emoji: String,
    pub count: u32,
    pub mine: bool,
}

pub fn split_suggestions(body: &str) -> (String, Vec<String>) {
    let mut prose: Vec<&str> = Vec::new();
    let mut suggestions: Vec<String> = Vec::new();
    let mut lines = body.lines();
    while let Some(line) = lines.next() {
        if line.trim_start().starts_with("```suggestion") {
            let mut block: Vec<&str> = Vec::new();
            for inner in lines.by_ref() {
                if inner.trim_start().starts_with("```") {
                    break;
                }
                block.push(inner);
            }
            suggestions.push(block.join("\n"));
        } else {
            prose.push(line);
        }
    }
    (prose.join("\n"), suggestions)
}

#[derive(Debug, Clone)]
pub struct CommentThread {
    pub comments: Vec<Comment>,
    /// Id of the comment a reply should hang under (None = can't reply, e.g. no id parsed).
    pub reply_to: Option<CommentId>,
    /// Code-review context: where the thread is anchored and its resolution.
    /// `None` for a general discussion thread (no code location, not resolvable).
    pub anchor: Option<ThreadAnchor>,
}

impl CommentThread {
    /// A thread is the agent's when the comment that started it is.
    pub fn authorship(&self) -> Authorship {
        self.comments
            .first()
            .map_or(Authorship::Human, |first| first.authorship)
    }
}

/// The review-thread specifics — present only when a thread is anchored to code.
#[derive(Debug, Clone)]
pub struct ThreadAnchor {
    pub revision: Option<String>,
    pub path: String,
    /// The line it is on; none for a thread on the file as a whole.
    pub line: Option<super::diff::LineRef>,
    pub resolved: bool,
    /// What resolving or reopening the thread takes; none when the provider
    /// gave nothing to address it by.
    pub handle: Option<ThreadHandle>,
}

/// How a provider addresses a thread to resolve or reopen it. The provider
/// that read the thread makes the handle, so there is exactly one way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreadHandle {
    /// GitHub: the GraphQL node id of the review thread.
    NodeId(String),
    /// Bitbucket: the thread's root comment, whose state is toggled.
    RootComment(CommentId),
}

impl CommentThread {
    pub fn matches_revision(&self, revision: Option<&super::diff::DiffRevision>) -> bool {
        self.anchor
            .as_ref()
            .is_some_and(|anchor| anchor.revision.as_deref() == revision.map(|r| r.head.as_str()))
    }

    /// Anchored threads hold review comments; a general discussion's are
    /// comments on the PR.
    pub const fn kind(&self) -> CommentKind {
        match self.anchor {
            Some(_) => CommentKind::Review,
            None => CommentKind::Conversation,
        }
    }

    /// A code-review thread that's been resolved. General discussion is never
    /// resolved.
    pub fn resolved(&self) -> bool {
        self.anchor.as_ref().is_some_and(|a| a.resolved)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_comment_needs_more_than_whitespace_and_is_kept_as_written() {
        use super::NonBlank;
        assert_eq!(NonBlank::new(String::new()), None);
        assert_eq!(NonBlank::new(" \n\t".into()), None);
        let text = NonBlank::new("  indented\n".into()).unwrap();
        assert_eq!(text.as_str(), "  indented\n");
        assert_eq!(text.into_string(), "  indented\n");
    }

    use super::*;

    #[test]
    fn splits_suggestion_from_prose() {
        let body = "Name it `line_spans`:\n```suggestion\nlet line_spans = x;\ndebug_assert!(!line_spans.is_empty());\n```\nthanks";
        let (prose, suggestions) = split_suggestions(body);
        assert_eq!(prose, "Name it `line_spans`:\nthanks");
        assert_eq!(
            suggestions,
            vec!["let line_spans = x;\ndebug_assert!(!line_spans.is_empty());"]
        );
    }

    #[test]
    fn body_without_suggestion_passes_through() {
        let (prose, suggestions) = split_suggestions("just a comment\nwith two lines");
        assert_eq!(prose, "just a comment\nwith two lines");
        assert!(suggestions.is_empty());
    }

    #[test]
    fn empty_suggestion_means_delete_the_line() {
        let (_, suggestions) = split_suggestions("remove this\n```suggestion\n```");
        assert_eq!(suggestions, vec![""]);
    }

    #[test]
    fn several_suggestions_keep_their_order_and_the_prose_between() {
        let body = "first\n```suggestion\na\n```\nmiddle\n```suggestion\nb\nc\n```\nlast";
        let (prose, suggestions) = split_suggestions(body);
        assert_eq!(prose, "first\nmiddle\nlast");
        assert_eq!(suggestions, vec!["a", "b\nc"]);
    }

    #[test]
    fn unterminated_suggestion_takes_the_rest_of_the_body() {
        let (prose, suggestions) = split_suggestions("intro\n```suggestion\nfoo\nbar");
        assert_eq!(prose, "intro");
        assert_eq!(suggestions, vec!["foo\nbar"]);
    }

    #[test]
    fn ordinary_code_fences_stay_in_the_prose() {
        let body = "see:\n```rust\nlet x = 1;\n```";
        let (prose, suggestions) = split_suggestions(body);
        assert_eq!(prose, body);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn indented_suggestion_fence_is_recognised() {
        let (prose, suggestions) = split_suggestions("  ```suggestion\nx\n  ```");
        assert_eq!(prose, "");
        assert_eq!(suggestions, vec!["x"]);
    }

    fn thread(anchor: Option<ThreadAnchor>) -> CommentThread {
        CommentThread {
            comments: Vec::new(),
            reply_to: None,
            anchor,
        }
    }

    fn anchor(revision: Option<&str>, resolved: bool) -> ThreadAnchor {
        ThreadAnchor {
            revision: revision.map(str::to_owned),
            path: "src/lib.rs".into(),
            line: Some(crate::domain::diff::LineRef::New(3)),
            resolved,
            handle: None,
        }
    }

    fn revision(head: &str) -> crate::domain::diff::DiffRevision {
        crate::domain::diff::DiffRevision {
            head: head.into(),
            base: None,
            commit: false,
        }
    }

    #[test]
    fn thread_matches_only_the_revision_it_was_anchored_to() {
        let anchored = thread(Some(anchor(Some("abc"), false)));
        assert!(anchored.matches_revision(Some(&revision("abc"))));
        assert!(!anchored.matches_revision(Some(&revision("def"))));
        assert!(!anchored.matches_revision(None));
        assert!(!thread(None).matches_revision(Some(&revision("abc"))));
    }

    #[test]
    fn thread_without_a_revision_matches_only_a_missing_one() {
        let unversioned = thread(Some(anchor(None, false)));
        assert!(unversioned.matches_revision(None));
        assert!(!unversioned.matches_revision(Some(&revision("abc"))));
    }

    #[test]
    fn only_anchored_threads_can_be_resolved() {
        assert!(thread(Some(anchor(None, true))).resolved());
        assert!(!thread(Some(anchor(None, false))).resolved());
        assert!(!thread(None).resolved());
    }
}

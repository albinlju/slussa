use super::event::TimelineEvent;
use super::{
    authorship::Authorship,
    comment::{Comment, CommentThread},
};

#[derive(Debug, Default, Clone)]
pub struct Activity {
    pub comments: Vec<Comment>,
    pub events: Vec<TimelineEvent>,
    pub threads: Vec<CommentThread>,
}

impl Activity {
    /// Whether any comment in it is an agent's.
    pub fn has_ai(&self) -> bool {
        self.comments.iter().any(Comment::is_ai)
            || self
                .threads
                .iter()
                .any(|t| t.authorship() == Authorship::Ai)
    }
}

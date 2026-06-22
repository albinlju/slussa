use super::comment::{Comment, CommentThread};
use super::event::TimelineEvent;

#[derive(Debug, Default, Clone)]
pub struct Activity {
    pub comments: Vec<Comment>,
    pub events: Vec<TimelineEvent>,
    pub threads: Vec<CommentThread>,
}

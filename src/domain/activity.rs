use super::comment::{Comment, ReviewThread};
use super::event::TimelineEvent;

#[derive(Debug, Default, Clone)]
pub struct Activity {
    pub comments: Vec<Comment>,
    pub events: Vec<TimelineEvent>,
    pub threads: Vec<ReviewThread>,
}

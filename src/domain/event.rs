use super::user::User;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventKind {
    Opened,
    Approved,
    ChangesRequested,
    ReviewRemoved,
    Merged,
    Declined,
    Reopened,
    Pushed(Vec<PushedCommit>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushedCommit {
    pub id: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct TimelineEvent {
    pub actor: Option<User>,
    pub kind: EventKind,
    pub created: DateTime<Utc>,
}

use super::user::User;
use chrono::{DateTime, Utc};

/// A non-comment lifecycle event in a PR's history — approvals, merges, state
/// changes. Comments and inline review threads are modelled separately
/// ([`super::comment`]); these are the "X did Y" rows that thread through the
/// Overview timeline alongside them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventKind {
    Opened,
    Approved,
    ChangesRequested,
    /// Approval or review withdrawn (Bitbucket UNAPPROVED, GitHub dismissed).
    ReviewRemoved,
    Merged,
    Declined,
    Reopened,
    /// New commits pushed to the PR (Bitbucket RESCOPED), with the added
    /// commits' short id + first message line for display.
    Pushed(Vec<PushedCommit>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushedCommit {
    pub id: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct TimelineEvent {
    /// Who did it. `None` when the provider doesn't attribute the event
    /// (e.g. a merge with no recorded actor).
    pub actor: Option<User>,
    pub kind: EventKind,
    pub created: DateTime<Utc>,
}

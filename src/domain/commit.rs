use super::{authorship::Authorship, user::AccountKind};
use chrono::{DateTime, Utc};

/// A commit's full object id. A type of its own, so that it cannot be passed
/// where another string (a path, a cursor, a branch) is expected.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommitOid(pub String);

impl CommitOid {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The seven characters a commit is known by.
    pub fn short(&self) -> String {
        self.0.chars().take(7).collect()
    }
}

#[cfg(test)]
impl From<&str> for CommitOid {
    fn from(oid: &str) -> Self {
        Self(oid.to_owned())
    }
}

#[derive(Debug, Clone)]
pub struct Commit {
    pub oid: CommitOid,
    pub headline: String,
    /// The whole message, trailers included: an agent that commits under its
    /// owner's account says so there.
    pub message: String,
    pub author_name: String,
    /// Whose account made it, where the provider tells a bot's from a person's.
    pub account: AccountKind,
    /// Whether an agent made it. `Human` until the store has judged it.
    pub authorship: Authorship,
    pub authored_at: DateTime<Utc>,
    pub additions: u32,
    pub deletions: u32,
}

impl Commit {
    pub fn is_ai(&self) -> bool {
        self.authorship == Authorship::Ai
    }
}

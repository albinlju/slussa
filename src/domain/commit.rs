use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Commit {
    pub oid: String,
    /// First line of the commit message.
    pub headline: String,
    pub author_name: String,
    pub authored_at: DateTime<Utc>,
    pub additions: u32,
    pub deletions: u32,
}

use super::user::User;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: u64,
    pub author: User,
    pub content: String,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub replies: Vec<Comment>,
    pub resolved: bool,
}

/// A review thread is a discussion anchored to a specific line in the diff.
/// Each thread has a starting comment (with file path + line + diff context)
/// and zero or more reply comments in chronological order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewThread {
    pub path: String,
    /// New-file (destination) line number this thread is anchored to. Set for
    /// threads on added or context lines.
    pub line: Option<usize>,
    /// Old-file (source) line number, set instead of `line` for threads
    /// anchored to a removed line. The diff pane keys these off the old-side
    /// line counter so deleted-line comments still render in place.
    pub old_line: Option<usize>,
    pub diff_hunk: String,
    pub comments: Vec<Comment>,
    pub resolved: bool,
}

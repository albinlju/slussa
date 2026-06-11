use super::user::User;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Comment {
    pub author: User,
    pub content: String,
    pub created: DateTime<Utc>,
    /// Emoji reactions on the comment (read-only), already aggregated to a
    /// count per emoji. Empty when none / the provider doesn't surface them.
    pub reactions: Vec<Reaction>,
}

#[derive(Debug, Clone)]
pub struct Reaction {
    pub emoji: String,
    pub count: u32,
    /// Whether the logged-in user is among the reactors — their own
    /// reactions render with a border.
    pub mine: bool,
}

/// A review thread is a discussion anchored to a specific line in the diff.
/// Each thread has a starting comment (with file path + line + diff context)
/// and zero or more reply comments in chronological order.
#[derive(Debug, Clone)]
pub struct ReviewThread {
    pub path: String,
    /// New-file (destination) line number this thread is anchored to. Set for
    /// threads on added or context lines.
    pub line: Option<usize>,
    /// Old-file (source) line number, set instead of `line` for threads
    /// anchored to a removed line. The diff pane keys these off the old-side
    /// line counter so deleted-line comments still render in place.
    pub old_line: Option<usize>,
    pub comments: Vec<Comment>,
    pub resolved: bool,
}

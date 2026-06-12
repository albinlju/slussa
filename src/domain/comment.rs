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
    /// reactions render with an accent-tinted background.
    pub mine: bool,
}

/// Split a comment body into its prose and any ```suggestion``` fenced
/// blocks — GitHub's (and GitLab's) syntax for proposing replacement lines.
/// Each returned block is the suggested content, newline-joined.
pub fn split_suggestions(body: &str) -> (String, Vec<String>) {
    let mut prose: Vec<&str> = Vec::new();
    let mut suggestions: Vec<String> = Vec::new();
    let mut lines = body.lines();
    while let Some(line) = lines.next() {
        // `starts_with` also covers GitLab's range syntax (```suggestion:-0+0).
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

#[cfg(test)]
mod tests {
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
}

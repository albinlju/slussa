use super::user::User;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Comment {
    pub author: User,
    pub content: String,
    pub created: DateTime<Utc>,
    pub reactions: Vec<Reaction>,
    /// Id to hang a reply under, when the provider threads this comment (None = no threading).
    pub reply_to: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct Reaction {
    pub emoji: String,
    pub count: u32,
    pub mine: bool,
}

pub fn split_suggestions(body: &str) -> (String, Vec<String>) {
    let mut prose: Vec<&str> = Vec::new();
    let mut suggestions: Vec<String> = Vec::new();
    let mut lines = body.lines();
    while let Some(line) = lines.next() {
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

#[derive(Debug, Clone)]
pub struct ReviewThread {
    pub path: String,
    pub line: Option<usize>,
    pub old_line: Option<usize>,
    pub comments: Vec<Comment>,
    pub resolved: bool,
    /// Id of the comment a reply should hang under (None = can't reply, e.g. no id parsed).
    pub reply_to: Option<u64>,
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

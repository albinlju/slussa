//! The line that says who wrote a comment: the name, what they are to the PR
//! (its author, a bot) and when.

use crate::{
    domain::comment::{Comment, split_suggestions},
    tui::{format, theme, widgets::comment_fold::Folds},
};
use chrono::{DateTime, Utc};
use ratatui::{
    style::{Modifier, Style},
    text::Span,
};

/// What a comment is read against: the PR's author, and which long comments are
/// folded.
#[derive(Clone, Copy)]
pub struct Reading<'a> {
    pub pr_author: &'a str,
    /// Which long comments are folded; `Open` where nothing can open them.
    pub folds: Folds<'a>,
}

/// `[AI]` after a name, in the label colour so that it stands out from the
/// muted notes beside it.
pub fn ai_tag() -> Span<'static> {
    Span::styled(
        " [AI]",
        Style::default()
            .fg(theme::current().decorative)
            .add_modifier(Modifier::BOLD),
    )
}

/// Name, then `[AI]` for an agent, then the role, then how long ago.
pub fn meta(comment: &Comment, reading: Reading<'_>, now: DateTime<Utc>) -> Vec<Span<'static>> {
    let theme = theme::current();
    let mut spans = vec![Span::styled(
        comment.author.username.clone(),
        Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
    )];
    if comment.is_ai() {
        spans.push(ai_tag());
    }
    if let Some(role) = role(comment, reading.pr_author) {
        spans.push(Span::styled(role, Style::default().fg(theme.muted)));
    }
    spans.push(Span::styled(
        format!(" · {}", format::relative_age(comment.created, now)),
        Style::default().fg(theme.muted),
    ));
    spans
}

fn role(comment: &Comment, pr_author: &str) -> Option<&'static str> {
    if comment.author.username == pr_author {
        Some(" · author")
    } else if !split_suggestions(&comment.content).1.is_empty() {
        Some(" · suggested a change")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        authorship::Authorship,
        user::{AccountKind, User},
    };

    fn comment(name: &str, authorship: Authorship) -> Comment {
        Comment {
            id: None,
            author: User {
                username: name.into(),
            },
            account: AccountKind::Person,
            authorship,
            content: "text".into(),
            created: Utc::now(),
            reactions: vec![],
            reply_to: None,
        }
    }

    fn text(spans: &[Span<'_>]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn an_agent_gets_the_tag_between_its_name_and_the_age() {
        let reading = Reading {
            pr_author: "alice",
            folds: Folds::Open,
        };
        let line = |c: &Comment| text(&meta(c, reading, Utc::now()));
        assert!(line(&comment("coderabbitai", Authorship::Ai)).starts_with("coderabbitai [AI] · "));
        let person = line(&comment("bob", Authorship::Human));
        assert!(!person.contains("[AI]"), "{person}");
        // Both at once: an agent that is also the PR's author.
        assert!(line(&comment("alice", Authorship::Ai)).starts_with("alice [AI] · author · "));
    }
}

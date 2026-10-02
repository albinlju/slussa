//! The line that says who wrote a comment: the name, what they are to the PR
//! (its author, a bot) and when.

use crate::{
    domain::comment::{Comment, Reaction, split_suggestions},
    tui::ui::{format, theme, widgets::comment::fold::Folds},
};
use chrono::{DateTime, Utc};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
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

pub(in crate::tui::ui) fn author_line(
    mut lead: Vec<Span<'static>>,
    created: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Line<'static> {
    lead.push(Span::styled(
        format!(" · {}", format::relative_age(created, now)),
        Style::default().fg(theme::current().muted),
    ));
    Line::from(lead)
}

pub(in crate::tui::ui) fn reactions_line(reactions: &[Reaction]) -> Option<Line<'static>> {
    if reactions.is_empty() {
        return None;
    }
    let theme = theme::current();
    let mut spans: Vec<Span<'static>> = Vec::new();
    for r in reactions {
        if !spans.is_empty() {
            spans.push(Span::raw(" "));
        }
        let fg = if r.mine {
            theme.reaction_mine
        } else {
            theme.fg
        };
        spans.push(Span::styled(
            format!(" {} {} ", r.emoji, r.count),
            Style::default().fg(fg).bg(theme.highlight_bg),
        ));
    }
    Some(Line::from(spans))
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

    fn reaction(emoji: &str, count: u32, mine: bool) -> Reaction {
        Reaction {
            emoji: emoji.to_string(),
            count,
            mine,
        }
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn renders_one_padded_pill_per_reaction() {
        let line = reactions_line(&[reaction("👍", 2, false), reaction("👀", 3, false)]).unwrap();
        assert_eq!(line_text(&line), " 👍 2   👀 3 ");
    }

    #[test]
    fn own_reaction_gets_the_accent_text() {
        let theme = theme::current();
        let line = reactions_line(&[reaction("👍", 4, true), reaction("👀", 3, false)]).unwrap();
        assert_eq!(line.spans[0].style.bg, Some(theme.highlight_bg));
        assert_eq!(line.spans[0].style.fg, Some(theme.reaction_mine));
        assert_eq!(line.spans[2].style.fg, Some(theme.fg));
    }
}

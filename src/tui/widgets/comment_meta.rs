//! The line that says who wrote a comment: the name, what they are to the PR
//! (its author, a bot) and when.

use crate::{
    domain::{
        authorship::{AiMarkers, Authorship},
        comment::{Comment, split_suggestions},
    },
    tui::{format, theme, widgets::comment_fold::Folds},
};
use chrono::{DateTime, Utc};
use ratatui::{
    style::{Modifier, Style},
    text::Span,
};

/// What a comment is read against: the PR's author, what makes an account or a
/// first line an AI agent's, and which long comments are folded.
#[derive(Clone, Copy)]
pub struct Roles<'a> {
    pub pr_author: &'a str,
    pub markers: &'a AiMarkers,
    /// Which long comments are folded; `Open` where nothing can open them.
    pub folds: Folds<'a>,
}

impl Roles<'_> {
    pub fn is_ai(&self, comment: &Comment) -> bool {
        self.markers.of_comment(comment) == Authorship::Ai
    }
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
pub fn meta(
    comment: &Comment,
    roles: Roles<'_>,
    created: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Vec<Span<'static>> {
    let theme = theme::current();
    let mut spans = vec![Span::styled(
        comment.author.username.clone(),
        Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
    )];
    if roles.is_ai(comment) {
        spans.push(ai_tag());
    }
    if let Some(role) = role(comment, roles.pr_author) {
        spans.push(Span::styled(role, Style::default().fg(theme.muted)));
    }
    spans.push(Span::styled(
        format!(" · {}", format::relative_age(created, now)),
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
    use crate::domain::user::{AccountKind, User};

    fn comment(name: &str, content: &str, account: AccountKind) -> Comment {
        Comment {
            id: None,
            author: User {
                username: name.into(),
            },
            account,
            content: content.into(),
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
        let markers = AiMarkers::from_config(&["> **gator**".into()]).0;
        let roles = Roles {
            pr_author: "alice",
            markers: &markers,
            folds: Folds::Open,
        };
        let now = Utc::now();
        let line = |c: &Comment| text(&meta(c, roles, c.created, now));
        assert!(
            line(&comment("coderabbitai", "hi", AccountKind::Bot))
                .starts_with("coderabbitai [AI] · ")
        );
        assert!(
            line(&comment("bob", "> **gator**\nx", AccountKind::Person)).starts_with("bob [AI] · ")
        );
        let person = line(&comment("bob", "hi", AccountKind::Person));
        assert!(!person.contains("[AI]"), "{person}");
        // Both roles at once: an agent that is also the PR's author.
        assert!(
            line(&comment("alice", "> **gator**", AccountKind::Person))
                .starts_with("alice [AI] · author · ")
        );
    }
}

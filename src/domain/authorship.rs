//! Whether a comment was written by an AI agent. Two things say so: the
//! account, when the provider marks it as a bot's (GitHub does), and a fixed
//! first line such as `> **gator-agent**`, for an agent that posts with its
//! owner's token. The second is configured; the first needs nothing.

use super::{
    activity::Activity,
    comment::{Comment, CommentThread},
    user::AccountKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Authorship {
    Human,
    Ai,
}

/// Which comments the Overview shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AuthorFilter {
    #[default]
    All,
    Humans,
    Ai,
}

impl AuthorFilter {
    pub const fn next(self) -> Self {
        match self {
            Self::All => Self::Humans,
            Self::Humans => Self::Ai,
            Self::Ai => Self::All,
        }
    }

    /// Whose comments this filter leaves out; none for `All`.
    pub const fn hides(self) -> Option<Authorship> {
        match self {
            Self::All => None,
            Self::Humans => Some(Authorship::Ai),
            Self::Ai => Some(Authorship::Human),
        }
    }

    pub const fn shows(self, authorship: Authorship) -> bool {
        match (self, authorship) {
            (Self::All, _) | (Self::Humans, Authorship::Human) | (Self::Ai, Authorship::Ai) => true,
            (Self::Humans, Authorship::Ai) | (Self::Ai, Authorship::Human) => false,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Humans => "humans",
            Self::Ai => "AI",
        }
    }
}

/// The first line a configured agent starts its comments with. Never blank, so
/// it cannot match every comment. `parse` is the only way to make one.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AiMarker(String);

impl AiMarker {
    fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        (!text.is_empty()).then(|| Self(text.to_lowercase()))
    }
}

/// The first lines of the session's agents, from the config. Empty when none is
/// set: bots' accounts are still recognised.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AiMarkers(Vec<AiMarker>);

impl AiMarkers {
    /// Blank entries are dropped; the second value says how many were.
    pub fn from_config(configured: &[String]) -> (Self, usize) {
        let markers: Vec<_> = configured
            .iter()
            .filter_map(|m| AiMarker::parse(m))
            .collect();
        let dropped = configured.len() - markers.len();
        (Self(markers), dropped)
    }

    /// A comment is the agent's when its account is a bot's, or its first line
    /// that has text begins with one of the markers, whatever the case.
    pub fn of_comment(&self, comment: &Comment) -> Authorship {
        if comment.account == AccountKind::Bot {
            return Authorship::Ai;
        }
        let first = comment
            .content
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(str::to_lowercase);
        match first {
            Some(line) if self.0.iter().any(|m| line.starts_with(&m.0)) => Authorship::Ai,
            Some(_) | None => Authorship::Human,
        }
    }

    /// Whether anything in the activity is an agent's.
    pub fn any_in(&self, activity: &Activity) -> bool {
        activity
            .comments
            .iter()
            .any(|c| self.of_comment(c) == Authorship::Ai)
            || activity
                .threads
                .iter()
                .any(|t| self.of_thread(t) == Authorship::Ai)
    }

    /// A thread is the agent's when the comment that started it is.
    pub fn of_thread(&self, thread: &CommentThread) -> Authorship {
        thread
            .comments
            .first()
            .map_or(Authorship::Human, |first| self.of_comment(first))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::user::User;

    fn comment(content: &str) -> Comment {
        Comment {
            id: None,
            author: User {
                username: "alice".into(),
            },
            account: AccountKind::Person,
            content: content.into(),
            created: chrono::Utc::now(),
            reactions: vec![],
            reply_to: None,
        }
    }

    fn markers(configured: &[&str]) -> AiMarkers {
        let configured: Vec<String> = configured.iter().map(|m| (*m).to_owned()).collect();
        AiMarkers::from_config(&configured).0
    }

    #[test]
    fn a_comment_is_the_agents_when_its_first_line_starts_with_a_marker() {
        let markers = markers(&["> **gator-agent**", "> **🏗️ build-agent**"]);
        let ai = |text: &str| markers.of_comment(&comment(text));
        assert_eq!(
            ai("> **gator-agent**\n\nThe lock is never released."),
            Authorship::Ai
        );
        assert_eq!(ai("> **🏗️ build-agent** (round 2)\nbody"), Authorship::Ai);
        // Case and leading blank lines do not matter.
        assert_eq!(ai("\n  \n> **Gator-Agent**\nbody"), Authorship::Ai);
        // Only the first line counts: a quote further down is a person quoting.
        assert_eq!(
            ai("Looks right.\n> **gator-agent**\nbody"),
            Authorship::Human
        );
        assert_eq!(ai("> **someone-else**\nbody"), Authorship::Human);
        assert_eq!(ai(""), Authorship::Human);
    }

    #[test]
    fn a_bots_account_marks_a_comment_without_any_marker() {
        let none = markers(&[]);
        let mut by_bot = comment("Looks fine.");
        by_bot.account = AccountKind::Bot;
        assert_eq!(none.of_comment(&by_bot), Authorship::Ai);
        let activity = |comments| Activity {
            comments,
            ..Activity::default()
        };
        assert!(none.any_in(&activity(vec![comment("hi"), by_bot])));
        assert!(!none.any_in(&activity(vec![comment("hi")])));
    }

    #[test]
    fn the_filter_cycles_and_shows_what_it_names() {
        let mut seen = vec![AuthorFilter::default()];
        for _ in 0..3 {
            seen.push(seen[seen.len() - 1].next());
        }
        assert_eq!(
            seen,
            [
                AuthorFilter::All,
                AuthorFilter::Humans,
                AuthorFilter::Ai,
                AuthorFilter::All
            ]
        );
        assert_eq!(AuthorFilter::All.hides(), None);
        assert_eq!(AuthorFilter::Humans.hides(), Some(Authorship::Ai));
        assert_eq!(AuthorFilter::Ai.hides(), Some(Authorship::Human));
        assert!(AuthorFilter::All.shows(Authorship::Ai));
        assert!(AuthorFilter::All.shows(Authorship::Human));
        assert!(AuthorFilter::Humans.shows(Authorship::Human));
        assert!(!AuthorFilter::Humans.shows(Authorship::Ai));
        assert!(AuthorFilter::Ai.shows(Authorship::Ai));
        assert!(!AuthorFilter::Ai.shows(Authorship::Human));
    }

    #[test]
    fn without_markers_nothing_is_marked_and_a_blank_one_matches_nothing() {
        let none = markers(&[]);
        assert_eq!(none, AiMarkers::default());
        assert_eq!(
            none.of_comment(&comment("> **gator-agent**")),
            Authorship::Human
        );

        let (blank, dropped) = AiMarkers::from_config(&[String::new(), "  \n".into()]);
        assert_eq!(blank, AiMarkers::default());
        assert_eq!(dropped, 2);
        assert_eq!(blank.of_comment(&comment("anything")), Authorship::Human);
    }

    #[test]
    fn a_thread_takes_the_authorship_of_its_first_comment() {
        let thread_of = |comments| CommentThread {
            comments,
            reply_to: None,
            anchor: None,
        };
        let markers = markers(&["> **bot**"]);
        let mut thread = thread_of(vec![comment("> **bot**\nfinding")]);
        assert_eq!(markers.of_thread(&thread), Authorship::Ai);
        // A human replying does not make it theirs.
        thread.comments.push(comment("fixed"));
        assert_eq!(markers.of_thread(&thread), Authorship::Ai);
        thread.comments.reverse();
        assert_eq!(markers.of_thread(&thread), Authorship::Human);
        assert_eq!(markers.of_thread(&thread_of(vec![])), Authorship::Human);
    }
}

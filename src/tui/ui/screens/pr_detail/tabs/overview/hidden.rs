//! The dimmed line that stands where the filter hides comments, so that a
//! filtered timeline never reads as an empty one.

use crate::{
    domain::{authorship::Authorship, comment::CommentThread},
    tui::ui::{theme, widgets},
};
use ratatui::{
    style::Style,
    text::{Line, Span},
};

/// What the filter hid between two things that are shown, all of it by one
/// side.
#[derive(Debug)]
pub(super) struct HiddenRun {
    side: Authorship,
    threads: usize,
    /// Comments on the PR as a whole.
    conversation: usize,
    /// Every comment inside the hidden threads.
    in_threads: usize,
    /// Who answered an agent's hidden thread, so that a person's reply is not
    /// lost with it. Only kept when it is the agent's comments that are hidden.
    replied: Vec<String>,
}

impl HiddenRun {
    pub(super) const fn of(side: Authorship) -> Self {
        Self {
            side,
            threads: 0,
            conversation: 0,
            in_threads: 0,
            replied: Vec::new(),
        }
    }

    pub(super) const fn add_conversation_comment(&mut self) {
        self.conversation += 1;
    }

    pub(super) fn add_thread(&mut self, thread: &CommentThread) {
        if thread.comments.is_empty() {
            return;
        }
        self.threads += 1;
        self.in_threads += thread.comments.len();
        if self.side != Authorship::Ai {
            return;
        }
        for reply in thread.comments.iter().skip(1) {
            if reply.authorship == Authorship::Human
                && !self.replied.contains(&reply.author.username)
            {
                self.replied.push(reply.author.username.clone());
            }
        }
    }

    /// The line, or none when nothing was hidden here.
    pub(super) fn line(&self, width: u16) -> Option<Line<'static>> {
        let items = self.threads + self.conversation;
        if items == 0 {
            return None;
        }
        let who = match self.side {
            Authorship::Ai => "AI",
            Authorship::Human => "human",
        };
        let noun = match (self.threads > 0, self.conversation > 0) {
            (true, false) => noun_for(items, "thread"),
            (false, _) => noun_for(items, "comment"),
            (true, true) => noun_for(items, "item"),
        };
        let mut parts = vec![format!("◆ {items} {who} {noun} hidden")];
        if self.threads > 0 {
            parts.push(plural(self.in_threads + self.conversation, "comment"));
        }
        match self.replied.as_slice() {
            [] => {}
            [one] => parts.push(format!("incl. a reply from {one}")),
            several => parts.push(format!("incl. replies from {} people", several.len())),
        }
        parts.push("f to cycle".to_owned());
        let text = parts.join(" · ");
        let style = Style::default().fg(theme::current().muted);
        Some(Line::from(widgets::truncate_to_width(
            vec![Span::styled(text, style)],
            usize::from(width),
        )))
    }
}

fn noun_for(count: usize, noun: &str) -> String {
    if count == 1 {
        noun.to_owned()
    } else {
        format!("{noun}s")
    }
}

fn plural(count: usize, noun: &str) -> String {
    format!("{count} {}", noun_for(count, noun))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        comment::Comment,
        user::{AccountKind, User},
    };

    fn comment(name: &str, account: AccountKind) -> Comment {
        Comment {
            id: None,
            author: User {
                username: name.into(),
            },
            account,
            authorship: match account {
                AccountKind::Bot => Authorship::Ai,
                AccountKind::Person => Authorship::Human,
            },
            content: "text".into(),
            created: chrono::Utc::now(),
            reactions: vec![],
            reply_to: None,
        }
    }

    fn thread(comments: Vec<Comment>) -> CommentThread {
        CommentThread {
            comments,
            reply_to: None,
            anchor: None,
        }
    }

    fn text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn it_counts_threads_and_comments_and_names_a_persons_reply() {
        let mut run = HiddenRun::of(Authorship::Ai);
        assert!(run.line(100).is_none());
        run.add_thread(&thread(vec![comment("bot", AccountKind::Bot)]));
        assert_eq!(
            text(&run.line(100).unwrap()),
            "◆ 1 AI thread hidden · 1 comment · f to cycle"
        );
        run.add_thread(&thread(vec![
            comment("bot", AccountKind::Bot),
            comment("sara.n", AccountKind::Person),
            comment("sara.n", AccountKind::Person),
            comment("bot", AccountKind::Bot),
        ]));
        assert_eq!(
            text(&run.line(100).unwrap()),
            "◆ 2 AI threads hidden · 5 comments · incl. a reply from sara.n · f to cycle"
        );
    }

    #[test]
    fn it_says_comments_for_the_pr_conversation_and_what_the_other_filter_hides() {
        let mut run = HiddenRun::of(Authorship::Human);
        run.add_conversation_comment();
        run.add_conversation_comment();
        assert_eq!(
            text(&run.line(100).unwrap()),
            "◆ 2 human comments hidden · f to cycle"
        );
    }

    #[test]
    fn a_persons_reply_is_named_only_when_it_is_the_agents_threads_that_are_hidden() {
        let by_people = thread(vec![
            comment("alice", AccountKind::Person),
            comment("sara.n", AccountKind::Person),
        ]);
        // The filter hides people's threads: every comment in them is a person's,
        // and naming a reply there would say nothing.
        let mut people = HiddenRun::of(Authorship::Human);
        people.add_thread(&by_people);
        let line = text(&people.line(100).unwrap());
        assert_eq!(line, "◆ 1 human thread hidden · 2 comments · f to cycle");
        // The filter hides the agent's: a person's reply in one is what is lost.
        let mut agents = HiddenRun::of(Authorship::Ai);
        agents.add_thread(&thread(vec![
            comment("bot", AccountKind::Bot),
            comment("sara.n", AccountKind::Person),
        ]));
        assert!(text(&agents.line(100).unwrap()).contains("incl. a reply from sara.n"));
    }

    #[test]
    fn an_empty_thread_is_not_counted() {
        let mut run = HiddenRun::of(Authorship::Ai);
        run.add_thread(&thread(vec![]));
        assert!(run.line(100).is_none());
    }
}

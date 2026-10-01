//! A long comment shows its first lines and a dimmed line that says how many
//! it hides, until the reader opens it with `space`. A bot's walkthrough runs
//! to dozens of lines and would push the conversation off the screen.

use crate::{domain::comment::CommentKey, tui::theme};
use ratatui::{
    style::Style,
    text::{Line, Span},
};
use std::collections::HashSet;

/// A comment longer than this many lines is folded.
const FOLD_ABOVE: usize = 12;
/// How many of its lines stay visible.
const HEAD: usize = 8;

/// Whether long comments are folded, and which ones the reader has opened. A
/// view with no key to open them is `Open`, so nothing is ever folded out of
/// reach.
#[derive(Clone, Copy)]
pub enum Folds<'a> {
    Open,
    Long { opened: &'a HashSet<CommentKey> },
}

impl Folds<'_> {
    /// `lines` as shown: all of them, or the head and the dimmed line. A comment
    /// without a key cannot be opened, so it is never folded.
    pub fn apply(
        self,
        key: Option<CommentKey>,
        mut lines: Vec<Line<'static>>,
    ) -> Vec<Line<'static>> {
        let (Self::Long { opened }, Some(key)) = (self, key) else {
            return lines;
        };
        if lines.len() <= FOLD_ABOVE || opened.contains(&key) {
            return lines;
        }
        let hidden = lines.len() - HEAD;
        lines.truncate(HEAD);
        lines.push(Line::from(Span::styled(
            format!("… {hidden} more lines · space expand"),
            Style::default().fg(theme::current().muted),
        )));
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::comment::{CommentId, CommentKind};

    fn key(id: u64) -> CommentKey {
        CommentKey {
            id: CommentId(id),
            kind: CommentKind::Review,
        }
    }

    fn lines(count: usize) -> Vec<Line<'static>> {
        (1..=count)
            .map(|n| Line::raw(format!("line {n}")))
            .collect()
    }

    #[test]
    fn a_long_comment_keeps_its_head_and_says_how_much_is_hidden() {
        let opened = HashSet::new();
        let folds = Folds::Long { opened: &opened };
        let shown = folds.apply(Some(key(1)), lines(30));
        assert_eq!(shown.len(), HEAD + 1);
        assert_eq!(shown[HEAD - 1].to_string(), "line 8");
        assert_eq!(shown[HEAD].to_string(), "… 22 more lines · space expand");
    }

    #[test]
    fn short_opened_keyless_and_open_views_are_left_whole() {
        let opened = HashSet::from([key(2)]);
        let folds = Folds::Long { opened: &opened };
        assert_eq!(
            folds.apply(Some(key(1)), lines(FOLD_ABOVE)).len(),
            FOLD_ABOVE
        );
        assert_eq!(folds.apply(Some(key(2)), lines(30)).len(), 30);
        assert_eq!(folds.apply(None, lines(30)).len(), 30);
        assert_eq!(Folds::Open.apply(Some(key(1)), lines(30)).len(), 30);
    }
}

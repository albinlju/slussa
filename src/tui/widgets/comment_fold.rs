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

/// The row of a comment that opens or folds it, where the reader can stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fold {
    /// Its row among the comment's lines.
    pub row: usize,
    pub key: CommentKey,
}

/// A comment's lines as shown, and where its fold row is, if it has one.
pub struct Folded {
    pub lines: Vec<Line<'static>>,
    pub fold: Option<Fold>,
}

impl Folds<'_> {
    /// `lines` as shown: all of them with a row that folds them again, or the
    /// head and a row that opens them. A comment without a key cannot be opened,
    /// so it is never folded.
    pub fn apply(self, key: Option<CommentKey>, mut lines: Vec<Line<'static>>) -> Folded {
        let (Self::Long { opened }, Some(key)) = (self, key) else {
            return Folded { lines, fold: None };
        };
        if lines.len() <= FOLD_ABOVE {
            return Folded { lines, fold: None };
        }
        let muted = Style::default().fg(theme::current().muted);
        let text = if opened.contains(&key) {
            "▲ fold · space".to_owned()
        } else {
            let hidden = lines.len() - HEAD;
            lines.truncate(HEAD);
            format!("… {hidden} more lines · space expand")
        };
        let row = lines.len();
        lines.push(Line::from(Span::styled(text, muted)));
        Folded {
            lines,
            fold: Some(Fold { row, key }),
        }
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
    fn a_long_comment_keeps_its_head_and_a_row_that_opens_it() {
        let opened = HashSet::new();
        let shown = Folds::Long { opened: &opened }.apply(Some(key(1)), lines(30));
        assert_eq!(shown.lines.len(), HEAD + 1);
        assert_eq!(shown.lines[HEAD - 1].to_string(), "line 8");
        assert_eq!(
            shown.lines[HEAD].to_string(),
            "… 22 more lines · space expand"
        );
        assert_eq!(
            shown.fold,
            Some(Fold {
                row: HEAD,
                key: key(1)
            })
        );
    }

    #[test]
    fn an_opened_comment_is_whole_and_ends_in_a_row_that_folds_it() {
        let opened = HashSet::from([key(2)]);
        let shown = Folds::Long { opened: &opened }.apply(Some(key(2)), lines(30));
        assert_eq!(shown.lines.len(), 31);
        assert_eq!(shown.lines[30].to_string(), "▲ fold · space");
        assert_eq!(
            shown.fold,
            Some(Fold {
                row: 30,
                key: key(2)
            })
        );
    }

    #[test]
    fn short_keyless_and_open_views_are_left_whole_without_a_row() {
        let opened = HashSet::new();
        let folds = Folds::Long { opened: &opened };
        for shown in [
            folds.apply(Some(key(1)), lines(FOLD_ABOVE)),
            folds.apply(None, lines(30)),
            Folds::Open.apply(Some(key(1)), lines(30)),
        ] {
            assert!(shown.fold.is_none());
            assert!(shown.lines.len() == FOLD_ABOVE || shown.lines.len() == 30);
        }
    }
}

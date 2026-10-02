//! A thread drawn in the code pane: its lines, and the stops the cursor gets in
//! it.

use super::{
    nav::{NavItem, NavKind, ThreadNav},
    pane::DIFF_GUTTER,
};
use crate::{
    domain::comment::CommentThread,
    tui::ui::widgets::{
        comment::{InlineThread, render_inline_thread},
        comment_meta::Reading,
    },
};
use chrono::{DateTime, Utc};
use ratatui::text::{Line, Span};

/// How to draw a thread.
#[derive(Clone, Copy)]
pub(super) struct ThreadDraw<'a> {
    pub(super) width: u16,
    pub(super) now: DateTime<Utc>,
    /// The item the cursor is on.
    pub(super) active: Option<usize>,
    pub(super) anchor_text: &'a str,
    pub(super) reading: Reading<'a>,
    /// A resolved thread shown in full.
    pub(super) expanded: bool,
}

/// Add the thread's lines and its stops: the thread itself, then one for each
/// long comment's fold row.
pub(super) fn push_thread(
    lines: &mut Vec<Line<'static>>,
    nav_items: &mut Vec<NavItem>,
    thread: &CommentThread,
    (line, removed): (usize, bool),
    draw: &ThreadDraw<'_>,
) {
    let first = nav_items.len();
    let start = lines.len();
    let render = |lit: bool| {
        render_inline_thread(
            thread,
            draw.width,
            draw.now,
            lit,
            Some(draw.anchor_text),
            draw.reading,
            draw.expanded,
        )
    };
    let mut rendered = render(false);
    // The thread is lit while the cursor is on it or on one of its fold rows.
    if draw
        .active
        .is_some_and(|at| at >= first && at <= first + rendered.folds.len())
    {
        rendered = render(true);
    }
    let InlineThread {
        lines: thread_lines,
        folds,
    } = rendered;
    let span = thread_lines.len();
    for tline in thread_lines {
        let line_style = tline.style;
        let mut spans: Vec<Span<'static>> = Vec::with_capacity(tline.spans.len() + 1);
        spans.push(Span::raw(DIFF_GUTTER));
        spans.extend(tline.spans);
        lines.push(Line::from(spans).style(line_style));
    }
    let nav = || ThreadNav {
        line,
        removed,
        reply_to: thread.reply_to,
        handle: thread.anchor.as_ref().and_then(|a| a.handle.clone()),
        resolved: thread.resolved(),
    };
    nav_items.push(NavItem {
        rendered_row: start,
        row_span: span,
        kind: NavKind::Thread(nav()),
    });
    for fold in folds {
        nav_items.push(NavItem {
            rendered_row: start + fold.row,
            row_span: 1,
            kind: NavKind::Fold {
                thread: nav(),
                key: fold.key,
            },
        });
    }
}

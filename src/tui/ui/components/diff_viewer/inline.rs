//! What stands under a line of the diff: the threads on it, the comments queued
//! for it and what an agent proposed there, each found by its line and drawn
//! below it.

use super::pane::DIFF_GUTTER;
use crate::{
    domain::{
        comment::CommentThread,
        diff::LineRef,
        proposal::{Proposal, Side},
        review::PendingComment,
    },
    tui::ui::{
        components::diff_viewer::ProposalAt,
        theme,
        widgets::{comment::meta, markdown},
    },
};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use std::collections::HashMap;

pub(super) type CommentIndex<'a> = HashMap<usize, Vec<&'a CommentThread>>;

pub(super) fn index_comments<'a>(
    threads: &'a [CommentThread],
    path: &str,
    revision: Option<&crate::domain::diff::DiffRevision>,
) -> (CommentIndex<'a>, CommentIndex<'a>) {
    let mut by_new: CommentIndex<'_> = HashMap::new();
    let mut by_old: CommentIndex<'_> = HashMap::new();
    // Only anchored (code-review) threads land in the diff; general discussion
    // has no path/line and is skipped.
    for thread in threads {
        let Some(anchor) = &thread.anchor else {
            continue;
        };
        if anchor.path != path || !thread.matches_revision(revision) {
            continue;
        }
        match anchor.line {
            Some(LineRef::New(line)) => by_new.entry(line).or_default().push(thread),
            Some(LineRef::Old(line)) => by_old.entry(line).or_default().push(thread),
            None => {}
        }
    }
    (by_new, by_old)
}

pub(super) type PendingIndex<'a> = HashMap<usize, Vec<(usize, &'a PendingComment)>>;

/// Bucket the queued review comments for `path` by their anchor line, carrying
/// each one's index in the original `pending` slice (so `d` can remove it).
pub(super) fn index_pending<'a>(
    pending: &'a [PendingComment],
    path: &str,
    revision: Option<&crate::domain::diff::DiffRevision>,
) -> (PendingIndex<'a>, PendingIndex<'a>) {
    let mut by_new: PendingIndex<'_> = HashMap::new();
    let mut by_old: PendingIndex<'_> = HashMap::new();
    for (i, pc) in pending.iter().enumerate() {
        if pc.anchor.path != path || pc.anchor.revision.as_ref() != revision {
            continue;
        }
        let bucket = if pc.anchor.removed {
            &mut by_old
        } else {
            &mut by_new
        };
        bucket.entry(pc.anchor.line).or_default().push((i, pc));
    }
    (by_new, by_old)
}

pub(super) type ProposalIndex<'a> = HashMap<usize, Vec<ProposalAt<'a>>>;

/// Bucket the agents' proposals for `path` by the line they are about, on the
/// new side of the file and on the old.
pub(super) fn index_proposals<'a>(
    proposals: &[ProposalAt<'a>],
    path: &str,
) -> (ProposalIndex<'a>, ProposalIndex<'a>) {
    let mut by_new: ProposalIndex<'_> = HashMap::new();
    let mut by_old: ProposalIndex<'_> = HashMap::new();
    for proposed in proposals.iter().filter(|p| p.proposal.path() == path) {
        let bucket = match proposed.proposal.side() {
            Side::New => &mut by_new,
            Side::Old => &mut by_old,
        };
        bucket
            .entry(proposed.proposal.line())
            .or_default()
            .push(*proposed);
    }
    (by_new, by_old)
}

/// An agent's proposal, drawn apart from what people wrote: its own bar and an
/// `[AI]` tag with the agent's name, and what the keys do with it while the
/// cursor is on it. Returns the row count for nav spans.
pub(super) fn push_proposal_lines(
    lines: &mut Vec<Line<'static>>,
    proposal: &Proposal,
    width: u16,
    active: bool,
) -> usize {
    let theme = theme::current();
    let bar = Style::default().fg(theme.info);
    let body_style = Style::default().fg(theme.fg);
    let row_style = if active {
        Style::default().bg(theme.highlight_bg)
    } else {
        Style::default()
    };
    let text_width = width.saturating_sub(2);

    let mut title = vec![
        Span::styled(format!("{DIFF_GUTTER}▌ "), bar),
        meta::ai_tag(),
        Span::styled(
            format!(" {}", proposal.agent().unwrap_or("proposal")),
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        ),
    ];
    // What it is: a comment from a review that has not been posted, which only
    // the reader can send.
    title.push(Span::styled(
        "  ·  proposed comment, not posted",
        Style::default().fg(theme.muted),
    ));
    if active {
        title.push(Span::styled(
            "   c: take as a comment  d: discard",
            Style::default().fg(theme.muted),
        ));
    }
    let mut block: Vec<Line<'static>> = vec![Line::from(title)];
    for body_line in markdown::render_no_margin(proposal.body(), text_width) {
        let mut spans = vec![Span::styled(format!("{DIFF_GUTTER}▌ "), bar)];
        spans.extend(
            body_line
                .spans
                .into_iter()
                .map(|s| Span::styled(s.content, body_style)),
        );
        block.push(Line::from(spans));
    }

    let count = block.len();
    for line in block {
        lines.push(line.style(row_style));
    }
    count
}

/// A queued review comment, rendered as a draft block (accent bar + `pending`
/// tag) so it reads as not-yet-posted. Returns the row count for nav spans.
pub(super) fn push_pending_lines(
    lines: &mut Vec<Line<'static>>,
    text: &str,
    width: u16,
    active: bool,
) -> usize {
    let theme = theme::current();
    let bar = Style::default().fg(theme.accent);
    let body_style = Style::default().fg(theme.fg);
    let row_style = if active {
        Style::default().bg(theme.highlight_bg)
    } else {
        Style::default()
    };
    let text_width = width.saturating_sub(2);

    let mut block: Vec<Line<'static>> = vec![Line::from(vec![
        Span::styled(format!("{DIFF_GUTTER}▌ "), bar),
        Span::styled(
            "pending",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
    ])];
    for body_line in markdown::render_no_margin(text, text_width) {
        let mut spans = vec![Span::styled(format!("{DIFF_GUTTER}▌ "), bar)];
        spans.extend(
            body_line
                .spans
                .into_iter()
                .map(|s| Span::styled(s.content, body_style)),
        );
        block.push(Line::from(spans));
    }

    let count = block.len();
    for line in block {
        lines.push(line.style(row_style));
    }
    count
}

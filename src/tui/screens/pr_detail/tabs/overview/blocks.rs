//! The blocks of the Overview timeline: one per comment, thread or event, and
//! the rail that joins them.

use crate::{
    domain::{
        comment::{Comment, CommentId, CommentKind, CommentThread},
        diff::Diff,
        event::{EventKind, TimelineEvent},
    },
    tui::{
        screens::pr_detail::view::{CommentRef, ThreadRef},
        theme, widgets,
    },
};
use chrono::{DateTime, Utc};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use std::cmp::Reverse;

pub(super) enum TimelineItem<'a> {
    Comment(&'a Comment),
    Review(&'a CommentThread),
    Event(&'a TimelineEvent),
}

impl TimelineItem<'_> {
    pub(super) fn timestamp(&self) -> DateTime<Utc> {
        match self {
            TimelineItem::Comment(c) => c.created,
            TimelineItem::Review(t) => t.comments.first().map_or_else(Utc::now, |c| c.created),
            TimelineItem::Event(e) => e.created,
        }
    }
}

pub(super) struct TimelineBlock {
    lines: Vec<Line<'static>>,
    selected_range: Option<std::ops::Range<usize>>,
    /// Colour of the rail node (`*`) for this activity.
    node: Color,
    /// Colour of the dash after the node — tracks the box's left border
    /// (muted-light when focused, divider otherwise).
    border: Color,
    reply_to: Option<CommentId>,
    /// Comments and review threads are focus targets for j/k; events render
    /// inline for context but the cursor skips them.
    focusable: bool,
    /// The individual comments in this block, for the Ctrl-j/k sub-cursor.
    comments: Vec<CommentRef>,
    /// Set when the block is a review thread, so `R` can resolve it.
    resolve: Option<ThreadRef>,
}

pub(super) struct ItemNav {
    pub(super) start: usize,
    pub(super) span: usize,
    pub(super) selected_range: Option<std::ops::Range<usize>>,
    pub(super) reply_to: Option<CommentId>,
    pub(super) focusable: bool,
    pub(super) comments: Vec<CommentRef>,
    pub(super) resolve: Option<ThreadRef>,
}

/// How many items the cursor can land on: comments and non-empty review threads.
/// Events are shown inline but aren't focus targets.
pub(super) fn focusable_count(comments: &[&Comment], threads: &[&CommentThread]) -> usize {
    comments.len() + threads.iter().filter(|t| !t.comments.is_empty()).count()
}

#[expect(clippy::too_many_arguments, reason = "render inputs; see ROADMAP")]
pub(super) fn build_blocks(
    comments: &[&Comment],
    threads: &[&CommentThread],
    events: &[TimelineEvent],
    diff: Option<&Diff>,
    width: u16,
    focused: usize,
    sub: usize,
    author: &str,
) -> Vec<TimelineBlock> {
    let mut items: Vec<TimelineItem<'_>> =
        Vec::with_capacity(comments.len() + threads.len() + events.len());
    items.extend(comments.iter().copied().map(TimelineItem::Comment));
    items.extend(threads.iter().copied().map(TimelineItem::Review));
    items.extend(events.iter().map(TimelineItem::Event));
    items.sort_by_key(|item| Reverse(item.timestamp()));

    let theme = theme::current();
    let now = Utc::now();
    let mut blocks: Vec<TimelineBlock> = Vec::new();
    // `focus_idx` counts only focusable blocks, so the cursor (which indexes
    // comments/threads) lines up with the block we mark active.
    let mut focus_idx = 0;
    for item in &items {
        let active = focus_idx == focused;
        match item {
            TimelineItem::Comment(c) => {
                blocks.push(TimelineBlock {
                    lines: widgets::comment::comment_box(c, width, now, active, author),
                    selected_range: None,
                    node: theme.link,
                    border: if active { theme.accent } else { theme.divider },
                    reply_to: c.reply_to,
                    focusable: true,
                    comments: vec![CommentRef::new(c.id, CommentKind::Conversation)],
                    resolve: None,
                });
                focus_idx += 1;
            }
            TimelineItem::Review(t) => {
                // Mark the sub-selected comment only on the focused thread.
                let selected = active.then_some(sub);
                if let Some((lines, selected_range)) = widgets::comment::comment_thread_box(
                    t, diff, width, now, active, selected, author,
                ) {
                    blocks.push(TimelineBlock {
                        lines,
                        selected_range,
                        node: theme.link,
                        border: if active { theme.accent } else { theme.divider },
                        reply_to: t.reply_to,
                        focusable: true,
                        comments: t
                            .comments
                            .iter()
                            .map(|c| CommentRef::new(c.id, t.kind()))
                            .collect(),
                        // Only anchored threads can be resolved — general
                        // discussion has no resolve target (so `R` no-ops).
                        resolve: t.anchor.as_ref().map(|a| ThreadRef {
                            handle: a.handle.clone(),
                            comment_id: t.reply_to,
                            resolved: a.resolved,
                        }),
                    });
                    focus_idx += 1;
                }
            }
            TimelineItem::Event(e) => {
                let (color, lines) = event_block(e, now);
                blocks.push(TimelineBlock {
                    lines,
                    selected_range: None,
                    node: color,
                    border: theme.divider,
                    reply_to: None,
                    focusable: false,
                    comments: Vec::new(),
                    resolve: None,
                });
            }
        }
    }
    blocks
}

pub(super) fn timeline_rail(blocks: Vec<TimelineBlock>) -> (Vec<Line<'static>>, Vec<ItemNav>) {
    let theme = theme::current();
    let connector = || Span::styled("|  ", Style::default().fg(theme.divider));

    let mut all: Vec<Line<'static>> = Vec::new();
    let mut navs: Vec<ItemNav> = Vec::new();
    for (i, block) in blocks.into_iter().enumerate() {
        // Connector lines keep the rail unbroken in the gap between activities.
        if i > 0 {
            all.push(Line::from(connector()));
            all.push(Line::from(connector()));
        }
        let start = all.len();
        let span = block.lines.len();
        // The node keeps its type colour; the dash reaching into the activity is muted.
        let node = vec![
            Span::styled(
                "*",
                Style::default().fg(block.node).add_modifier(Modifier::BOLD),
            ),
            Span::styled("─ ", Style::default().fg(block.border)),
        ];
        for (j, line) in block.lines.into_iter().enumerate() {
            let mut spans = if j == 0 {
                node.clone()
            } else {
                vec![connector()]
            };
            spans.extend(line.spans);
            all.push(Line::from(spans));
        }
        navs.push(ItemNav {
            start,
            span,
            selected_range: block.selected_range.map(|r| start + r.start..start + r.end),
            reply_to: block.reply_to,
            focusable: block.focusable,
            comments: block.comments,
            resolve: block.resolve,
        });
    }

    if all.is_empty() {
        all.push(Line::default());
    }
    (all, navs)
}

fn event_block(event: &TimelineEvent, now: DateTime<Utc>) -> (Color, Vec<Line<'static>>) {
    let theme = theme::current();

    let header = |verb: String, color: Color| -> Line<'static> {
        let mut lead: Vec<Span<'static>> = Vec::new();
        if let Some(actor) = &event.actor {
            lead.push(Span::styled(
                format!("@{}", actor.username),
                Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
            ));
            lead.push(Span::raw(" "));
        }
        lead.push(Span::styled(verb, Style::default().fg(color)));
        widgets::author_line(lead, event.created, now)
    };

    let (verb, color) = match &event.kind {
        EventKind::Pushed(commits) => {
            let verb = if commits.len() == 1 {
                "added 1 commit".to_string()
            } else {
                format!("added {} commits", commits.len())
            };
            let mut lines = vec![header(verb, theme.info)];
            for c in commits {
                lines.push(Line::from(vec![
                    Span::styled(format!("{}  ", c.id), Style::default().fg(theme.warning)),
                    Span::styled(c.message.clone(), Style::default().fg(theme.muted)),
                ]));
            }
            return (theme.info, lines);
        }
        EventKind::Opened => ("opened this pull request", theme.success),
        EventKind::Approved => ("approved these changes", theme.success),
        EventKind::ChangesRequested => ("requested changes", theme.error),
        EventKind::ReviewRemoved => ("dismissed their review", theme.muted),
        EventKind::Merged => ("merged this pull request", theme.status_merged),
        EventKind::Declined => ("declined this pull request", theme.status_declined),
        EventKind::Reopened => ("reopened this pull request", theme.success),
    };
    (color, vec![header(verb.to_string(), color)])
}

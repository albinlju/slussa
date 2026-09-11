use crate::{
    app::{
        action::{Action, DetailAction},
        store::{LoadState, PrData},
    },
    domain::{
        comment::{Comment, CommentThread},
        diff::Diff,
        event::{EventKind, TimelineEvent},
    },
    tui::{
        component::{Component, scroll, step_index},
        screens::pr_detail::view::{CommentRef, ThreadRef},
        theme, widgets,
    },
};
use chrono::{DateTime, Utc};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use std::cmp::Reverse;

const TIMELINE_RIGHT_PAD: u16 = 3;
const RAIL_WIDTH: u16 = 3;

pub struct TimelineContext<'a> {
    pub data: Option<&'a PrData>,
    pub author: &'a str,
    pub scrollbar: Rect,
}

fn render_timeline(
    frame: &mut Frame,
    pr_data: Option<&PrData>,
    ui: &mut Timeline,
    author: &str,
    area: Rect,
    scrollbar_area: Rect,
) {
    let Some(activity) =
        widgets::loaded_or_placeholder(frame, pr_data.map(|d| &d.activity), "activity", area)
    else {
        return;
    };

    if activity.comments.is_empty() && activity.threads.is_empty() && activity.events.is_empty() {
        frame.render_widget(widgets::empty_state("(no activity)"), area);
        return;
    }

    let diff = pr_data.and_then(|d| match &d.diff {
        LoadState::Loaded(diff) => Some(diff),
        _ => None,
    });

    let count = focusable_count(&activity.comments, &activity.threads);
    let cursor = ui.cursor.min(count.saturating_sub(1));
    ui.item_count = count;
    ui.cursor = cursor;

    let blocks = build_blocks(
        &activity.comments,
        &activity.threads,
        &activity.events,
        diff,
        area.width.saturating_sub(TIMELINE_RIGHT_PAD + RAIL_WIDTH),
        cursor,
        ui.sub,
        author,
    );

    let (content, navs) = timeline_rail(blocks);

    // The cursor walks only the focusable items; resolve it to the matching nav
    // for highlight-scroll, reply target, and the per-comment sub-cursor.
    let focused = navs.iter().filter(|n| n.focusable).nth(cursor);
    ui.reply = focused.and_then(|n| n.reply_to);
    ui.thread = focused.and_then(|n| n.resolve.clone());
    let block_len = focused.map_or(0, |n| n.comments.len());
    let sub = ui.sub.min(block_len.saturating_sub(1));
    ui.block_len = block_len;
    ui.sub = sub;
    ui.selected = focused.and_then(|n| n.comments.get(sub).copied());

    let viewport = area.height as usize;
    let max_scroll = content.len().saturating_sub(viewport) as u16;
    let scroll = if count <= 1 {
        // 0 or 1 comments: nothing to jump between — the timeline scrolls freely
        // (driven by the component) so events above/below stay reachable.
        ui.scroll
    } else if cursor == 0 {
        // On the first comment, pin to the top so newer events above it show.
        0
    } else if cursor + 1 == count {
        // On the last comment, pin to the bottom so trailing events show.
        max_scroll
    } else {
        focused.map_or(ui.scroll, |n| {
            scroll_to_item(ui.scroll, n.start, n.span, content.len(), viewport)
        })
    };
    ui.scroll = scroll.min(max_scroll);
    ui.viewport = area.height;

    let content_area = Rect {
        width: area.width.saturating_sub(TIMELINE_RIGHT_PAD),
        ..area
    };
    frame.render_widget(Paragraph::new(content).scroll((ui.scroll, 0)), content_area);

    if max_scroll > 0 {
        let bar = widgets::scrollbar(ui.scroll, max_scroll, scrollbar_area.height);
        frame.render_widget(Paragraph::new(bar), scrollbar_area);
    }
}

fn scroll_to_item(scroll: u16, start: usize, span: usize, total: usize, viewport: usize) -> u16 {
    let max_scroll = total.saturating_sub(viewport) as u16;
    let start = start as u16;
    let end = start + (span.max(1) as u16) - 1;
    let mut s = scroll.min(max_scroll);
    if start < s {
        s = start;
    } else if viewport > 0 && end >= s + viewport as u16 {
        s = end.saturating_sub(viewport as u16 - 1);
    }
    s.min(max_scroll)
}

enum TimelineItem<'a> {
    Comment(&'a Comment),
    Review(&'a CommentThread),
    Event(&'a TimelineEvent),
}

impl TimelineItem<'_> {
    fn timestamp(&self) -> DateTime<Utc> {
        match self {
            TimelineItem::Comment(c) => c.created,
            TimelineItem::Review(t) => t.comments.first().map_or_else(Utc::now, |c| c.created),
            TimelineItem::Event(e) => e.created,
        }
    }
}

struct TimelineBlock {
    lines: Vec<Line<'static>>,
    /// Colour of the rail node (`*`) for this activity.
    node: Color,
    /// Colour of the dash after the node — tracks the box's left border
    /// (muted-light when focused, divider otherwise).
    border: Color,
    reply_to: Option<u64>,
    /// Comments and review threads are focus targets for j/k; events render
    /// inline for context but the cursor skips them.
    focusable: bool,
    /// The individual comments in this block, for the Ctrl-j/k sub-cursor.
    comments: Vec<CommentRef>,
    /// Set when the block is a review thread, so `R` can resolve it.
    resolve: Option<ThreadRef>,
}

struct ItemNav {
    start: usize,
    span: usize,
    reply_to: Option<u64>,
    focusable: bool,
    comments: Vec<CommentRef>,
    resolve: Option<ThreadRef>,
}

/// How many items the cursor can land on: comments and non-empty review threads.
/// Events are shown inline but aren't focus targets.
fn focusable_count(comments: &[Comment], threads: &[CommentThread]) -> usize {
    comments.len() + threads.iter().filter(|t| !t.comments.is_empty()).count()
}

#[allow(clippy::too_many_arguments)]
fn build_blocks(
    comments: &[Comment],
    threads: &[CommentThread],
    events: &[TimelineEvent],
    diff: Option<&Diff>,
    width: u16,
    focused: usize,
    sub: usize,
    author: &str,
) -> Vec<TimelineBlock> {
    let mut items: Vec<TimelineItem<'_>> =
        Vec::with_capacity(comments.len() + threads.len() + events.len());
    items.extend(comments.iter().map(TimelineItem::Comment));
    items.extend(threads.iter().map(TimelineItem::Review));
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
                    lines: crate::tui::widgets::comment::comment_box(c, width, now, active, author),
                    node: theme.link,
                    border: if active { theme.muted } else { theme.divider },
                    reply_to: c.reply_to,
                    focusable: true,
                    comments: vec![CommentRef {
                        id: c.id,
                        review: false,
                    }],
                    resolve: None,
                });
                focus_idx += 1;
            }
            TimelineItem::Review(t) => {
                // Mark the sub-selected comment only on the focused thread.
                let selected = active.then_some(sub);
                if let Some(lines) = crate::tui::widgets::comment::comment_thread_box(
                    t, diff, width, now, active, selected, author,
                ) {
                    blocks.push(TimelineBlock {
                        lines,
                        node: theme.link,
                        border: if active { theme.muted } else { theme.divider },
                        reply_to: t.reply_to,
                        focusable: true,
                        comments: t
                            .comments
                            .iter()
                            .map(|c| CommentRef {
                                id: c.id,
                                // Anchored threads are review comments; a general
                                // discussion's are PR-level (issue) comments.
                                review: t.anchor.is_some(),
                            })
                            .collect(),
                        // Only anchored threads can be resolved — general
                        // discussion has no resolve target (so `R` no-ops).
                        resolve: t.anchor.as_ref().map(|a| ThreadRef {
                            node_id: a.node_id.clone(),
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

fn timeline_rail(blocks: Vec<TimelineBlock>) -> (Vec<Line<'static>>, Vec<ItemNav>) {
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

    if let EventKind::Pushed(commits) = &event.kind {
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

    let (verb, color) = match &event.kind {
        EventKind::Opened => ("opened this pull request", theme.success),
        EventKind::Approved => ("approved these changes", theme.success),
        EventKind::ChangesRequested => ("requested changes", theme.error),
        EventKind::ReviewRemoved => ("dismissed their review", theme.muted),
        EventKind::Merged => ("merged this pull request", theme.status_merged),
        EventKind::Declined => ("declined this pull request", theme.status_declined),
        EventKind::Reopened => ("reopened this pull request", theme.success),
        EventKind::Pushed(_) => unreachable!("handled above"),
    };
    (color, vec![header(verb.to_string(), color)])
}

#[derive(Debug, Default)]
pub struct Timeline {
    pub scroll: u16,
    pub cursor: usize,
    pub item_count: usize,
    pub reply: Option<u64>,
    pub thread: Option<ThreadRef>,
    pub sub: usize,
    pub block_len: usize,
    pub selected: Option<CommentRef>,
    pub viewport: u16,
}

impl Component for Timeline {
    type Context<'a> = TimelineContext<'a>;
    type Message = DetailAction;
    fn handle_key(&self, key: KeyEvent, _: &Self::Context<'_>) -> Option<Action> {
        if key
            .modifiers
            .contains(ratatui::crossterm::event::KeyModifiers::CONTROL)
        {
            match key.code {
                KeyCode::Char('j') => {
                    return Some(Action::Detail(DetailAction::OverviewSubMove(1)));
                }
                KeyCode::Char('k') => {
                    return Some(Action::Detail(DetailAction::OverviewSubMove(-1)));
                }
                _ => {}
            }
        }

        let delta = match key.code {
            KeyCode::Char('j') | KeyCode::Down => 1,
            KeyCode::Char('k') | KeyCode::Up => -1,
            KeyCode::PageDown => crate::tui::screens::half_page(self.viewport),
            KeyCode::PageUp => -crate::tui::screens::half_page(self.viewport),
            _ => return None,
        };
        Some(Action::Detail(DetailAction::OverviewMove(delta)))
    }
    fn update(&mut self, action: DetailAction, _: &Self::Context<'_>) -> Option<Action> {
        match action {
            DetailAction::OverviewMove(delta) => {
                if self.item_count <= 1 {
                    self.scroll = scroll(self.scroll, delta);
                } else {
                    let next = step_index(self.cursor, delta, self.item_count);
                    if next != self.cursor {
                        self.cursor = next;
                        self.sub = 0;
                    }
                }
            }
            DetailAction::OverviewSubMove(delta) => {
                self.sub = step_index(self.sub, delta, self.block_len);
            }
            _ => {}
        }
        None
    }
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Self::Context<'_>) {
        render_timeline(frame, ctx.data, self, ctx.author, area, ctx.scrollbar);
    }
}

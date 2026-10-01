use super::blocks::{TimelineItem, build_blocks, focusable_count, timeline_rail};
use crate::tui::widgets::{comment_fold::Folds, comment_meta::Roles};
use crate::{
    app::{
        action::{Action, Effect, TimelineAction},
        store::{LoadState, PrData},
    },
    domain::{
        authorship::{AiMarkers, AuthorFilter},
        comment::{Comment, CommentId, CommentKey, CommentKind, CommentThread},
        event::TimelineEvent,
    },
    tui::{
        component::{Component, saturating_u16, scroll, scroll_to_item, step_index},
        screens::pr_detail::view::{CommentRef, ThreadRef},
        widgets,
    },
};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::Rect,
    widgets::Paragraph,
};
use std::{cmp::Reverse, collections::HashSet};

const TIMELINE_RIGHT_PAD: u16 = 3;
const RAIL_WIDTH: u16 = 3;

pub struct TimelineContext<'a> {
    pub data: Option<&'a PrData>,
    pub pr_author: &'a str,
    pub markers: &'a AiMarkers,
    pub scrollbar: Rect,
}

fn render_timeline(
    frame: &mut Frame<'_>,
    pr_data: Option<&PrData>,
    ui: &mut Timeline,
    (pr_author, markers): (&str, &AiMarkers),
    area: Rect,
    scrollbar_area: Rect,
) {
    let Some(activity) =
        widgets::loaded_or_placeholder(frame, pr_data.map(|d| &d.activity), "activity", area)
    else {
        return;
    };

    if activity.comments.is_empty() && activity.threads.is_empty() && activity.events.is_empty() {
        *ui = Timeline::default();
        frame.render_widget(widgets::empty_state("(no activity)"), area);
        return;
    }

    let roles = Roles {
        pr_author,
        markers,
        folds: Folds::Long {
            opened: &ui.expanded,
        },
    };

    // What the filter leaves: events are not comments, so they go with it.
    let comments: Vec<&Comment> = activity
        .comments
        .iter()
        .filter(|c| ui.filter.shows(roles.markers.of_comment(c)))
        .collect();
    let threads: Vec<&CommentThread> = activity
        .threads
        .iter()
        .filter(|t| ui.filter.shows(roles.markers.of_thread(t)))
        .collect();
    let events: &[TimelineEvent] = if ui.filter == AuthorFilter::All {
        &activity.events
    } else {
        &[]
    };
    if comments.is_empty() && threads.is_empty() && events.is_empty() {
        *ui = Timeline {
            filter: ui.filter,
            ..Timeline::default()
        };
        frame.render_widget(widgets::empty_state(ui.filter.empty_text()), area);
        return;
    }

    // Anchored comments gain snippets (and suggestion context) from the diff.
    // On first load, reveal them together instead of resizing cards under the reader.
    let waiting_for_context = threads.iter().any(|thread| thread.anchor.is_some())
        && pr_data.is_some_and(|data| matches!(data.diff, LoadState::Loading));
    if waiting_for_context {
        frame.render_widget(
            Paragraph::new(widgets::loading("Loading code context…")),
            area,
        );
        return;
    }

    let diff = pr_data.and_then(|data| data.diff.loaded());

    let previous_selection = ui.selected.filter(|selected| selected.id.is_some());
    if let Some(selected) = previous_selection {
        let mut items: Vec<_> = comments
            .iter()
            .copied()
            .map(TimelineItem::Comment)
            .chain(
                threads
                    .iter()
                    .copied()
                    .filter(|t| !t.comments.is_empty())
                    .map(TimelineItem::Review),
            )
            .collect();
        items.sort_by_key(|item| Reverse(item.timestamp()));
        let found = items
            .iter()
            .enumerate()
            .find_map(|(cursor, item)| match item {
                TimelineItem::Comment(comment)
                    if selected == CommentRef::new(comment.id, CommentKind::Conversation) =>
                {
                    Some((cursor, 0))
                }
                TimelineItem::Review(thread) if selected.kind == thread.kind() => thread
                    .comments
                    .iter()
                    .position(|comment| comment.id == selected.id)
                    .map(|sub| (cursor, sub)),
                TimelineItem::Comment(_) | TimelineItem::Review(_) | TimelineItem::Event(_) => None,
            });
        if let Some((cursor, sub)) = found {
            ui.cursor = cursor;
            ui.sub = sub;
        } else {
            ui.sub = 0;
            ui.selected_row = None;
            ui.reveal_selection = true;
        }
    }

    let count = focusable_count(&comments, &threads);
    let cursor = ui.cursor.min(count.saturating_sub(1));
    ui.item_count = count;
    ui.cursor = cursor;

    let blocks = build_blocks(
        &comments,
        &threads,
        events,
        diff,
        area.width.saturating_sub(TIMELINE_RIGHT_PAD + RAIL_WIDTH),
        cursor,
        ui.sub,
        roles,
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

    let selected_row = focused.map(|n| n.selected_range.as_ref().map_or(n.start, |r| r.start));
    if previous_selection.is_some_and(|old| ui.selected.is_some_and(|new| old == new))
        && let (Some(before), Some(after)) = (ui.selected_row, selected_row)
    {
        ui.scroll = saturating_u16(
            usize::from(ui.scroll)
                .saturating_add(after)
                .saturating_sub(before),
        );
    }
    ui.selected_row = selected_row;

    let viewport = area.height as usize;
    let max_scroll = saturating_u16(content.len().saturating_sub(viewport));
    let scroll = if ui.reveal_selection {
        focused.map_or(ui.scroll, |n| {
            let range = n
                .selected_range
                .clone()
                .unwrap_or(n.start..n.start + n.span);
            scroll_to_item(ui.scroll, range.start, range.len(), content.len(), viewport)
        })
    } else {
        ui.scroll
    };
    ui.reveal_selection = false;
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

#[derive(Debug, Default)]
pub struct Timeline {
    pub scroll: u16,
    pub cursor: usize,
    pub item_count: usize,
    pub reply: Option<CommentId>,
    pub thread: Option<ThreadRef>,
    pub sub: usize,
    pub block_len: usize,
    pub selected: Option<CommentRef>,
    pub viewport: u16,
    pub filter: AuthorFilter,
    /// The long comments the reader has opened with `space`.
    pub expanded: HashSet<CommentKey>,
    reveal_selection: bool,
    selected_row: Option<usize>,
}

impl Component for Timeline {
    type Input<'a> = ();
    type View<'a> = TimelineContext<'a>;
    type Message = TimelineAction;
    fn handle_key(&self, key: KeyEvent, (): &()) -> Option<Action> {
        let control = key
            .modifiers
            .contains(crossterm::event::KeyModifiers::CONTROL);
        let action = match key.code {
            KeyCode::Char('j') if control => TimelineAction::SubMove(1),
            KeyCode::Char('k') if control => TimelineAction::SubMove(-1),
            KeyCode::Char('j') | KeyCode::Down => TimelineAction::Move(1),
            KeyCode::Char('k') | KeyCode::Up => TimelineAction::Move(-1),
            KeyCode::PageDown => {
                TimelineAction::Scroll(crate::tui::screens::half_page(self.viewport))
            }
            KeyCode::PageUp => {
                TimelineAction::Scroll(-crate::tui::screens::half_page(self.viewport))
            }
            _ => return None,
        };
        Some(action.into())
    }
    fn update(&mut self, action: TimelineAction, (): &()) -> Option<Effect> {
        match action {
            TimelineAction::Scroll(delta) => {
                self.scroll = scroll(self.scroll, delta);
                self.reveal_selection = false;
            }
            TimelineAction::Move(delta) => {
                if self.item_count <= 1 {
                    self.scroll = scroll(self.scroll, delta);
                } else {
                    let next = step_index(self.cursor, delta, self.item_count);
                    if next != self.cursor {
                        self.selected = None;
                        self.selected_row = None;
                        self.cursor = next;
                        self.sub = 0;
                        self.reveal_selection = true;
                    }
                }
            }
            TimelineAction::CycleFilter => {
                *self = Self {
                    filter: self.filter.next(),
                    expanded: std::mem::take(&mut self.expanded),
                    reveal_selection: true,
                    ..Self::default()
                };
            }
            TimelineAction::ToggleFold => {
                if let Some(key) = self.selected.and_then(CommentRef::key)
                    && !self.expanded.remove(&key)
                {
                    self.expanded.insert(key);
                }
                // Keep the comment the reader is on in view as it grows or shrinks.
                self.reveal_selection = true;
            }
            TimelineAction::SubMove(delta) => {
                let next = step_index(self.sub, delta, self.block_len);
                if next != self.sub {
                    self.selected = None;
                    self.selected_row = None;
                }
                self.reveal_selection = next != self.sub;
                self.sub = next;
            }
        }
        None
    }
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, ctx: &TimelineContext<'_>) {
        render_timeline(
            frame,
            ctx.data,
            self,
            (ctx.pr_author, ctx.markers),
            area,
            ctx.scrollbar,
        );
    }
}

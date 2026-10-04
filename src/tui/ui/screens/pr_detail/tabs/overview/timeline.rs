use super::blocks::{Hidden, TimelineItem, build_blocks, focusable_count, timeline_rail};
use crate::tui::ui::widgets::comment::{fold::Folds, meta::Reading};
use crate::{
    domain::{
        authorship::AuthorFilter,
        comment::{Comment, CommentId, CommentKey, CommentKind, CommentThread},
    },
    tui::{
        app::{
            effect::Effect,
            store::{LoadState, PrData},
        },
        ui::{
            action::{Action, TimelineAction},
            component::{Component, saturating_u16, scroll, scroll_to_item, step_index},
            screens::pr_detail::view::{CommentRef, ThreadRef},
            widgets,
        },
    },
};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::Rect,
    widgets::Paragraph,
};
use std::{cmp::Reverse, collections::HashSet};

/// How many rows `j` or `k` scroll to read on through an item that does not fit
/// on the screen, before the next press moves to the neighbouring item.
const READ_STEP: usize = 5;

const TIMELINE_RIGHT_PAD: u16 = 3;
const RAIL_WIDTH: u16 = 3;

pub struct TimelineContext<'a> {
    pub data: Option<&'a PrData>,
    pub pr_author: &'a str,
    pub scrollbar: Rect,
}

fn render_timeline(
    frame: &mut Frame<'_>,
    pr_data: Option<&PrData>,
    ui: &mut Timeline,
    pr_author: &str,
    area: Rect,
    scrollbar_area: Rect,
) {
    let Some(activity) =
        widgets::loaded_or_placeholder(frame, pr_data.map(|d| &d.activity), "activity", area)
    else {
        // Nothing is drawn, so there is nowhere for `u` to go.
        ui.unresolved.clear();
        return;
    };

    if activity.comments.is_empty() && activity.threads.is_empty() && activity.events.is_empty() {
        *ui = Timeline::default();
        frame.render_widget(widgets::empty_state("(no activity)"), area);
        return;
    }

    let reading = Reading {
        pr_author,
        folds: Folds::Long {
            opened: &ui.expanded,
        },
    };

    // What the filter leaves, and what it hides: the events stay, and the hidden
    // comments come back as a dimmed line where they were.
    let (comments, hidden_comments): (Vec<&Comment>, Vec<&Comment>) = activity
        .comments
        .iter()
        .partition(|c| ui.filter.shows(c.authorship));
    let (threads, hidden_threads): (Vec<&CommentThread>, Vec<&CommentThread>) = activity
        .threads
        .iter()
        .partition(|t| ui.filter.shows(t.authorship()));

    // Anchored comments gain snippets (and suggestion context) from the diff.
    // On first load, reveal them together instead of resizing cards under the reader.
    let waiting_for_context = threads.iter().any(|thread| thread.anchor.is_some())
        && pr_data.is_some_and(|data| matches!(data.diff, LoadState::Loading));
    if waiting_for_context {
        ui.unresolved.clear();
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
            ui.reveal = Reveal::Selection;
        }
    }

    let count = focusable_count(&comments, &threads);
    let cursor = ui.cursor.min(count.saturating_sub(1));
    ui.item_count = count;
    ui.cursor = cursor;

    let blocks = build_blocks(
        &comments,
        &threads,
        ui.filter
            .hides()
            .map(|side| Hidden {
                side,
                comments: &hidden_comments,
                threads: &hidden_threads,
            })
            .as_ref(),
        &activity.events,
        diff,
        area.width.saturating_sub(TIMELINE_RIGHT_PAD + RAIL_WIDTH),
        cursor,
        ui.sub,
        reading,
    );

    let (content, navs) = timeline_rail(blocks);

    ui.unresolved = navs
        .iter()
        .filter(|nav| nav.focusable)
        .enumerate()
        .filter(|(_, nav)| nav.resolve.as_ref().is_some_and(|thread| !thread.resolved))
        .map(|(place, _)| place)
        .collect();

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
    ui.focused_rows = focused.map(|n| n.start..n.start + n.span);

    let viewport = area.height as usize;
    let max_scroll = saturating_u16(content.len().saturating_sub(viewport));
    let scroll = match (ui.reveal, focused) {
        (Reveal::Keep, _) | (_, None) => ui.scroll,
        (Reveal::End, Some(n)) if n.span > viewport => {
            // Stepped back up into an item taller than the screen: show its end.
            saturating_u16((n.start + n.span).saturating_sub(viewport))
        }
        (Reveal::Selection | Reveal::End, Some(n)) => {
            let range = n
                .selected_range
                .clone()
                .unwrap_or(n.start..n.start + n.span);
            scroll_to_item(ui.scroll, range.start, range.len(), content.len(), viewport)
        }
    };
    ui.reveal = Reveal::Keep;
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

/// What the next draw does about the scroll position.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Reveal {
    /// Leave it where it is.
    #[default]
    Keep,
    /// Bring the selected comment into view, moving as little as possible.
    Selection,
    /// Bring the end of the focused item into view: it was stepped back into
    /// from below.
    End,
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
    /// What the next draw does about the scroll position.
    reveal: Reveal,
    /// The items, by their place among the focusable ones, that are review threads
    /// not resolved: what `u` goes between.
    pub unresolved: Vec<usize>,
    /// The rows the focused item takes, so that `j` and `k` can read on through it.
    focused_rows: Option<std::ops::Range<usize>>,
    selected_row: Option<usize>,
}

/// Where `u` (delta 1) or `U` (-1) goes from `cursor`, among the unresolved
/// threads in order: the next one after it, or the first when there is none, and
/// the other way for `U`. None when there is no unresolved thread.
fn unresolved_from(unresolved: &[usize], cursor: usize, delta: i16) -> Option<usize> {
    if delta > 0 {
        unresolved
            .iter()
            .copied()
            .find(|&place| place > cursor)
            .or_else(|| unresolved.first().copied())
    } else {
        unresolved
            .iter()
            .rev()
            .copied()
            .find(|&place| place < cursor)
            .or_else(|| unresolved.last().copied())
    }
}

impl Timeline {
    /// How far `j` (down) or `k` (up) scrolls to read on through the focused item
    /// instead of leaving it, when some of it is out of view that way.
    fn read_on(&self, delta: i16) -> Option<i16> {
        let rows = self.focused_rows.as_ref()?;
        if self.viewport == 0 {
            return None;
        }
        let top = usize::from(self.scroll);
        let bottom = top + usize::from(self.viewport);
        let rows_to_go = if delta > 0 && rows.end > bottom {
            rows.end - bottom
        } else if delta < 0 && rows.start < top {
            top - rows.start
        } else {
            return None;
        };
        let step = i16::try_from(rows_to_go.min(READ_STEP)).unwrap_or(i16::MAX);
        Some(if delta > 0 { step } else { -step })
    }
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
            KeyCode::Char('u') if !control => TimelineAction::NextUnresolved(1),
            KeyCode::Char('U') if !control => TimelineAction::NextUnresolved(-1),
            KeyCode::Char('j') | KeyCode::Down => TimelineAction::Move(1),
            KeyCode::Char('k') | KeyCode::Up => TimelineAction::Move(-1),
            KeyCode::PageDown => {
                TimelineAction::Scroll(crate::tui::ui::screens::half_page(self.viewport))
            }
            KeyCode::PageUp => {
                TimelineAction::Scroll(-crate::tui::ui::screens::half_page(self.viewport))
            }
            _ => return None,
        };
        Some(action.into())
    }
    fn update(&mut self, action: TimelineAction, (): &()) -> Option<Effect> {
        match action {
            TimelineAction::Scroll(delta) => {
                self.scroll = scroll(self.scroll, delta);
                self.reveal = Reveal::Keep;
            }
            TimelineAction::Move(delta) => {
                if self.item_count <= 1 {
                    self.scroll = scroll(self.scroll, delta);
                } else if let Some(step) = self.read_on(delta) {
                    // Some of this item is out of view the way the reader is going.
                    self.scroll = scroll(self.scroll, step);
                    self.reveal = Reveal::Keep;
                } else {
                    let next = step_index(self.cursor, delta, self.item_count);
                    if next != self.cursor {
                        self.selected = None;
                        self.selected_row = None;
                        self.cursor = next;
                        self.sub = 0;
                        // Stepping back up, the end of what was stepped into is the
                        // part that is next to where the reader was.
                        self.reveal = if delta < 0 {
                            Reveal::End
                        } else {
                            Reveal::Selection
                        };
                    }
                }
            }
            TimelineAction::CycleFilter => {
                // From the top, with the cursor on the first comment left.
                *self = Self {
                    filter: self.filter.next(),
                    expanded: std::mem::take(&mut self.expanded),
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
                self.reveal = Reveal::Selection;
            }
            TimelineAction::NextUnresolved(delta) => {
                if let Some(target) = unresolved_from(&self.unresolved, self.cursor, delta)
                    && target != self.cursor
                {
                    self.selected = None;
                    self.selected_row = None;
                    self.cursor = target;
                    self.sub = 0;
                    self.reveal = Reveal::Selection;
                }
            }
            TimelineAction::SubMove(delta) => {
                let next = step_index(self.sub, delta, self.block_len);
                if next != self.sub {
                    self.selected = None;
                    self.selected_row = None;
                }
                self.reveal = if next == self.sub {
                    Reveal::Keep
                } else {
                    Reveal::Selection
                };
                self.sub = next;
            }
        }
        None
    }
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, ctx: &TimelineContext<'_>) {
        render_timeline(frame, ctx.data, self, ctx.pr_author, area, ctx.scrollbar);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u_goes_to_the_next_unresolved_thread_and_round_from_the_last() {
        let places = [1, 4, 6];
        assert_eq!(unresolved_from(&places, 0, 1), Some(1));
        assert_eq!(
            unresolved_from(&places, 1, 1),
            Some(4),
            "past the one it is on"
        );
        assert_eq!(unresolved_from(&places, 2, 1), Some(4), "from between two");
        assert_eq!(
            unresolved_from(&places, 6, 1),
            Some(1),
            "round to the first"
        );
    }

    #[test]
    fn capital_u_goes_the_other_way_and_round_from_the_first() {
        let places = [1, 4, 6];
        assert_eq!(unresolved_from(&places, 6, -1), Some(4));
        assert_eq!(unresolved_from(&places, 5, -1), Some(4));
        assert_eq!(
            unresolved_from(&places, 1, -1),
            Some(6),
            "round to the last"
        );
        assert_eq!(unresolved_from(&places, 0, -1), Some(6));
    }

    #[test]
    fn with_nothing_unresolved_there_is_nowhere_to_go() {
        assert_eq!(unresolved_from(&[], 3, 1), None);
        assert_eq!(unresolved_from(&[], 3, -1), None);
        // Alone, it stays where it is: the target is the cursor.
        assert_eq!(unresolved_from(&[2], 2, 1), Some(2));
    }
}

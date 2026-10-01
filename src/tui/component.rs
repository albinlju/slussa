//! The contract for interactive UI owners: screens, dialogs and the diff viewer.
//!
//! A component owns its local state. `handle_key` turns input into an `Action`
//! and changes nothing. `update` applies a message to local state. `render`
//! draws and may record layout-derived values such as the viewport size. The
//! `Context` is borrowed data, never the mutable application. A returned
//! `Action` asks the application for work; `None` means the component handled
//! it locally.
//!
//! A key ignored by a modal must not fall through to what is behind it. Callers
//! rely on the explicit modal and focus priority.

use crate::app::action::Action;
use ratatui::{Frame, crossterm::event::KeyEvent, layout::Rect};

/// Interactive UI owner. Context is borrowed data, never the mutable application.
/// Local updates stay here; returned actions request application-level work.
pub trait Component {
    type Context<'a>;
    type Message;

    fn handle_key(&self, key: KeyEvent, context: &Self::Context<'_>) -> Option<Action>;
    fn update(&mut self, message: Self::Message, context: &Self::Context<'_>) -> Option<Action>;
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, context: &Self::Context<'_>);
}

pub fn step_index(current: usize, delta: i16, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    (current as i64 + i64::from(delta)).clamp(0, (len - 1) as i64) as usize
}

/// A length or offset as ratatui's `u16`, capped instead of wrapped: a diff of
/// 70 000 lines must scroll to line 65 535, not to line 4 464.
pub const fn saturating_u16(value: usize) -> u16 {
    if value > u16::MAX as usize {
        u16::MAX
    } else {
        value as u16
    }
}

/// The scroll offset that brings an item of `span` rows starting at `start`
/// into a viewport, moving as little as possible. An item taller than the
/// viewport is shown from its first row.
pub fn scroll_to_item(
    scroll: u16,
    start: usize,
    span: usize,
    total: usize,
    viewport: usize,
) -> u16 {
    let max_scroll = saturating_u16(total.saturating_sub(viewport));
    let end = start.saturating_add(span.max(1).min(viewport.max(1)) - 1);
    let mut offset = usize::from(scroll.min(max_scroll));
    if start < offset {
        offset = start;
    } else if viewport > 0 && end >= offset + viewport {
        offset = end.saturating_sub(viewport - 1);
    }
    saturating_u16(offset).min(max_scroll)
}

pub const fn scroll(offset: u16, delta: i16) -> u16 {
    if delta >= 0 {
        offset.saturating_add(delta as u16)
    } else {
        offset.saturating_sub(delta.unsigned_abs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_past_the_u16_range_are_capped_and_not_wrapped() {
        assert_eq!(saturating_u16(0), 0);
        assert_eq!(saturating_u16(65_535), u16::MAX);
        assert_eq!(saturating_u16(70_000), u16::MAX);
    }

    #[test]
    fn an_item_is_brought_into_view_with_the_smallest_move() {
        // Already visible: stay.
        assert_eq!(scroll_to_item(5, 7, 2, 100, 10), 5);
        // Above the viewport: its first row becomes the top.
        assert_eq!(scroll_to_item(5, 2, 3, 100, 10), 2);
        // Below: its last row becomes the bottom.
        assert_eq!(scroll_to_item(0, 12, 3, 100, 10), 5);
        // Taller than the viewport: shown from its first row.
        assert_eq!(scroll_to_item(0, 20, 50, 100, 10), 20);
        // Never past the end of the content.
        assert_eq!(scroll_to_item(0, 99, 1, 100, 10), 90);
        assert_eq!(scroll_to_item(3, 0, 1, 5, 10), 0);
    }

    #[test]
    fn a_position_past_the_u16_range_scrolls_to_the_last_reachable_row() {
        assert_eq!(scroll_to_item(0, 70_000, 4, 80_000, 30), u16::MAX);
        assert_eq!(scroll_to_item(u16::MAX, 10, 1, 80_000, 30), 10);
    }
}

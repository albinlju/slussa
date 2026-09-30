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

pub const fn scroll(offset: u16, delta: i16) -> u16 {
    if delta >= 0 {
        offset.saturating_add(delta as u16)
    } else {
        offset.saturating_sub(delta.unsigned_abs())
    }
}

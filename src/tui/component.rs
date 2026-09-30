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

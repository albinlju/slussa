use crate::app::{App, action::Action};

pub mod commits;
pub mod detail;
pub mod diff;
pub mod list;
pub mod loads;
pub mod search;

/// Move an index by a signed delta, clamped to the collection; 0 when empty.
pub(super) fn step_index(current: usize, delta: i16, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    (current as i64 + delta as i64).clamp(0, (len - 1) as i64) as usize
}

/// Clamps at the top; the bottom is clamped at render time against the
/// actual content height.
pub(super) fn scroll(offset: u16, delta: i16) -> u16 {
    if delta >= 0 {
        offset.saturating_add(delta as u16)
    } else {
        offset.saturating_sub(delta.unsigned_abs())
    }
}

impl App {
    pub(super) fn apply(&mut self, action: Action) {
        match action {
            Action::Quit => unreachable!("handled in run()"),
            Action::List(a) => self.apply_list(a),
            Action::Detail(a) => self.apply_detail(a),
            Action::Diff(a) => self.apply_diff(a),
            Action::Commits(a) => self.apply_commits(a),
            Action::Search(a) => self.apply_search(a),
            Action::Loaded(a) => self.apply_loaded(a),
        }
    }
}

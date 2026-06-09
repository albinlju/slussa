use crate::app::{App, action::Action};

pub mod commits;
pub mod detail;
pub mod diff;
pub mod list;
pub mod loads;

/// Apply a signed line delta to a scroll offset, clamping at the top. The
/// bottom is clamped at render time against the actual content height.
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
            Action::Loaded(a) => self.apply_loaded(a),
        }
    }
}

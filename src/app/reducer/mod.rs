use crate::app::{App, action::Action};

pub mod commits;
pub mod detail;
pub mod diff;
pub mod list;
pub mod loads;
pub mod refresh;
pub mod search;

pub(super) fn step_index(current: usize, delta: i16, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    (current as i64 + i64::from(delta)).clamp(0, (len - 1) as i64) as usize
}

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
            Action::Refresh => self.refresh_actions(),
            Action::List(a) => self.list_actions(a),
            Action::Detail(a) => self.detail_actions(a),
            Action::Diff(a) => self.diff_actions(a),
            Action::Commits(a) => self.commits_actions(a),
            Action::Search(a) => self.search_actions(a),
            Action::Loaded(a) => self.loaded_actions(a),
        }
    }
}

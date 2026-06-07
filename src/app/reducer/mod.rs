//! Dispatcher for the reducer — `apply` peels off the outer `Action` variant
//! and hands the inner sub-action enum to the matching submodule's handler.
//! Add a new action subdomain by adding a variant here and a submodule.

use crate::{app::App, tui::Action};

pub mod detail;
pub mod diff;
pub mod list;
pub mod loads;

impl App {
    pub(super) fn apply(&mut self, action: Action) {
        match action {
            Action::Quit => unreachable!("handled in run()"),
            Action::List(a) => self.apply_list(a),
            Action::Detail(a) => self.apply_detail(a),
            Action::Diff(a) => self.apply_diff(a),
            Action::Loaded(a) => self.apply_loaded(a),
        }
    }
}

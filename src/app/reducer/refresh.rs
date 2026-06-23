use std::time::{Duration, Instant};

use crate::app::{
    App,
    state::{DetailTab, Screen},
};

/// CI moves fast, so the Builds tab re-fetches more often than everything else.
pub const BUILDS_INTERVAL: Duration = Duration::from_secs(15);
#[allow(clippy::duration_suboptimal_units)] // `from_mins` is still unstable
pub const FULL_INTERVAL: Duration = Duration::from_secs(60);

impl App {
    /// Background tick: re-fetch the active view on its cadence. Fires on the
    /// `BUILDS_INTERVAL` clock; the slower `FULL_INTERVAL` is gated by elapsed time.
    pub fn tick_refresh(&mut self) {
        if self.modal_open() {
            return;
        }
        let now = Instant::now();
        let full_due = now.duration_since(self.full_refreshed) >= FULL_INTERVAL;
        match self.state.screen {
            Screen::List => {
                if full_due {
                    self.refresh_prs();
                    self.full_refreshed = now;
                }
            }
            Screen::Detail { pr_id, tab } => {
                if tab == DetailTab::Builds {
                    self.refresh_builds(pr_id);
                }
                if full_due {
                    if tab != DetailTab::Builds {
                        self.refresh_detail_view(pr_id, tab);
                    }
                    self.refresh_mergeability(pr_id);
                    // The reviewer/approval state in the sidebar comes from the list.
                    self.refresh_prs();
                    self.full_refreshed = now;
                }
            }
        }
    }

    /// Manual `F`: re-fetch the active view right now, ignoring the cadence.
    pub(super) fn refresh_actions(&mut self) {
        if self.modal_open() {
            return;
        }
        match self.state.screen {
            Screen::List => self.refresh_prs(),
            Screen::Detail { pr_id, tab } => {
                self.refresh_detail_view(pr_id, tab);
                self.refresh_mergeability(pr_id);
                self.refresh_prs();
            }
        }
        self.full_refreshed = Instant::now();
    }

    fn refresh_detail_view(&mut self, pr_id: u64, tab: DetailTab) {
        match tab {
            DetailTab::Overview => self.refresh_activity(pr_id),
            DetailTab::Diff => self.refresh_diff(pr_id),
            DetailTab::Commits => self.refresh_commits(pr_id),
            DetailTab::Builds => self.refresh_builds(pr_id),
            // The description is the PR body; it rides along with the list refresh.
            DetailTab::Description => {}
        }
    }

    fn refresh_prs(&mut self) {
        if !self.state.cache.prs.is_loading() {
            self.state.ui.refreshing = true;
            self.spawn_load_prs();
        }
    }

    fn refresh_activity(&mut self, pr_id: u64) {
        if self.fetch_in_flight(pr_id, |d| d.activity.is_loading()) {
            return;
        }
        self.state.ui.refreshing = true;
        self.spawn_load_activity(pr_id);
    }

    fn refresh_diff(&mut self, pr_id: u64) {
        if self.fetch_in_flight(pr_id, |d| d.diff.is_loading()) {
            return;
        }
        self.state.ui.refreshing = true;
        self.spawn_load_diff(pr_id);
    }

    fn refresh_commits(&mut self, pr_id: u64) {
        if self.fetch_in_flight(pr_id, |d| d.commits.is_loading()) {
            return;
        }
        self.state.ui.refreshing = true;
        self.spawn_load_commits(pr_id);
    }

    fn refresh_builds(&mut self, pr_id: u64) {
        if self.fetch_in_flight(pr_id, |d| d.builds.is_loading()) {
            return;
        }
        self.state.ui.refreshing = true;
        self.spawn_load_builds(pr_id);
    }

    /// The mergeability badge lives in the always-visible header, so it re-checks
    /// on any detail refresh regardless of the focused tab.
    fn refresh_mergeability(&mut self, pr_id: u64) {
        if self.fetch_in_flight(pr_id, |d| d.mergeability.is_loading()) {
            return;
        }
        self.state.ui.refreshing = true;
        self.spawn_load_mergeability(pr_id);
    }

    fn fetch_in_flight(&self, pr_id: u64, f: impl Fn(&crate::app::state::PrData) -> bool) -> bool {
        self.state.cache.details.get(&pr_id).is_some_and(f)
    }

    fn modal_open(&self) -> bool {
        let ui = &self.state.ui;
        ui.comment_draft.is_some()
            || ui.confirm.is_some()
            || ui.error.is_some()
            || ui.help_open
            || ui.filter_picker_open
    }
}

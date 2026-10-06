use crate::{
    domain::pr::{PrGroup, PrId},
    tui::{
        app::{App, navigation::Screen},
        ui::screens::pr_detail::tabs::DetailTab,
    },
};
use std::time::{Duration, Instant};

/// CI moves fast, so the Builds tab re-fetches more often than everything else.
pub const BUILDS_INTERVAL: Duration = Duration::from_secs(15);
pub const FULL_INTERVAL: Duration = Duration::from_mins(1);

const _: () = assert!(BUILDS_INTERVAL.as_millis() < FULL_INTERVAL.as_millis());

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
                    self.refresh_list();
                    self.full_refreshed = now;
                }
            }
            Screen::Detail { pr_id, tab } => {
                if tab == DetailTab::Builds {
                    self.spawn_load_builds(pr_id);
                }
                if full_due {
                    self.read_proposals();
                    if tab != DetailTab::Builds {
                        self.refresh_detail_view(pr_id, tab);
                    }
                    self.spawn_load_mergeability(pr_id);
                    self.spawn_load_info(pr_id);
                    // The reviewer/approval state in the sidebar comes from the list.
                    self.spawn_load_prs(PrGroup::Open, None);
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
            Screen::List => self.refresh_list(),
            Screen::Detail { pr_id, tab } => {
                self.read_proposals();
                self.refresh_detail_view(pr_id, tab);
                self.spawn_load_mergeability(pr_id);
                self.spawn_load_info(pr_id);
                self.spawn_load_prs(PrGroup::Open, None);
            }
        }
        self.full_refreshed = Instant::now();
    }

    fn refresh_detail_view(&mut self, pr_id: PrId, tab: DetailTab) {
        match tab {
            DetailTab::Overview => self.spawn_load_activity(pr_id),
            DetailTab::Diff => {
                self.spawn_load_diff(pr_id);
                self.retry_new_since_read(pr_id);
            }
            DetailTab::Commits => self.spawn_load_commits(pr_id),
            DetailTab::Builds => self.spawn_load_builds(pr_id),
            // The description and labels are read with `spawn_load_info` below.
            DetailTab::Description => {}
        }
    }

    /// What is new, when it is on screen and could not be read, is asked for
    /// again. One that was read is left as it is.
    fn retry_new_since_read(&mut self, pr_id: PrId) {
        let Some(since) = &self.state.ui.detail.since else {
            return;
        };
        let range = since.range().clone();
        self.load_new_since(pr_id, range);
    }

    fn modal_open(&self) -> bool {
        self.state
            .ui
            .modal_open(&self.state.store, self.state.screen)
    }
}

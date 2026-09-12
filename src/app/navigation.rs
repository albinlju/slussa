use crate::{app::App, tui::screens::pr_detail::tabs::DetailTab};

impl App {
    pub(super) fn open_pr(&mut self, pr_id: u64) {
        self.state.ui.open_pr(pr_id);
        self.state.screen = Screen::Detail {
            pr_id,
            tab: self.state.ui.detail.active_tab,
        };

        let pr_data = self.state.store.cache.details.entry(pr_id).or_default();
        let load_commits = pr_data.commits.start_loading();
        let load_diff = pr_data.diff.start_loading();
        let load_builds = self
            .state
            .store
            .capabilities
            .supports(crate::domain::capabilities::Feature::Builds)
            && pr_data.builds.start_loading();
        let load_activity = pr_data.activity.start_loading();
        let load_mergeability = self
            .state
            .store
            .capabilities
            .supports(crate::domain::capabilities::Feature::Mergeability)
            && pr_data.mergeability.start_loading();
        if load_commits {
            self.spawn_load_commits(pr_id);
        }
        if load_diff {
            self.spawn_load_diff(pr_id);
        }
        if load_builds {
            self.spawn_load_builds(pr_id);
        }
        if load_activity {
            self.spawn_load_activity(pr_id);
        }
        if load_mergeability {
            self.spawn_load_mergeability(pr_id);
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Screen {
    #[default]
    List,
    Detail {
        pr_id: u64,
        tab: DetailTab,
    },
}

use crate::{
    app::App,
    tui::{
        components::{diff_viewer::DiffViewer, search_input::SearchInput},
        screens::pr_detail::tabs::{DetailTab, commits::CommitList},
    },
};

impl App {
    pub(super) fn open_pr(&mut self, pr_id: u64) {
        self.state.screen = Screen::Detail {
            pr_id,
            tab: DetailTab::default(),
        };
        self.state.ui.list.search = SearchInput::default();
        self.state.ui.detail.diff = DiffViewer::default();
        self.state.ui.detail.commits = CommitList::default();
        self.state.ui.detail.description.scroll = 0;
        self.state.ui.detail.overview.timeline.scroll = 0;

        let pr_data = self.state.store.cache.details.entry(pr_id).or_default();
        let load_commits = pr_data.commits.start_loading();
        let load_diff = pr_data.diff.start_loading();
        let load_builds = pr_data.builds.start_loading();
        let load_activity = pr_data.activity.start_loading();
        let load_mergeability = pr_data.mergeability.start_loading();
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

use crate::{
    app::{
        App,
        action::CommitsAction,
        state::{DiffViewState, LoadState, Screen},
    },
    domain::commit::Commit,
};

impl App {
    pub(super) fn apply_commits(&mut self, action: CommitsAction) {
        match action {
            CommitsAction::MoveSelection(delta) => self.commits_move_selection(delta),
            CommitsAction::Open => self.commits_open_selected(),
            CommitsAction::Back => self.state.ui.commits.open_commit = None,
            CommitsAction::StepCommit(delta) => {
                let before = self.state.ui.commits.selected;
                self.commits_move_selection(delta);
                if self.state.ui.commits.selected != before {
                    self.commits_open_selected();
                }
            }
        }
    }

    fn filtered_commits(&self) -> Vec<&Commit> {
        let Screen::Detail { pr_id, .. } = self.state.screen else {
            return Vec::new();
        };
        match self.state.cache.details.get(&pr_id).map(|d| &d.commits) {
            Some(LoadState::Loaded(commits)) => {
                self.state.ui.commits.search.filter_commits(commits)
            }
            _ => Vec::new(),
        }
    }

    fn commits_move_selection(&mut self, delta: i16) {
        let len = self.filtered_commits().len();
        self.state.ui.commits.selected =
            super::step_index(self.state.ui.commits.selected, delta, len);
    }

    fn commits_open_selected(&mut self) {
        let Some(oid) = self
            .filtered_commits()
            .get(self.state.ui.commits.selected)
            .map(|c| c.oid.clone())
        else {
            return;
        };
        self.state.ui.commits.open_commit = Some(oid.clone());
        self.state.ui.commits.diff = DiffViewState::default();
        self.ensure_commit_diff(oid);
    }

    fn ensure_commit_diff(&mut self, oid: String) {
        let Screen::Detail { pr_id, .. } = self.state.screen else {
            return;
        };
        let pr_data = self.state.cache.details.entry(pr_id).or_default();
        let should_load = pr_data
            .commit_diffs
            .entry(oid.clone())
            .or_insert(LoadState::NotRequested)
            .start_loading();
        if should_load {
            self.spawn_load_commit_diff(pr_id, oid);
        }
    }
}

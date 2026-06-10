use crate::app::{
    App,
    action::CommitsAction,
    state::{DiffViewState, LoadState, Screen},
};

impl App {
    pub(super) fn apply_commits(&mut self, action: CommitsAction) {
        match action {
            CommitsAction::MoveSelection(delta) => self.commits_move_selection(delta),
            CommitsAction::Open => self.commits_open_selected(),
            CommitsAction::Back => self.state.ui.commits.drilled = None,
            CommitsAction::StepCommit(delta) => {
                self.commits_move_selection(delta);
                self.commits_open_selected();
            }
        }
    }

    fn commit_oids(&self) -> Vec<String> {
        let Screen::Detail { pr_id, .. } = self.state.screen else {
            return Vec::new();
        };
        self.state
            .cache
            .details
            .get(&pr_id)
            .and_then(|d| match &d.commits {
                LoadState::Loaded(commits) => {
                    Some(commits.iter().map(|c| c.oid.clone()).collect())
                }
                _ => None,
            })
            .unwrap_or_default()
    }

    fn commits_move_selection(&mut self, delta: i16) {
        let len = self.commit_oids().len();
        if len == 0 {
            self.state.ui.commits.selected = 0;
            return;
        }
        let last = (len - 1) as i64;
        let next = (self.state.ui.commits.selected as i64 + delta as i64).clamp(0, last);
        self.state.ui.commits.selected = next as usize;
    }

    fn commits_open_selected(&mut self) {
        let oids = self.commit_oids();
        let Some(oid) = oids.get(self.state.ui.commits.selected).cloned() else {
            return;
        };
        self.state.ui.commits.drilled = Some(oid.clone());
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

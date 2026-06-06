use crate::{
    app::{
        App,
        state::{DiffViewState, LoadState, Screen},
    },
    tui::{
        Action,
        pr_detail::{
            DetailTab,
            file_tree::{TreeRow, build_visible_rows},
        },
    },
};

impl App {
    pub(super) fn apply(&mut self, action: Action) {
        match action {
            Action::Quit => unreachable!("handled in run()"),
            Action::Back => {
                self.state.screen = Screen::List;
            }
            Action::NextPr => {
                let len = match &self.state.cache.prs {
                    LoadState::Loaded(prs) => prs.len(),
                    _ => 0,
                };
                let last = len.saturating_sub(1);
                self.state.ui.list_selected = (self.state.ui.list_selected + 1).min(last);
            }
            Action::PrevPr => {
                self.state.ui.list_selected = self.state.ui.list_selected.saturating_sub(1);
            }
            Action::NextTab => {
                if let Screen::Detail { tab, .. } = &mut self.state.screen {
                    *tab = tab.next();
                }
            }
            Action::PrevTab => {
                if let Screen::Detail { tab, .. } = &mut self.state.screen {
                    *tab = tab.prev();
                }
            }
            Action::SelectTab(new_tab) => {
                if let Screen::Detail { tab, .. } = &mut self.state.screen {
                    *tab = new_tab;
                }
            }
            Action::OpenPr(pr_id) => {
                self.state.screen = Screen::Detail {
                    pr_id,
                    tab: DetailTab::default(),
                };
                self.state.ui.diff = DiffViewState::default();
                self.state.ui.description_expanded = false;
                self.state.ui.description_scroll = 0;
                let (load_commits, load_diff) = {
                    let pr_data = self.state.cache.details.entry(pr_id).or_default();
                    let load_commits = matches!(pr_data.commits, LoadState::NotRequested);
                    let load_diff = matches!(pr_data.diff, LoadState::NotRequested);
                    if load_commits {
                        pr_data.commits = LoadState::Loading;
                    }
                    if load_diff {
                        pr_data.diff = LoadState::Loading;
                    }
                    (load_commits, load_diff)
                };
                if load_commits {
                    self.spawn_load_commits(pr_id);
                }
                if load_diff {
                    self.spawn_load_diff(pr_id);
                }
            }
            Action::PrsLoaded(prs) => {
                self.state.cache.prs = LoadState::Loaded(prs);
            }
            Action::CommitsLoaded(pr_id, commits) => {
                let pr_data = self.state.cache.details.entry(pr_id).or_default();
                pr_data.commits = LoadState::Loaded(commits);
            }
            Action::DiffLoaded(pr_id, diff) => {
                let pr_data = self.state.cache.details.entry(pr_id).or_default();
                pr_data.diff = LoadState::Loaded(diff);
            }
            Action::DiffCursorDown => {
                let rows = self.current_visible_rows();
                let last = rows.len().saturating_sub(1);
                let new_cursor = (self.state.ui.diff.cursor + 1).min(last);
                self.state.ui.diff.cursor = new_cursor;
                if let Some(TreeRow::File { file_index, .. }) = rows.get(new_cursor) {
                    self.state.ui.diff.focused_file = *file_index;
                }
            }
            Action::DiffCursorUp => {
                let new_cursor = self.state.ui.diff.cursor.saturating_sub(1);
                self.state.ui.diff.cursor = new_cursor;
                let rows = self.current_visible_rows();
                if let Some(TreeRow::File { file_index, .. }) = rows.get(new_cursor) {
                    self.state.ui.diff.focused_file = *file_index;
                }
            }
            Action::DiffToggleAtCursor => {
                let rows = self.current_visible_rows();
                if let Some(row) = rows.get(self.state.ui.diff.cursor) {
                    match row {
                        TreeRow::Dir {
                            path, expanded, ..
                        } => {
                            if *expanded {
                                self.state.ui.diff.collapsed.insert(path.clone());
                            } else {
                                self.state.ui.diff.collapsed.remove(path);
                            }
                        }
                        TreeRow::File { file_index, .. } => {
                            self.state.ui.diff.focused_file = *file_index;
                        }
                    }
                }
            }
            Action::DiffCollapseAtCursor => {
                let rows = self.current_visible_rows();
                if let Some(TreeRow::Dir { path, .. }) = rows.get(self.state.ui.diff.cursor) {
                    self.state.ui.diff.collapsed.insert(path.clone());
                }
            }
            Action::DiffExpandAtCursor => {
                let rows = self.current_visible_rows();
                if let Some(TreeRow::Dir { path, .. }) = rows.get(self.state.ui.diff.cursor) {
                    self.state.ui.diff.collapsed.remove(path);
                }
            }
            Action::ToggleDescription => {
                self.state.ui.description_expanded = !self.state.ui.description_expanded;
                self.state.ui.description_scroll = 0;
            }
            Action::DescriptionScrollDown => {
                self.state.ui.description_scroll =
                    self.state.ui.description_scroll.saturating_add(1);
            }
            Action::DescriptionScrollUp => {
                self.state.ui.description_scroll =
                    self.state.ui.description_scroll.saturating_sub(1);
            }
        }
    }

    fn current_visible_rows(&self) -> Vec<TreeRow> {
        let pr_id = match self.state.screen {
            Screen::Detail { pr_id, .. } => pr_id,
            _ => return Vec::new(),
        };
        let files = self
            .state
            .cache
            .details
            .get(&pr_id)
            .and_then(|d| match &d.diff {
                LoadState::Loaded(diff) => Some(&diff.files[..]),
                _ => None,
            });
        match files {
            Some(files) => build_visible_rows(files, &self.state.ui.diff.collapsed),
            None => Vec::new(),
        }
    }
}

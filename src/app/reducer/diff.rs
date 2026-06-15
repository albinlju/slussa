use crate::{
    app::{
        App,
        action::DiffAction,
        file_tree::{TreeRow, build_visible_rows},
        state::{DiffFocus, DiffViewState, LoadState, Screen},
    },
    domain::diff::FileDiff,
};

impl App {
    pub(super) fn apply_diff(&mut self, action: DiffAction) {
        match action {
            DiffAction::MoveCursor(delta) => self.diff_move_cursor(delta),
            DiffAction::ToggleAtCursor => self.diff_toggle_at_cursor(),
            DiffAction::CollapseAtCursor => self.diff_collapse_at_cursor(),
            DiffAction::ExpandAtCursor => self.diff_expand_at_cursor(),
            DiffAction::MovePaneCursor(delta) => self.diff_move_pane_cursor(delta),
            DiffAction::JumpMatch(delta) => self.diff_jump_match(delta),
            DiffAction::EnterPane => self.diff_enter_pane(),
            DiffAction::FocusTree => self.diff_view_mut().focus = DiffFocus::Tree,
        }
    }

    pub(super) fn diff_view(&self) -> &DiffViewState {
        self.state.ui.active_diff_view()
    }

    pub(super) fn diff_view_mut(&mut self) -> &mut DiffViewState {
        self.state.ui.active_diff_view_mut()
    }

    fn active_diff_files(&self) -> Option<&[FileDiff]> {
        let Screen::Detail { pr_id, .. } = self.state.screen else {
            return None;
        };
        let detail = self.state.cache.details.get(&pr_id)?;
        let state = match &self.state.ui.commits.open_commit {
            Some(oid) => detail.commit_diffs.get(oid)?,
            None => &detail.diff,
        };
        match state {
            LoadState::Loaded(diff) => Some(&diff.files),
            _ => None,
        }
    }

    pub(super) fn focus_file(&mut self, file_index: usize) {
        let view = self.diff_view_mut();
        if file_index != view.focused_file {
            view.focused_file = file_index;
            view.pane_scroll = 0;
            view.pane_cursor = 0;
        }
    }

    fn diff_enter_pane(&mut self) {
        match self.row_at_cursor() {
            Some(TreeRow::File { file_index, .. }) => {
                self.focus_file(file_index);
                self.diff_view_mut().focus = DiffFocus::Pane;
            }
            Some(TreeRow::Dir { .. }) => self.diff_toggle_at_cursor(),
            None => {}
        }
    }

    fn diff_move_cursor(&mut self, delta: i16) {
        let rows = self.current_visible_rows();
        if rows.is_empty() {
            return;
        }
        let new_cursor = super::step_index(self.diff_view().cursor, delta, rows.len());
        self.diff_view_mut().cursor = new_cursor;
        if let Some(TreeRow::File { file_index, .. }) = rows.get(new_cursor) {
            self.focus_file(*file_index);
        }
    }

    fn diff_jump_match(&mut self, delta: i16) {
        let view = self.diff_view();
        let matches = &view.pane_matches;
        if matches.is_empty() {
            return;
        }
        let cur = view.pane_cursor;
        let next = if delta >= 0 {
            matches.iter().copied().find(|&m| m > cur).unwrap_or(matches[0])
        } else {
            matches
                .iter()
                .copied()
                .rev()
                .find(|&m| m < cur)
                .unwrap_or(matches[matches.len() - 1])
        };
        self.diff_view_mut().pane_cursor = next;
    }

    fn diff_move_pane_cursor(&mut self, delta: i16) {
        self.diff_view_mut().pane_cursor = super::step_index(
            self.diff_view().pane_cursor,
            delta,
            self.diff_view().pane_item_count,
        );
    }

    fn diff_toggle_at_cursor(&mut self) {
        match self.row_at_cursor() {
            Some(TreeRow::Dir { expanded, .. }) => self.set_dir_collapsed_at_cursor(expanded),
            Some(TreeRow::File { file_index, .. }) => self.focus_file(file_index),
            None => {}
        }
    }

    fn diff_collapse_at_cursor(&mut self) {
        self.set_dir_collapsed_at_cursor(true);
    }

    fn diff_expand_at_cursor(&mut self) {
        self.set_dir_collapsed_at_cursor(false);
    }

    fn set_dir_collapsed_at_cursor(&mut self, collapsed: bool) {
        if let Some(TreeRow::Dir { path, .. }) = self.row_at_cursor() {
            let set = &mut self.diff_view_mut().collapsed;
            if collapsed {
                set.insert(path);
            } else {
                set.remove(&path);
            }
        }
    }

    fn row_at_cursor(&self) -> Option<TreeRow> {
        let rows = self.current_visible_rows();
        rows.get(self.diff_view().cursor).cloned()
    }

    pub(super) fn current_visible_rows(&self) -> Vec<TreeRow> {
        match self.active_diff_files() {
            Some(files) => build_visible_rows(
                files,
                &self.diff_view().collapsed,
                &self.diff_view().tree_search.query,
            ),
            None => Vec::new(),
        }
    }
}

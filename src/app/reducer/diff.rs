use crate::{
    app::{
        App,
        action::DiffAction,
        state::{DiffFocus, DiffViewState, LoadState, Screen},
    },
    domain::diff::FileDiff,
    tui::screens::pr_detail::file_tree::{TreeRow, build_visible_rows},
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

    /// The diff view the keyboard drives: the Commits drill-in when a commit
    /// is open, otherwise the Diff tab's own.
    pub(super) fn diff_view(&self) -> &DiffViewState {
        if self.state.ui.commits.drilled.is_some() {
            &self.state.ui.commits.diff
        } else {
            &self.state.ui.diff
        }
    }

    pub(super) fn diff_view_mut(&mut self) -> &mut DiffViewState {
        if self.state.ui.commits.drilled.is_some() {
            &mut self.state.ui.commits.diff
        } else {
            &mut self.state.ui.diff
        }
    }

    fn active_diff_files(&self) -> Option<&[FileDiff]> {
        let Screen::Detail { pr_id, .. } = self.state.screen else {
            return None;
        };
        let detail = self.state.cache.details.get(&pr_id)?;
        let state = match &self.state.ui.commits.drilled {
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
        let rows = self.current_visible_rows();
        match rows.get(self.diff_view().cursor) {
            Some(TreeRow::File { file_index, .. }) => {
                self.focus_file(*file_index);
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
        let last = (rows.len() - 1) as i64;
        let new_cursor = (self.diff_view().cursor as i64 + delta as i64).clamp(0, last) as usize;
        self.diff_view_mut().cursor = new_cursor;
        if let Some(TreeRow::File { file_index, .. }) = rows.get(new_cursor) {
            self.focus_file(*file_index);
        }
    }

    /// Step the pane cursor to the next (+1) / previous (-1) search match,
    /// wrapping around. No-op when there are no matches.
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
        let count = self.diff_view().pane_items;
        if count == 0 {
            self.diff_view_mut().pane_cursor = 0;
            return;
        }
        let last = (count - 1) as i64;
        let next = (self.diff_view().pane_cursor as i64 + delta as i64).clamp(0, last);
        self.diff_view_mut().pane_cursor = next as usize;
    }

    fn diff_toggle_at_cursor(&mut self) {
        let rows = self.current_visible_rows();
        let Some(row) = rows.get(self.diff_view().cursor) else {
            return;
        };
        match row {
            TreeRow::Dir { path, expanded, .. } => {
                let path = path.clone();
                let expanded = *expanded;
                let collapsed = &mut self.diff_view_mut().collapsed;
                if expanded {
                    collapsed.insert(path);
                } else {
                    collapsed.remove(&path);
                }
            }
            TreeRow::File { file_index, .. } => self.focus_file(*file_index),
        }
    }

    fn diff_collapse_at_cursor(&mut self) {
        let rows = self.current_visible_rows();
        if let Some(TreeRow::Dir { path, .. }) = rows.get(self.diff_view().cursor) {
            let path = path.clone();
            self.diff_view_mut().collapsed.insert(path);
        }
    }

    fn diff_expand_at_cursor(&mut self) {
        let rows = self.current_visible_rows();
        if let Some(TreeRow::Dir { path, .. }) = rows.get(self.diff_view().cursor) {
            let path = path.clone();
            self.diff_view_mut().collapsed.remove(&path);
        }
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

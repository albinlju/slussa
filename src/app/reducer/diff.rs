use crate::{
    app::{
        App,
        action::DiffAction,
        state::{DiffFocus, LoadState, Screen},
    },
    tui::screens::pr_detail::file_tree::{TreeRow, build_visible_rows},
};

impl App {
    pub(super) fn apply_diff(&mut self, action: DiffAction) {
        match action {
            DiffAction::MoveCursor(delta) => self.diff_move_cursor(delta),
            DiffAction::ToggleAtCursor => self.diff_toggle_at_cursor(),
            DiffAction::CollapseAtCursor => self.diff_collapse_at_cursor(),
            DiffAction::ExpandAtCursor => self.diff_expand_at_cursor(),
            DiffAction::PaneScroll(delta) => {
                self.state.ui.diff.pane_scroll = super::scroll(self.state.ui.diff.pane_scroll, delta);
            }
            DiffAction::EnterPane => self.diff_enter_pane(),
            DiffAction::FocusTree => self.state.ui.diff.focus = DiffFocus::Tree,
        }
    }

    fn diff_enter_pane(&mut self) {
        let rows = self.current_visible_rows();
        match rows.get(self.state.ui.diff.cursor) {
            Some(TreeRow::File { file_index, .. }) => {
                let file_index = *file_index;
                if file_index != self.state.ui.diff.focused_file {
                    self.state.ui.diff.focused_file = file_index;
                    self.state.ui.diff.pane_scroll = 0;
                }
                self.state.ui.diff.focus = DiffFocus::Pane;
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
        let new_cursor = (self.state.ui.diff.cursor as i64 + delta as i64).clamp(0, last) as usize;
        self.state.ui.diff.cursor = new_cursor;
        // Landing on a file row focuses it in the pane (and resets its scroll).
        if let Some(TreeRow::File { file_index, .. }) = rows.get(new_cursor)
            && *file_index != self.state.ui.diff.focused_file
        {
            self.state.ui.diff.focused_file = *file_index;
            self.state.ui.diff.pane_scroll = 0;
        }
    }

    fn diff_toggle_at_cursor(&mut self) {
        let rows = self.current_visible_rows();
        if let Some(row) = rows.get(self.state.ui.diff.cursor) {
            match row {
                TreeRow::Dir { path, expanded, .. } => {
                    if *expanded {
                        self.state.ui.diff.collapsed.insert(path.clone());
                    } else {
                        self.state.ui.diff.collapsed.remove(path);
                    }
                }
                TreeRow::File { file_index, .. } => {
                    if *file_index != self.state.ui.diff.focused_file {
                        self.state.ui.diff.focused_file = *file_index;
                        self.state.ui.diff.pane_scroll = 0;
                    }
                }
            }
        }
    }

    fn diff_collapse_at_cursor(&mut self) {
        let rows = self.current_visible_rows();
        if let Some(TreeRow::Dir { path, .. }) = rows.get(self.state.ui.diff.cursor) {
            self.state.ui.diff.collapsed.insert(path.clone());
        }
    }

    fn diff_expand_at_cursor(&mut self) {
        let rows = self.current_visible_rows();
        if let Some(TreeRow::Dir { path, .. }) = rows.get(self.state.ui.diff.cursor) {
            self.state.ui.diff.collapsed.remove(path);
        }
    }

    fn current_visible_rows(&self) -> Vec<TreeRow> {
        let Screen::Detail { pr_id, .. } = self.state.screen else {
            return Vec::new();
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

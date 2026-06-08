use crate::{
    app::{
        App,
        action::DiffAction,
        state::{LoadState, Screen},
    },
    tui::screens::pr_detail::file_tree::{TreeRow, build_visible_rows},
};

impl App {
    pub(super) fn apply_diff(&mut self, action: DiffAction) {
        match action {
            DiffAction::CursorDown => self.diff_cursor_down(),
            DiffAction::CursorUp => self.diff_cursor_up(),
            DiffAction::ToggleAtCursor => self.diff_toggle_at_cursor(),
            DiffAction::CollapseAtCursor => self.diff_collapse_at_cursor(),
            DiffAction::ExpandAtCursor => self.diff_expand_at_cursor(),
            DiffAction::PaneScrollDown => {
                self.state.ui.diff.pane_scroll = self.state.ui.diff.pane_scroll.saturating_add(1);
            }
            DiffAction::PaneScrollUp => {
                self.state.ui.diff.pane_scroll = self.state.ui.diff.pane_scroll.saturating_sub(1);
            }
        }
    }

    fn diff_cursor_down(&mut self) {
        let rows = self.current_visible_rows();
        let last = rows.len().saturating_sub(1);
        let new_cursor = (self.state.ui.diff.cursor + 1).min(last);
        self.state.ui.diff.cursor = new_cursor;
        if let Some(TreeRow::File { file_index, .. }) = rows.get(new_cursor)
            && *file_index != self.state.ui.diff.focused_file
        {
            self.state.ui.diff.focused_file = *file_index;
            self.state.ui.diff.pane_scroll = 0;
        }
    }

    fn diff_cursor_up(&mut self) {
        let new_cursor = self.state.ui.diff.cursor.saturating_sub(1);
        self.state.ui.diff.cursor = new_cursor;
        let rows = self.current_visible_rows();
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

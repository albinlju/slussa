pub(crate) mod file_tree;
use crate::{
    app::{
        action::{Action, DiffAction, Effect},
        reviews::{CommentAnchor, PendingComment},
        store::LoadState,
    },
    domain::{
        comment::CommentThread,
        diff::{Diff, FileDiff},
    },
    tui::{
        component::Component,
        components::{
            diff_viewer::file_tree::{TreeRow, build_visible_rows},
            search_input::{SearchInput, SearchKind},
        },
        screens::pr_detail::view::ThreadRef,
    },
};
use ratatui::{Frame, crossterm::event::KeyEvent, layout::Rect};
use std::collections::HashSet;

mod keys;
mod pane;
mod render;
mod tree;

pub struct DiffContext<'a> {
    pub diff: Option<&'a LoadState<Diff>>,
    pub threads: &'a [CommentThread],
    pub pending: &'a [PendingComment],
    pub author: &'a str,
}

#[derive(Debug, Default)]
pub struct DiffViewer {
    pub cursor: usize,
    pub focused_file: usize,
    pub collapsed: HashSet<String>,
    pub pane_scroll: u16,
    pub pane_cursor: usize,
    pub pane_viewport: u16,
    pub tree_viewport: u16,
    pub pane_item_count: usize,
    pub tree_search: SearchInput,
    pub pane_search: SearchInput,
    pub pane_matches: Vec<usize>,
    pub pane_anchor: Option<CommentAnchor>,
    pub pane_reply: Option<u64>,
    pub pane_thread: Option<ThreadRef>,
    /// Index (into `PendingReview::comments`) of the queued review comment the
    /// pane cursor is on, so `d` can remove it. `None` unless one is focused.
    pub pane_pending: Option<usize>,
    /// Root-comment ids of resolved threads the user has expanded (otherwise
    /// resolved threads render collapsed in the diff).
    pub expanded_threads: HashSet<u64>,
    pub focus: DiffFocus,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DiffFocus {
    #[default]
    Tree,
    Pane,
}

impl Component for DiffViewer {
    /// The files of the diff on screen, to move between.
    type Input<'a> = &'a [FileDiff];
    type View<'a> = DiffContext<'a>;
    type Message = DiffAction;
    fn handle_key(&self, key: KeyEvent, _: &&[FileDiff]) -> Option<Action> {
        keys::key_to_action(key.code, self).map(Action::Diff)
    }
    fn update(&mut self, action: DiffAction, files: &&[FileDiff]) -> Option<Effect> {
        match action {
            DiffAction::MoveCursor(delta) => self.diff_move_cursor(files, delta),
            DiffAction::ToggleAtCursor => self.diff_toggle_at_cursor(files),
            DiffAction::CollapseAtCursor => self.diff_collapse_at_cursor(files),
            DiffAction::MovePaneCursor(delta) => self.diff_move_pane_cursor(delta),
            DiffAction::JumpMatch(delta) => self.diff_jump_match(delta),
            DiffAction::EnterPane => self.diff_enter_pane(files),
            DiffAction::FocusTree => self.focus = DiffFocus::Tree,
            DiffAction::ToggleThreadExpand => self.diff_toggle_thread_expand(),
        }
        None
    }
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, ctx: &DiffContext<'_>) {
        render::render(
            frame,
            ctx.diff,
            ctx.threads,
            ctx.pending,
            self,
            ctx.author,
            area,
        );
    }
}

impl DiffViewer {
    pub(super) fn clear_targets(&mut self) {
        self.pane_anchor = None;
        self.pane_reply = None;
        self.pane_thread = None;
        self.pane_pending = None;
    }

    fn diff_toggle_thread_expand(&mut self) {
        let view = &mut *self;
        let Some(id) = view.pane_thread.as_ref().and_then(|t| t.comment_id) else {
            return;
        };
        if !view.expanded_threads.insert(id) {
            view.expanded_threads.remove(&id);
        }
    }

    pub(crate) const fn focus_file(&mut self, file_index: usize) {
        let view = &mut *self;
        if file_index != view.focused_file {
            view.focused_file = file_index;
            view.pane_scroll = 0;
            view.pane_cursor = 0;
        }
    }

    fn diff_enter_pane(&mut self, files: &[FileDiff]) {
        match self.row_at_cursor(files) {
            Some(TreeRow::File { file_index, .. }) => {
                self.focus_file(file_index);
                self.focus = DiffFocus::Pane;
            }
            Some(TreeRow::Dir { .. }) => self.diff_toggle_at_cursor(files),
            None => {}
        }
    }

    fn diff_move_cursor(&mut self, files: &[FileDiff], delta: i16) {
        let rows = self.current_visible_rows(files);
        if rows.is_empty() {
            return;
        }
        let new_cursor = crate::tui::component::step_index(self.cursor, delta, rows.len());
        self.cursor = new_cursor;
        if let Some(TreeRow::File { file_index, .. }) = rows.get(new_cursor) {
            self.focus_file(*file_index);
        }
    }

    fn diff_jump_match(&mut self, delta: i16) {
        let matches = &self.pane_matches;
        let cur = self.pane_cursor;
        // Past the last match the search wraps to the first, and the other way.
        let next = if delta >= 0 {
            matches
                .iter()
                .copied()
                .find(|&m| m > cur)
                .or_else(|| matches.first().copied())
        } else {
            matches
                .iter()
                .copied()
                .rev()
                .find(|&m| m < cur)
                .or_else(|| matches.last().copied())
        };
        if let Some(next) = next {
            self.pane_cursor = next;
        }
    }

    fn diff_move_pane_cursor(&mut self, delta: i16) {
        self.pane_cursor =
            crate::tui::component::step_index(self.pane_cursor, delta, self.pane_item_count);
    }

    fn diff_toggle_at_cursor(&mut self, files: &[FileDiff]) {
        match self.row_at_cursor(files) {
            Some(TreeRow::Dir { expanded, .. }) => {
                self.set_dir_collapsed_at_cursor(files, expanded);
            }
            Some(TreeRow::File { file_index, .. }) => self.focus_file(file_index),
            None => {}
        }
    }

    fn diff_collapse_at_cursor(&mut self, files: &[FileDiff]) {
        self.set_dir_collapsed_at_cursor(files, true);
    }

    fn set_dir_collapsed_at_cursor(&mut self, files: &[FileDiff], collapsed: bool) {
        if let Some(TreeRow::Dir { path, .. }) = self.row_at_cursor(files) {
            let set = &mut self.collapsed;
            if collapsed {
                set.insert(path);
            } else {
                set.remove(&path);
            }
        }
    }

    fn row_at_cursor(&self, files: &[FileDiff]) -> Option<TreeRow> {
        let rows = self.current_visible_rows(files);
        rows.get(self.cursor).cloned()
    }

    pub(crate) fn current_visible_rows(&self, files: &[FileDiff]) -> Vec<TreeRow> {
        build_visible_rows(files, &self.collapsed, &self.tree_search.query)
    }
}

impl DiffViewer {
    /// The search field in use and what it does: the tree filters its files,
    /// the pane finds text.
    pub const fn active_search(&self) -> (&SearchInput, SearchKind) {
        match self.focus {
            DiffFocus::Tree => (&self.tree_search, SearchKind::Filter),
            DiffFocus::Pane => (&self.pane_search, SearchKind::Find),
        }
    }
    pub fn update_search(&mut self, action: crate::app::action::SearchAction, files: &[FileDiff]) {
        use crate::app::action::SearchAction;
        let (search, kind) = match self.focus {
            DiffFocus::Tree => (&mut self.tree_search, SearchKind::Filter),
            DiffFocus::Pane => (&mut self.pane_search, SearchKind::Find),
        };
        search.update(action, &kind);
        if kind == SearchKind::Filter
            && !matches!(action, SearchAction::Open | SearchAction::Confirm)
        {
            self.cursor = 0;
            if let Some(TreeRow::File { file_index, .. }) = self.current_visible_rows(files).first()
            {
                self.focus_file(*file_index);
            }
        }
    }
}

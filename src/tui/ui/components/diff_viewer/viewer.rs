use super::{
    file_tree::{TreeRow, build_visible_rows},
    keys, render,
};
use crate::{
    domain::{
        comment::{CommentId, CommentKey, CommentThread},
        diff::{Diff, FileDiff},
        review::{CommentAnchor, PendingComment},
    },
    tui::{
        app::{effect::Effect, store::LoadState},
        ui::{
            action::{Action, DiffAction},
            component::Component,
            components::search_input::{SearchInput, SearchKind},
            screens::pr_detail::view::ThreadRef,
            widgets::comment_meta::Reading,
        },
    },
};
use ratatui::{Frame, crossterm::event::KeyEvent, layout::Rect};
use std::collections::HashSet;

pub struct DiffContext<'a> {
    pub diff: Option<&'a LoadState<Diff>>,
    pub threads: &'a [CommentThread],
    pub pending: &'a [PendingComment],
    pub reading: Reading<'a>,
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
    pub tree_search: SearchInput,
    pub pane_search: SearchInput,
    /// What the pane drew last, for the keys to act on.
    pub pane: PaneNav,
    /// Root-comment ids of resolved threads the reader has expanded (otherwise a
    /// resolved thread is one line).
    pub expanded_threads: HashSet<CommentId>,
    /// The long comments the reader has opened with their fold row.
    pub opened_comments: HashSet<CommentKey>,
    pub focus: DiffFocus,
}

/// What the keys can act on in the code pane, as it was last drawn. The
/// renderer replaces all of it at once, and empties it when no pane is drawn,
/// so the count, the matches and the focused item are always from the same
/// frame.
#[derive(Debug, Default)]
pub struct PaneNav {
    /// How many items the cursor can stand on.
    pub item_count: usize,
    /// The items the search matches, in order.
    pub matches: Vec<usize>,
    /// What the cursor is on, while the pane has focus.
    pub focused: Option<FocusedNav>,
}

/// The item under the pane cursor.
#[derive(Debug, Clone)]
pub struct FocusedNav {
    /// The line it is on or attached to: where a new comment would go.
    pub anchor: CommentAnchor,
    pub target: NavTarget,
}

#[cfg(test)]
impl FocusedNav {
    /// The cursor on `target`, on the first line of the fixture's file.
    pub fn on(target: NavTarget) -> Self {
        Self {
            anchor: CommentAnchor {
                revision: None,
                path: "src/main.rs".into(),
                line: 1,
                removed: false,
            },
            target,
        }
    }

    /// The cursor on a thread whose root comment is `id`.
    pub fn on_thread(id: CommentId) -> Self {
        Self::on(NavTarget::Thread(ThreadRef {
            handle: None,
            comment_id: Some(id),
            resolved: false,
        }))
    }
}

/// What kind of item the cursor is on. It is one of these: a thread is not
/// also a queued comment.
#[derive(Debug, Clone)]
pub enum NavTarget {
    Line,
    Thread(ThreadRef),
    /// The row that opens or folds one long comment of a thread. It stands for the
    /// thread when a reply or a resolve is asked for.
    Fold {
        thread: ThreadRef,
        key: CommentKey,
    },
    /// A queued review comment, by its index in the pending review.
    Pending(usize),
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
            DiffAction::ToggleCommentFold => self.diff_toggle_comment_fold(),
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
            ctx.reading,
            area,
        );
    }
}

impl DiffViewer {
    /// Where a new comment would go from the pane cursor.
    pub const fn focused_anchor(&self) -> Option<&CommentAnchor> {
        match &self.pane.focused {
            Some(focused) => Some(&focused.anchor),
            None => None,
        }
    }

    /// The thread the pane cursor is on.
    pub const fn focused_thread(&self) -> Option<&ThreadRef> {
        match &self.pane.focused {
            Some(FocusedNav {
                target: NavTarget::Thread(thread) | NavTarget::Fold { thread, .. },
                ..
            }) => Some(thread),
            Some(FocusedNav {
                target: NavTarget::Line | NavTarget::Pending(_),
                ..
            })
            | None => None,
        }
    }

    /// The comment a reply from the pane cursor would answer.
    pub const fn focused_reply(&self) -> Option<CommentId> {
        match self.focused_thread() {
            Some(thread) => thread.comment_id,
            None => None,
        }
    }

    /// The queued review comment the pane cursor is on, so `d` can remove it.
    pub const fn focused_pending(&self) -> Option<usize> {
        match &self.pane.focused {
            Some(FocusedNav {
                target: NavTarget::Pending(index),
                ..
            }) => Some(*index),
            Some(FocusedNav {
                target: NavTarget::Line | NavTarget::Thread(_) | NavTarget::Fold { .. },
                ..
            })
            | None => None,
        }
    }

    /// `space` on a resolved thread: one line, or the whole thread.
    fn diff_toggle_thread_expand(&mut self) {
        let Some(id) = self.focused_reply() else {
            return;
        };
        if !self.expanded_threads.insert(id) {
            self.expanded_threads.remove(&id);
        }
    }

    /// `space` on a fold row: open that comment, or fold it again.
    fn diff_toggle_comment_fold(&mut self) {
        let Some(key) = self.focused_fold() else {
            return;
        };
        if !self.opened_comments.insert(key) {
            self.opened_comments.remove(&key);
        }
    }

    /// The comment whose fold row the pane cursor is on.
    pub const fn focused_fold(&self) -> Option<CommentKey> {
        match &self.pane.focused {
            Some(FocusedNav {
                target: NavTarget::Fold { key, .. },
                ..
            }) => Some(*key),
            Some(FocusedNav {
                target: NavTarget::Line | NavTarget::Thread(_) | NavTarget::Pending(_),
                ..
            })
            | None => None,
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
        let new_cursor = crate::tui::ui::component::step_index(self.cursor, delta, rows.len());
        self.cursor = new_cursor;
        if let Some(TreeRow::File { file_index, .. }) = rows.get(new_cursor) {
            self.focus_file(*file_index);
        }
    }

    fn diff_jump_match(&mut self, delta: i16) {
        let matches = &self.pane.matches;
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
        // With no pane drawn there is nothing to move over, and the cursor
        // keeps its place for when the diff is back.
        if self.pane.item_count > 0 {
            self.pane_cursor = crate::tui::ui::component::step_index(
                self.pane_cursor,
                delta,
                self.pane.item_count,
            );
        }
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
    pub fn update_search(
        &mut self,
        action: crate::tui::ui::action::SearchAction,
        files: &[FileDiff],
    ) {
        use crate::tui::ui::action::SearchAction;
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

//! What the cursor in the code pane can stand on, as the pane drew it.

use super::NavTarget;
use crate::{
    domain::comment::{CommentId, CommentKey, ThreadHandle},
    tui::screens::pr_detail::view::ThreadRef,
};

/// A thread, which the cursor can be on or on a fold row of.
pub(super) struct ThreadNav {
    pub(super) line: usize,
    pub(super) removed: bool,
    pub(super) reply_to: Option<CommentId>,
    pub(super) handle: Option<ThreadHandle>,
    pub(super) resolved: bool,
}

pub(super) enum NavKind {
    Line {
        line: usize,
        removed: bool,
    },
    Thread(ThreadNav),
    /// The row that opens or folds one long comment of a thread. It stands for
    /// the thread when a reply or a resolve is asked for.
    Fold {
        thread: ThreadNav,
        key: CommentKey,
    },
    /// A queued (not-yet-posted) review comment — `index` into the pending review.
    Pending {
        line: usize,
        removed: bool,
        index: usize,
    },
}

pub(super) struct NavItem {
    pub(super) rendered_row: usize,
    pub(super) row_span: usize,
    pub(super) kind: NavKind,
}

impl NavItem {
    pub(super) const fn anchor(&self) -> (usize, bool) {
        match &self.kind {
            NavKind::Line { line, removed } | NavKind::Pending { line, removed, .. } => {
                (*line, *removed)
            }
            NavKind::Thread(thread) | NavKind::Fold { thread, .. } => (thread.line, thread.removed),
        }
    }

    pub(super) fn target(&self) -> NavTarget {
        match &self.kind {
            NavKind::Line { .. } => NavTarget::Line,
            NavKind::Thread(thread) | NavKind::Fold { thread, .. } => {
                NavTarget::Thread(ThreadRef {
                    handle: thread.handle.clone(),
                    comment_id: thread.reply_to,
                    resolved: thread.resolved,
                })
            }
            NavKind::Pending { index, .. } => NavTarget::Pending(*index),
        }
    }

    /// The comment whose fold row the cursor is on.
    pub(super) const fn fold(&self) -> Option<CommentKey> {
        match &self.kind {
            NavKind::Fold { key, .. } => Some(*key),
            NavKind::Line { .. } | NavKind::Thread(_) | NavKind::Pending { .. } => None,
        }
    }
}

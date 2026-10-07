//! What the cursor in the code pane can stand on, as the pane drew it.

use super::NavTarget;
use crate::{
    domain::comment::{CommentId, CommentKey, ThreadHandle},
    tui::ui::screens::pr_detail::view::ThreadRef,
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
    /// An agent's proposal — `index` among the PR's proposals.
    Proposal {
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
            NavKind::Line { line, removed }
            | NavKind::Pending { line, removed, .. }
            | NavKind::Proposal { line, removed, .. } => (*line, *removed),
            NavKind::Thread(thread) | NavKind::Fold { thread, .. } => (thread.line, thread.removed),
        }
    }

    pub(super) fn target(&self) -> NavTarget {
        let thread_ref = |thread: &ThreadNav| ThreadRef {
            handle: thread.handle.clone(),
            comment_id: thread.reply_to,
            resolved: thread.resolved,
        };
        match &self.kind {
            NavKind::Line { .. } => NavTarget::Line,
            NavKind::Thread(thread) => NavTarget::Thread(thread_ref(thread)),
            NavKind::Fold { thread, key } => NavTarget::Fold {
                thread: thread_ref(thread),
                key: *key,
            },
            NavKind::Pending { index, .. } => NavTarget::Pending(*index),
            NavKind::Proposal { index, .. } => NavTarget::Proposal(*index),
        }
    }
}

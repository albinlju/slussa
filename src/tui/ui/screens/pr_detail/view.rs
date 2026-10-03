use super::{PrDetailScreen, Surface};
use crate::{
    domain::{
        comment::{Comment, CommentId, CommentKey, CommentKind, ThreadHandle},
        commit::CommitOid,
        diff::FileDiff,
        pr::{Mergeability, PrId, PrStatus, PullRequest},
        review::{CommentTarget, ReviewVerdict},
    },
    tui::{
        app::{
            navigation::Screen,
            store::{LoadState, PrData, Store},
        },
        ui::{
            components::diff_viewer::{DiffFocus, DiffViewer},
            screens::pr_detail::tabs::DetailTab,
        },
    },
};

/// What the PR screen works from: the PR it shows, already looked up, with its
/// tab and what has been read for it. `new` is the only way to make one, so
/// nothing on the screen asks again whether there is a PR.
#[derive(Clone, Copy)]
pub struct DetailContext<'a> {
    pub store: &'a Store,
    pub pr_id: PrId,
    pub tab: DetailTab,
    pub pr: &'a PullRequest,
    pub data: Option<&'a PrData>,
    pub refreshing: bool,
}

impl<'a> DetailContext<'a> {
    /// `None` when the PR is not in the list. The list keeps the PR that is
    /// open (`App::adopt_group`), so that means there is nothing to show.
    pub fn new(store: &'a Store, pr_id: PrId, tab: DetailTab) -> Option<Self> {
        let pr = store.cache.prs.loaded()?.iter().find(|pr| pr.id == pr_id)?;
        Some(Self {
            store,
            pr_id,
            tab,
            pr,
            data: store.cache.details.get(&pr_id),
            refreshing: store.refreshing(Screen::Detail { pr_id, tab }),
        })
    }
}

pub fn diff_files<'a>(data: Option<&'a PrData>, commit: Option<&CommitOid>) -> &'a [FileDiff] {
    data.and_then(|data| data.diff_for(commit))
        .and_then(LoadState::loaded)
        .map_or(&[], |diff| diff.files.as_slice())
}

/// The screen's own state together with its context: what the read-only
/// questions of key handling, the footer and rendering are asked of.
pub struct DetailView<'a> {
    pub detail: &'a PrDetailScreen,
    pub store: &'a Store,
    pub pr_id: PrId,
    pub tab: DetailTab,
    pub pr: &'a PullRequest,
    pub data: Option<&'a PrData>,
    pub refreshing: bool,
}

impl<'a> DetailView<'a> {
    pub const fn new(detail: &'a PrDetailScreen, ctx: &DetailContext<'a>) -> Self {
        Self {
            detail,
            store: ctx.store,
            pr_id: ctx.pr_id,
            tab: ctx.tab,
            pr: ctx.pr,
            data: ctx.data,
            refreshing: ctx.refreshing,
        }
    }

    /// The files of the diff on screen: the PR's, or the open commit's.
    pub fn diff_files(&self) -> &'a [FileDiff] {
        diff_files(self.data, self.detail.commits.open_commit())
    }

    pub fn review_context(&self) -> super::dialogs::review::ReviewContext<'a> {
        super::dialogs::review::ReviewContext {
            options: self
                .review_verdicts()
                .into_iter()
                .map(|v| (v, self.verdict_disabled_reason(v)))
                .collect(),
            pending: self.pending_review(),
        }
    }

    pub fn pending_review(&self) -> Option<&'a crate::domain::review::PendingReview> {
        self.store.reviews.get(&self.pr_id)
    }

    pub fn viewing_own_pr(&self) -> bool {
        self.store.current_user.is(&self.pr.author.username)
    }

    /// Whether the PR is still actionable (open/draft, not already merged or
    /// declined). Gates both `m` (merge) and `x` (decline).
    pub const fn pr_is_open(&self) -> bool {
        matches!(self.pr.status, PrStatus::Open | PrStatus::Draft)
    }

    /// Why merging is unavailable, for the dimmed footer hint — `None` when it's
    /// offered. Unknown/loading mergeability still allows an attempt (server decides).
    pub fn merge_blocked_reason(&self) -> Option<&'static str> {
        match self.pr.status {
            PrStatus::Merged => return Some("merged"),
            PrStatus::Declined => return Some("declined"),
            PrStatus::Open | PrStatus::Draft => {}
        }
        (self
            .store
            .capabilities
            .supports(crate::domain::capabilities::Feature::Mergeability)
            && matches!(
                self.data.map(|d| &d.mergeability),
                Some(LoadState::Loaded(Mergeability::Conflicts(_)))
            ))
        .then_some("conflicts")
    }

    /// Whether the PR was closed without merging, so `x` reopens it.
    pub const fn pr_is_declined(&self) -> bool {
        matches!(self.pr.status, PrStatus::Declined)
    }

    /// Why declining is unavailable, for the dimmed footer hint — `None` when open.
    pub const fn decline_blocked_reason(&self) -> Option<&'static str> {
        match self.pr.status {
            PrStatus::Open | PrStatus::Draft => None,
            PrStatus::Merged => Some("merged"),
            PrStatus::Declined => Some("declined"),
        }
    }

    /// Whether to offer the `m` merge action.
    pub fn can_merge(&self) -> bool {
        !self.store.capabilities.merge_strategies.is_empty()
            && self.merge_blocked_reason().is_none()
    }

    /// Why a review verdict can't be submitted on this PR, for dimming it in the
    /// picker — you can't approve / request changes on your own PR (but you *can*
    /// leave a plain comment, which GitHub allows).
    pub fn verdict_disabled_reason(&self, verdict: ReviewVerdict) -> Option<&'static str> {
        (self.viewing_own_pr() && !self.store.capabilities.own_pr_verdicts().contains(&verdict))
            .then_some("your PR")
    }

    /// The loaded comment with this key in the current PR's activity, if any.
    pub fn find_comment(&self, key: CommentKey) -> Option<&'a Comment> {
        let activity = self.data?.activity.loaded()?;
        activity
            .comments
            .iter()
            .filter(|_| key.kind == CommentKind::Conversation)
            .chain(
                activity
                    .threads
                    .iter()
                    .filter(|thread| thread.kind() == key.kind)
                    .flat_map(|t| t.comments.iter()),
            )
            .find(|c| c.id == Some(key.id))
    }

    /// The comment a draft is a reply to or an edit of; none for a new comment.
    pub fn comment_under(&self, target: &CommentTarget) -> Option<&'a Comment> {
        match target {
            CommentTarget::Reply(id) => self
                .data?
                .activity
                .loaded()?
                .threads
                .iter()
                .find(|thread| thread.reply_to == Some(*id))?
                .comments
                .first(),
            CommentTarget::Edit(key) => self.find_comment(*key),
            CommentTarget::Line(_) | CommentTarget::Review { .. } | CommentTarget::Pr => None,
        }
    }

    /// The thread the cursor is on, for resolve/unresolve (`R`).
    pub const fn focused_thread(&self) -> Option<&'a ThreadRef> {
        match self.surface() {
            Surface::Overview => self.detail.overview.timeline.thread.as_ref(),
            Surface::Diff(viewer) | Surface::CommitDiff(viewer) => viewer.focused_thread(),
            Surface::Description | Surface::CommitList | Surface::Builds => None,
        }
    }

    /// What the content area shows.
    pub const fn surface(&self) -> Surface<'a> {
        self.detail.surface(self.tab)
    }

    /// The overview sub-selected comment, but only when it's the viewer's own
    /// (so it can be edited/deleted). `None` otherwise.
    pub fn editable_selected(&self) -> Option<CommentKey> {
        let key = self.detail.overview.timeline.selected?.key()?;
        let comment = self.find_comment(key)?;
        self.store
            .current_user
            .is(&comment.author.username)
            .then_some(key)
    }

    /// The review verdicts to offer in the menu — `Unapprove` only where the
    /// provider supports withdrawing approval.
    pub fn review_verdicts(&self) -> Vec<ReviewVerdict> {
        self.store.capabilities.review_verdicts().to_vec()
    }

    /// Target for a brand-new comment (`c`): a top-level PR comment in Overview,
    /// or the focused line in the Diff / commit pane.
    pub fn comment_target(&self) -> Option<CommentTarget> {
        match self.surface() {
            Surface::Overview => self
                .store
                .capabilities
                .supports(crate::domain::capabilities::Feature::PrComments)
                .then_some(CommentTarget::Pr),
            Surface::Diff(viewer) | Surface::CommitDiff(viewer) => self.pane_line_target(viewer),
            Surface::Description | Surface::CommitList | Surface::Builds => None,
        }
    }

    /// Target for a reply (`r`): the focused comment/thread. `None`
    /// when nothing repliable is focused.
    pub fn reply_target(&self) -> Option<CommentTarget> {
        if !self
            .store
            .capabilities
            .supports(crate::domain::capabilities::Feature::Replies)
        {
            return None;
        }
        match self.surface() {
            Surface::Overview => self
                .detail
                .overview
                .timeline
                .reply
                .map(CommentTarget::Reply),
            Surface::Diff(view) | Surface::CommitDiff(view) => (view.focus == DiffFocus::Pane)
                .then(|| view.focused_reply())
                .flatten()
                .map(CommentTarget::Reply),
            Surface::Description | Surface::CommitList | Surface::Builds => None,
        }
    }

    fn pane_line_target(&self, view: &DiffViewer) -> Option<CommentTarget> {
        if view.focus != DiffFocus::Pane {
            return None;
        }
        if let Some(parent) = view.focused_reply() {
            return self
                .store
                .capabilities
                .supports(crate::domain::capabilities::Feature::Replies)
                .then_some(CommentTarget::Reply(parent));
        }
        if !self
            .store
            .capabilities
            .supports(crate::domain::capabilities::Feature::InlineComments)
        {
            return None;
        }
        view.focused_anchor().cloned().map(CommentTarget::Line)
    }
}

/// The comment the overview sub-cursor points at within the focused block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommentRef {
    /// `None` when the provider gave no id (can't edit/delete it).
    pub id: Option<CommentId>,
    pub kind: CommentKind,
}

impl CommentRef {
    pub const fn new(id: Option<CommentId>, kind: CommentKind) -> Self {
        Self { id, kind }
    }

    /// What an edit or a delete needs; none for a comment without an id.
    pub fn key(self) -> Option<CommentKey> {
        Some(CommentKey {
            id: self.id?,
            kind: self.kind,
        })
    }
}

/// The thread the cursor is on.
#[derive(Debug, Clone)]
pub struct ThreadRef {
    /// What resolving it takes; none when it cannot be resolved.
    pub handle: Option<ThreadHandle>,
    /// Its root comment: what a reply hangs under and what folding remembers.
    pub comment_id: Option<CommentId>,
    pub resolved: bool,
}

impl DetailView<'_> {
    pub fn operation_pending(&self) -> bool {
        self.store.operations.contains_key(&self.pr_id)
    }
    pub fn error(&self) -> Option<&str> {
        self.store.errors.get(&self.pr_id).map(String::as_str)
    }
}

impl DetailView<'_> {
    /// Whether the provider supports what the action asks for. An action that
    /// needs no capability is always supported.
    pub fn supports_action(&self, action: crate::tui::ui::action::DetailAction) -> bool {
        use crate::{
            domain::capabilities::Feature as F,
            tui::ui::action::{DetailAction as A, MergeAction, NavAction, PrAction, ReviewAction},
        };
        let caps = &self.store.capabilities;
        match action {
            A::Pr(action) => match action {
                PrAction::OpenReviewPicker
                | PrAction::StartReview
                | PrAction::FinishReview
                | PrAction::AbandonReview
                | PrAction::RemovePendingComment => caps.reviews(),
                PrAction::OpenMergePicker => !caps.merge_strategies.is_empty(),
                PrAction::OpenDecline => caps.supports(F::ClosePr),
                PrAction::OpenReopen => caps.supports(F::ReopenPr),
                PrAction::OpenComment => {
                    self.detail.editor.has_draft() || self.comment_target().is_some()
                }
                PrAction::OpenReply => self.reply_target().is_some(),
                PrAction::EditComment => caps.supports(F::EditComments),
                PrAction::DeleteComment => caps.supports(F::DeleteComments),
                PrAction::ResolveThread => caps.supports(F::ResolveThreads),
            },
            A::Review(ReviewAction::Select) => caps.reviews(),
            A::Merge(MergeAction::Select) => !caps.merge_strategies.is_empty(),
            A::Nav(NavAction::SelectTab(tab)) => tab.supported_by(caps),
            A::Review(ReviewAction::Move(_) | ReviewAction::Preview | ReviewAction::Close)
            | A::Merge(MergeAction::Move(_) | MergeAction::Close)
            | A::Nav(
                NavAction::Back | NavAction::NextTab | NavAction::PrevTab | NavAction::ToggleHelp,
            )
            | A::Description(_)
            | A::BuildsScroll(_)
            | A::Timeline(_)
            | A::Error(_)
            | A::Confirm(_)
            | A::Editor(_) => true,
        }
    }
}

impl DetailView<'_> {
    pub const fn has_pr_link(&self) -> bool {
        self.pr.url.is_some()
    }
}

use super::PrDetailScreen;
use crate::{
    app::{
        navigation::Screen,
        reviews::CommentTarget,
        store::{LoadState, Store},
    },
    domain::{
        comment::Comment,
        pr::{Mergeability, PrStatus},
        review::ReviewVerdict,
    },
    tui::{components::diff_viewer::DiffFocus, screens::pr_detail::tabs::DetailTab},
};

pub struct DetailContext<'a> {
    pub store: &'a Store,
    pub screen: Screen,
    pub refreshing: bool,
}
pub struct DetailView<'a> {
    pub detail: &'a PrDetailScreen,
    pub store: &'a Store,
    pub screen: Screen,
    pub refreshing: bool,
}
impl<'a> DetailView<'a> {
    pub fn review_context(&self) -> super::dialogs::review::ReviewContext<'a> {
        let pr_id = match self.screen {
            Screen::Detail { pr_id, .. } => pr_id,
            Screen::List => 0,
        };
        super::dialogs::review::ReviewContext {
            options: self
                .review_verdicts()
                .into_iter()
                .map(|v| (v, self.verdict_disabled_reason(v, pr_id)))
                .collect(),
            pending: self.pending_review(),
        }
    }

    pub fn pending_review(&self) -> Option<&'a crate::app::reviews::PendingReview> {
        let Screen::Detail { pr_id, .. } = self.screen else {
            return None;
        };
        self.store.reviews.get(&pr_id)
    }

    pub fn viewing_own_pr(&self, pr_id: u64) -> bool {
        if self.store.current_user.is_empty() {
            return false;
        }
        let LoadState::Loaded(prs) = &self.store.cache.prs else {
            return false;
        };
        prs.iter()
            .any(|pr| pr.id == pr_id && pr.author.username == self.store.current_user)
    }

    fn pr_status(&self, pr_id: u64) -> Option<PrStatus> {
        let LoadState::Loaded(prs) = &self.store.cache.prs else {
            return None;
        };
        prs.iter()
            .find(|pr| pr.id == pr_id)
            .map(|pr| pr.status.clone())
    }

    /// Whether the PR is still actionable (open/draft, not already merged or
    /// declined). Gates both `m` (merge) and `x` (decline).
    pub fn pr_is_open(&self, pr_id: u64) -> bool {
        matches!(
            self.pr_status(pr_id),
            Some(PrStatus::Open | PrStatus::Draft)
        )
    }

    /// Why merging is unavailable, for the dimmed footer hint — `None` when it's
    /// offered. Unknown/loading mergeability still allows an attempt (server decides).
    pub fn merge_blocked_reason(&self, pr_id: u64) -> Option<&'static str> {
        match self.pr_status(pr_id) {
            Some(PrStatus::Merged) => return Some("merged"),
            Some(PrStatus::Declined) => return Some("declined"),
            Some(PrStatus::Open | PrStatus::Draft) => {}
            None => return Some("unavailable"),
        }
        (self
            .store
            .capabilities
            .supports(crate::domain::capabilities::Feature::Mergeability)
            && matches!(
                self.store
                    .cache
                    .details
                    .get(&pr_id)
                    .map(|d| &d.mergeability),
                Some(LoadState::Loaded(Mergeability::Conflicts))
            ))
        .then_some("conflicts")
    }

    /// Why declining is unavailable, for the dimmed footer hint — `None` when open.
    pub fn decline_blocked_reason(&self, pr_id: u64) -> Option<&'static str> {
        match self.pr_status(pr_id) {
            Some(PrStatus::Open | PrStatus::Draft) => None,
            Some(PrStatus::Merged) => Some("merged"),
            Some(PrStatus::Declined) => Some("declined"),
            None => Some("unavailable"),
        }
    }

    /// Whether to offer the `m` merge action.
    pub fn can_merge(&self, pr_id: u64) -> bool {
        !self.store.capabilities.merge_strategies.is_empty()
            && self.merge_blocked_reason(pr_id).is_none()
    }

    /// Why a review verdict can't be submitted on this PR, for dimming it in the
    /// picker — you can't approve / request changes on your own PR (but you *can*
    /// leave a plain comment, which GitHub allows).
    pub fn verdict_disabled_reason(
        &self,
        verdict: ReviewVerdict,
        pr_id: u64,
    ) -> Option<&'static str> {
        (self.viewing_own_pr(pr_id) && !self.store.capabilities.own_pr_verdicts.contains(&verdict))
            .then_some("your PR")
    }

    /// The loaded comment with `id` in the current PR's activity, if any.
    pub fn find_comment(&self, id: u64, review: bool) -> Option<&'a Comment> {
        let Screen::Detail { pr_id, .. } = self.screen else {
            return None;
        };
        let LoadState::Loaded(activity) = &self.store.cache.details.get(&pr_id)?.activity else {
            return None;
        };
        activity
            .comments
            .iter()
            .filter(|_| !review)
            .chain(
                activity
                    .threads
                    .iter()
                    .filter(|thread| thread.anchor.is_some() == review)
                    .flat_map(|t| t.comments.iter()),
            )
            .find(|c| c.id == Some(id))
    }

    /// The thread the cursor is on, for resolve/unresolve (`R`).
    pub fn focused_thread(&self) -> Option<&'a ThreadRef> {
        let Screen::Detail { tab, .. } = self.screen else {
            return None;
        };
        match tab {
            DetailTab::Overview => self.detail.overview.timeline.thread.as_ref(),
            DetailTab::Diff => self.detail.active_diff_view().pane_thread.as_ref(),
            DetailTab::Commits if self.detail.commits.open_commit.is_some() => {
                self.detail.active_diff_view().pane_thread.as_ref()
            }
            _ => None,
        }
    }

    /// The overview sub-selected comment, but only when it's the viewer's own
    /// (so it can be edited/deleted). `None` otherwise.
    pub fn editable_selected(&self) -> Option<CommentRef> {
        let sel = self.detail.overview.timeline.selected?;
        let id = sel.id?;
        let comment = self.find_comment(id, sel.review)?;
        (!self.store.current_user.is_empty() && comment.author.username == self.store.current_user)
            .then_some(sel)
    }

    /// The review verdicts to offer in the menu — `Unapprove` only where the
    /// provider supports withdrawing approval.
    pub fn review_verdicts(&self) -> Vec<ReviewVerdict> {
        if self.store.capabilities.reviews() {
            self.store.capabilities.review_verdicts.clone()
        } else {
            Vec::new()
        }
    }

    /// Target for a brand-new comment (`c`): a top-level PR comment in Overview,
    /// or the focused line in the Diff / commit pane.
    pub fn comment_target(&self) -> Option<CommentTarget> {
        let Screen::Detail { tab, .. } = self.screen else {
            return None;
        };
        match tab {
            DetailTab::Overview => self
                .store
                .capabilities
                .supports(crate::domain::capabilities::Feature::PrComments)
                .then_some(CommentTarget::Pr),
            DetailTab::Diff => self.pane_line_target(),
            DetailTab::Commits if self.detail.commits.open_commit.is_some() => {
                self.pane_line_target()
            }
            _ => None,
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
        let Screen::Detail { tab, .. } = self.screen else {
            return None;
        };
        match tab {
            DetailTab::Overview => self
                .detail
                .overview
                .timeline
                .reply
                .map(CommentTarget::Reply),
            DetailTab::Diff | DetailTab::Commits => {
                let view = self.detail.active_diff_view();
                (view.focus == DiffFocus::Pane)
                    .then_some(view.pane_reply)
                    .flatten()
                    .map(CommentTarget::Reply)
            }
            _ => None,
        }
    }

    fn pane_line_target(&self) -> Option<CommentTarget> {
        let view = self.detail.active_diff_view();
        if view.focus != DiffFocus::Pane {
            return None;
        }
        if let Some(parent) = view.pane_reply {
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
        view.pane_anchor.clone().map(CommentTarget::Line)
    }
}

/// The comment the overview sub-cursor points at within the focused block.
#[derive(Debug, Clone, Copy)]
pub struct CommentRef {
    /// `None` when the provider gave no id (can't edit/delete it).
    pub id: Option<u64>,
    /// A diff/line comment (vs a PR-level one) — GitHub edits them differently.
    pub review: bool,
}

/// Identity of a focused thread, for resolve/unresolve.
#[derive(Debug, Clone)]
pub struct ThreadRef {
    /// GitHub GraphQL thread id.
    pub node_id: Option<String>,
    /// Root comment id (Bitbucket toggles its state).
    pub comment_id: Option<u64>,
    pub resolved: bool,
}

impl DetailView<'_> {
    pub fn operation_pending(&self) -> bool {
        matches!(self.screen, Screen::Detail { pr_id, .. } if self.store.operations.contains_key(&pr_id))
    }
    pub fn error(&self) -> Option<&str> {
        let Screen::Detail { pr_id, .. } = self.screen else {
            return None;
        };
        self.store.errors.get(&pr_id).map(String::as_str)
    }
}

impl DetailView<'_> {
    pub fn supports_action(&self, action: crate::app::action::DetailAction) -> bool {
        use crate::{app::action::DetailAction as A, domain::capabilities::Feature as F};
        let caps = &self.store.capabilities;
        match action {
            A::OpenReviewPicker
            | A::ReviewSelect
            | A::StartReview
            | A::FinishReview
            | A::AbandonReview
            | A::RemovePendingComment => caps.reviews(),
            A::OpenMergePicker | A::MergeSelect => !caps.merge_strategies.is_empty(),
            A::OpenDecline => caps.supports(F::ClosePr),
            A::OpenComment => self.detail.editor.draft.is_some() || self.comment_target().is_some(),
            A::OpenReply => self.reply_target().is_some(),
            A::EditComment => caps.supports(F::EditComments),
            A::DeleteComment => caps.supports(F::DeleteComments),
            A::ResolveThread => caps.supports(F::ResolveThreads),
            A::SelectTab(tab) => tab.supported_by(caps),
            _ => true,
        }
    }
}

impl DetailView<'_> {
    pub fn has_pr_link(&self) -> bool {
        let Screen::Detail { pr_id, .. } = self.screen else {
            return false;
        };
        let LoadState::Loaded(prs) = &self.store.cache.prs else {
            return false;
        };
        prs.iter().any(|pr| pr.id == pr_id && pr.url.is_some())
    }
}

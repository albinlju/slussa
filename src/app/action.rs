use crate::{
    domain::{
        activity::Activity,
        ci::Build,
        commit::Commit,
        diff::Diff,
        pr::{MergeStatus, PullRequest},
    },
    tui::screens::pr_detail::tabs::DetailTab,
};

#[derive(Debug)]
pub enum Action {
    Quit,
    HelpScroll(i16),
    Paste(String),
    PrLink {
        pr_id: u64,
        kind: LinkAction,
    },
    LinkFinished(Result<String, String>),
    Command {
        pr_id: u64,
        command: Command,
    },
    Navigate(crate::app::navigation::Screen),
    /// Force a re-fetch of the active view now (`F`).
    Refresh,
    List(ListAction),
    Detail(DetailAction),
    Diff(DiffAction),
    Commits(CommitsAction),
    LoadCommitDiff {
        pr_id: u64,
        oid: String,
    },
    Search(SearchAction),
    Loaded(LoadedAction),
}

#[derive(Debug, Clone, Copy)]
pub enum SearchAction {
    Open,
    Type(char),
    Backspace,
    Confirm,
    Cancel,
}

#[derive(Debug, Clone, Copy)]
pub enum ListAction {
    ToggleHelp,
    ToggleSort,
    MoveSelection(i16),
    OpenPr(u64),
    OpenFilterPicker,
    CloseFilterPicker,
    FilterPickerNext,
    FilterPickerPrev,
    ApplyFilter,
}

#[derive(Debug, Clone, Copy)]
pub enum DetailAction {
    Back,
    NextTab,
    PrevTab,
    SelectTab(DetailTab),
    DescriptionScroll(i16),
    DescriptionHorizontal(i16),
    BuildsScroll(i16),
    OverviewMove(i16),
    OverviewScroll(i16),
    ToggleHelp,
    CloseConfirm,
    ConfirmMove(i16),
    SubmitConfirm,
    OpenReviewPicker,
    ReviewMove(i16),
    ReviewPreview,
    ReviewSelect,
    CloseReviewPicker,
    StartReview,
    FinishReview,
    AbandonReview,
    RemovePendingComment,
    OpenMergePicker,
    MergeMove(i16),
    MergeSelect,
    CloseMergePicker,
    OpenDecline,
    OpenComment,
    OpenReply,
    OverviewSubMove(i16),
    EditComment,
    DeleteComment,
    ResolveThread,
    ErrorScroll(i16),
    DismissError,
    CommentType(char),
    CommentBackspace,
    CommentDelete,
    CommentMove(i16),
    CommentVertical(i16),
    CommentHome,
    CommentEnd,
    CommentDiscard,
    CommentDiscardConfirm,
    CommentKeep,
    CommentSubmit,
    CommentCancel,
}

#[derive(Debug, Clone, Copy)]
pub enum DiffAction {
    MoveCursor(i16),
    ToggleAtCursor,
    CollapseAtCursor,
    MovePaneCursor(i16),
    JumpMatch(i16),
    EnterPane,
    FocusTree,
    ToggleThreadExpand,
}

#[derive(Debug, Clone, Copy)]
pub enum CommitsAction {
    MoveSelection(i16),
    Open,
    Back,
    StepCommit(i16),
}

#[derive(Debug)]
pub enum LoadedAction {
    ReviewFailed {
        pr_id: u64,
        posted_comments: usize,
        submitted_summary: Option<String>,
        message: String,
    },
    Prs(Result<Vec<PullRequest>, String>),
    Commits(u64, Result<Vec<Commit>, String>),
    Diff(u64, Result<Diff, String>),
    Builds(u64, Result<Vec<Build>, String>),
    Activity(u64, Result<Activity, String>),
    Mergeability(u64, Result<MergeStatus, String>),
    Merged(u64, Result<(), String>),
    Declined(u64, Result<(), String>),
    CommitDiff(u64, String, Result<Diff, String>),
    Commented(u64, Result<(), String>),
}

/// Fully resolved user intent; no dialog or editor state is read by the app.
#[derive(Debug)]
pub enum Command {
    StartReview,
    AbandonReview,
    RemovePendingComment(usize),
    SubmitComment {
        target: crate::app::reviews::CommentTarget,
        text: String,
    },
    SubmitReview {
        verdict: crate::domain::review::ReviewVerdict,
        body: String,
    },
    Merge(crate::domain::pr::MergeStrategy),
    Decline,
    DeleteComment {
        id: u64,
        review: bool,
    },
    ResolveThread {
        node_id: Option<String>,
        comment_id: Option<u64>,
        resolved: bool,
    },
    DismissError,
}

impl Command {
    pub fn supported_by(&self, caps: &crate::domain::capabilities::Capabilities) -> bool {
        use crate::{app::reviews::CommentTarget, domain::capabilities::Feature};
        match self {
            Self::DismissError => true,
            Self::StartReview | Self::AbandonReview | Self::RemovePendingComment(_) => {
                caps.reviews()
            }
            Self::SubmitReview { verdict, .. } => caps.can_submit_verdict(*verdict, false),
            Self::SubmitComment { target, .. } => match target {
                CommentTarget::Pr => caps.supports(Feature::PrComments),
                CommentTarget::Line(_) => caps.supports(Feature::InlineComments),
                CommentTarget::Reply(_) => caps.supports(Feature::Replies),
                CommentTarget::Edit { .. } => caps.supports(Feature::EditComments),
                CommentTarget::Review { verdict } => caps.can_submit_verdict(*verdict, false),
            },
            Self::Merge(strategy) => caps.merge_strategies.contains(strategy),
            Self::Decline => caps.supports(Feature::ClosePr),
            Self::DeleteComment { .. } => caps.supports(Feature::DeleteComments),
            Self::ResolveThread { .. } => caps.supports(Feature::ResolveThreads),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkAction {
    Open,
    Copy,
}

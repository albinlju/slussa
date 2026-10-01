use crate::{
    app::store::{FetchKey, WriteTicket},
    domain::{
        activity::Activity,
        ci::Build,
        commit::Commit,
        diff::Diff,
        pr::{Mergeability, PrBatch, PrGroup, PrInfo},
    },
    providers::{FetchError, ReviewError},
    tui::screens::pr_detail::tabs::DetailTab,
};

/// Input for the UI: what a key or a paste becomes. A component consumes its
/// own kind in `update` and never sees another's. `Effect` is here for the few
/// keys that ask the application for work directly (`q`, `F`, `o`, `y`).
#[derive(Debug)]
pub enum Action {
    HelpScroll(i16),
    Paste(String),
    List(ListAction),
    Detail(DetailAction),
    Diff(DiffAction),
    Commits(CommitsAction),
    Search(SearchAction),
    Effect(Effect),
}

/// Work for the application, returned by a component's `update`. Only these
/// reach `App`: a message a component handles itself cannot be one.
#[derive(Debug)]
pub enum Effect {
    Quit,
    Navigate(crate::app::navigation::Screen),
    /// Force a re-fetch of the active view now (`F`).
    Refresh,
    OpenPr(u64),
    /// Read the next older page of each closed group the view shows.
    LoadOlder,
    /// The list shows another view, which may need PRs not read yet.
    LoadView,
    LoadCommitDiff {
        pr_id: u64,
        oid: String,
    },
    PrLink {
        pr_id: u64,
        kind: LinkAction,
    },
    Command {
        pr_id: u64,
        command: Command,
    },
    /// Close the error shown on this PR.
    DismissError {
        pr_id: u64,
    },
}

/// What work that ran off the UI thread sends back.
#[derive(Debug)]
pub enum TaskResult {
    Read(Read),
    /// A write is over. The ticket names the PR and the operation it was.
    Written {
        ticket: WriteTicket,
        result: Result<(), WriteError>,
    },
    LinkFinished(Result<String, String>),
}

impl Action {
    /// A key typed or a cursor moved in the comment editor: frequent, and only
    /// the text of the open draft can change. A paste is one action and is not
    /// counted, so pasted text is on disk at once.
    pub const fn is_editor_keystroke(&self) -> bool {
        match self {
            Self::Detail(DetailAction::Editor(action)) => action.is_keystroke(),
            Self::Detail(_)
            | Self::HelpScroll(_)
            | Self::Paste(_)
            | Self::List(_)
            | Self::Diff(_)
            | Self::Commits(_)
            | Self::Search(_)
            | Self::Effect(_) => false,
        }
    }
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
    LoadOlder,
    MoveSelection(i16),
    OpenPr(u64),
    OpenFilterPicker,
    CloseFilterPicker,
    FilterPickerNext,
    FilterPickerPrev,
    ApplyFilter,
}

/// A message for the PR screen, grouped by the part that handles it, so each
/// part is handed only its own kind and matches it in full.
#[derive(Debug, Clone, Copy)]
pub enum DetailAction {
    Nav(NavAction),
    Description(DescriptionAction),
    BuildsScroll(i16),
    Timeline(TimelineAction),
    Error(ErrorAction),
    Confirm(ConfirmAction),
    Review(ReviewAction),
    Merge(MergeAction),
    Editor(EditorAction),
    Pr(PrAction),
}

impl DetailAction {
    /// Reading and moving around stay possible while a write for this PR is on
    /// its way; anything that could start or change one does not.
    pub const fn allowed_while_sending(self) -> bool {
        match self {
            Self::Nav(_)
            | Self::Description(_)
            | Self::BuildsScroll(_)
            | Self::Timeline(_)
            | Self::Error(_) => true,
            Self::Confirm(_) | Self::Review(_) | Self::Merge(_) | Self::Editor(_) | Self::Pr(_) => {
                false
            }
        }
    }
}

macro_rules! detail_action_from {
    ($($part:ident => $variant:ident),+ $(,)?) => {$(
        impl From<$part> for DetailAction {
            fn from(action: $part) -> Self {
                Self::$variant(action)
            }
        }
        impl From<$part> for Action {
            fn from(action: $part) -> Self {
                Self::Detail(DetailAction::$variant(action))
            }
        }
    )+};
}
detail_action_from! {
    NavAction => Nav,
    DescriptionAction => Description,
    TimelineAction => Timeline,
    ErrorAction => Error,
    ConfirmAction => Confirm,
    ReviewAction => Review,
    MergeAction => Merge,
    EditorAction => Editor,
    PrAction => Pr,
}

#[derive(Debug, Clone, Copy)]
pub enum NavAction {
    Back,
    NextTab,
    PrevTab,
    SelectTab(DetailTab),
    ToggleHelp,
}

#[derive(Debug, Clone, Copy)]
pub enum DescriptionAction {
    Scroll(i16),
    Horizontal(i16),
}

#[derive(Debug, Clone, Copy)]
pub enum TimelineAction {
    /// Step between comments and threads.
    Move(i16),
    Scroll(i16),
    /// Step between the comments of the focused thread.
    SubMove(i16),
}

#[derive(Debug, Clone, Copy)]
pub enum ErrorAction {
    Scroll(i16),
    Dismiss,
}

#[derive(Debug, Clone, Copy)]
pub enum ConfirmAction {
    Move(i16),
    Accept,
    Close,
}

#[derive(Debug, Clone, Copy)]
pub enum ReviewAction {
    Move(i16),
    /// Switch between the verdicts and the queued comments.
    Preview,
    Select,
    Close,
}

#[derive(Debug, Clone, Copy)]
pub enum MergeAction {
    Move(i16),
    Select,
    Close,
}

#[derive(Debug, Clone, Copy)]
pub enum EditorAction {
    Type(char),
    Backspace,
    Delete,
    Move(i16),
    Vertical(i16),
    Home,
    End,
    /// Ask whether to discard the draft.
    Discard,
    DiscardConfirm,
    Keep,
    Submit,
    /// Close the editor and keep the draft.
    Cancel,
}

impl EditorAction {
    pub const fn is_keystroke(self) -> bool {
        match self {
            Self::Type(_)
            | Self::Backspace
            | Self::Delete
            | Self::Move(_)
            | Self::Vertical(_)
            | Self::Home
            | Self::End => true,
            Self::Discard | Self::DiscardConfirm | Self::Keep | Self::Submit | Self::Cancel => {
                false
            }
        }
    }
}

/// What a key asks of the PR itself or of the thing under the cursor.
#[derive(Debug, Clone, Copy)]
pub enum PrAction {
    OpenReviewPicker,
    StartReview,
    FinishReview,
    AbandonReview,
    RemovePendingComment,
    OpenMergePicker,
    OpenDecline,
    OpenReopen,
    OpenComment,
    OpenReply,
    EditComment,
    DeleteComment,
    ResolveThread,
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

/// What a read brought back, and of which resource.
#[derive(Debug)]
pub enum Read {
    /// A group of PRs, or with `after` the page of a closed group that follows it.
    Prs {
        group: PrGroup,
        after: Option<String>,
        result: Result<PrBatch, FetchError>,
    },
    Commits(u64, Result<Vec<Commit>, FetchError>),
    Diff(u64, Result<Diff, FetchError>),
    Builds(u64, Result<Vec<Build>, FetchError>),
    Activity(u64, Result<Activity, FetchError>),
    Mergeability(u64, Result<Mergeability, FetchError>),
    Info(u64, Result<PrInfo, FetchError>),
    CommitDiff(u64, String, Result<Diff, FetchError>),
}

impl Read {
    /// The resource this is a read of: the key its fetch was registered under.
    pub fn key(&self) -> FetchKey {
        match self {
            Self::Prs { group, .. } => FetchKey::Prs(*group),
            Self::Commits(id, _) => FetchKey::Commits(*id),
            Self::Diff(id, _) => FetchKey::Diff(*id),
            Self::Builds(id, _) => FetchKey::Builds(*id),
            Self::Activity(id, _) => FetchKey::Activity(*id),
            Self::Mergeability(id, _) => FetchKey::Mergeability(*id),
            Self::Info(id, _) => FetchKey::Info(*id),
            Self::CommitDiff(id, oid, _) => FetchKey::CommitDiff(*id, oid.clone()),
        }
    }

    /// Why it failed, if it did.
    pub fn failure(&self) -> Option<&FetchError> {
        match self {
            Self::Prs { result, .. } => result.as_ref().err(),
            Self::Commits(_, result) => result.as_ref().err(),
            Self::Diff(_, result) | Self::CommitDiff(_, _, result) => result.as_ref().err(),
            Self::Builds(_, result) => result.as_ref().err(),
            Self::Activity(_, result) => result.as_ref().err(),
            Self::Mergeability(_, result) => result.as_ref().err(),
            Self::Info(_, result) => result.as_ref().err(),
        }
    }
}

/// Why a write did not go through, or not all of it.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error(transparent)]
    Failed(#[from] FetchError),
    /// A review sent as several requests, some of which arrived.
    #[error("review partially sent ({posted_comments} comments): {source}")]
    PartialReview {
        posted_comments: usize,
        /// The summary, when it was among what arrived.
        submitted_summary: Option<String>,
        source: FetchError,
    },
}

impl WriteError {
    /// A review's failure, with `summary` kept if it was among what arrived.
    pub fn from_review(error: ReviewError, summary: String) -> Self {
        match error {
            ReviewError::Failed(error) => Self::Failed(error),
            ReviewError::Partial {
                posted_comments,
                summary_posted,
                source,
            } => Self::PartialReview {
                posted_comments,
                submitted_summary: summary_posted.then_some(summary),
                source,
            },
        }
    }

    pub fn user_message(&self) -> String {
        match self {
            Self::Failed(error) => error.user_message(),
            Self::PartialReview {
                posted_comments,
                source,
                ..
            } => format!(
                "{}\n{posted_comments} line comments were sent; confirmed posts will be skipped on retry. Check the last attempted post before retrying.",
                source.user_message()
            ),
        }
    }

    /// Whether the server may have applied the write, or part of it.
    pub const fn may_have_reached_server(&self) -> bool {
        match self {
            Self::Failed(error) => error.may_have_reached_server(),
            Self::PartialReview { .. } => true,
        }
    }
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
    Reopen,
    DeleteComment(crate::domain::comment::CommentKey),
    ResolveThread {
        thread: crate::domain::comment::ThreadHandle,
        resolved: bool,
    },
}

impl Command {
    pub fn supported_by(&self, caps: &crate::domain::capabilities::Capabilities) -> bool {
        use crate::{app::reviews::CommentTarget, domain::capabilities::Feature};
        match self {
            Self::StartReview | Self::AbandonReview | Self::RemovePendingComment(_) => {
                caps.reviews()
            }
            Self::SubmitReview { verdict, .. } => caps.can_submit_verdict(*verdict, false),
            Self::SubmitComment { target, .. } => match target {
                CommentTarget::Pr => caps.supports(Feature::PrComments),
                CommentTarget::Line(_) => caps.supports(Feature::InlineComments),
                CommentTarget::Reply(_) => caps.supports(Feature::Replies),
                CommentTarget::Edit(_) => caps.supports(Feature::EditComments),
                CommentTarget::Review { verdict } => caps.can_submit_verdict(*verdict, false),
            },
            Self::Merge(strategy) => caps.merge_strategies.contains(strategy),
            Self::Decline => caps.supports(Feature::ClosePr),
            Self::Reopen => caps.supports(Feature::ReopenPr),
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

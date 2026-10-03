//! The UI's messages: what a key or a paste becomes, and the small enums a
//! component consumes in its own `update`.

use crate::{
    domain::pr::PrId,
    tui::{app::effect::Effect, ui::screens::pr_detail::tabs::DetailTab},
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
    OpenPr(PrId),
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
    /// Show all comments, only people's, or only the agents'.
    CycleFilter,
    /// Open or fold the long comment the cursor is on.
    ToggleFold,
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
    /// `space` on a fold row: open or fold that comment.
    ToggleCommentFold,
}

#[derive(Debug, Clone, Copy)]
pub enum CommitsAction {
    MoveSelection(i16),
    Open,
    Back,
    StepCommit(i16),
}

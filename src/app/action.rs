use crate::{
    app::state::DetailTab,
    domain::{
        activity::Activity,
        ci::Build,
        commit::Commit,
        diff::Diff,
        pr::{Mergeability, PullRequest},
    },
};

#[derive(Debug)]
pub enum Action {
    Quit,
    /// Force a re-fetch of the active view now (`F`).
    Refresh,
    List(ListAction),
    Detail(DetailAction),
    Diff(DiffAction),
    Commits(CommitsAction),
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
    OverviewMove(i16),
    ToggleHelp,
    CloseConfirm,
    ConfirmMove(i16),
    SubmitConfirm,
    OpenReviewPicker,
    ReviewMove(i16),
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
    DismissError,
    CommentType(char),
    CommentBackspace,
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
    Prs(Result<Vec<PullRequest>, String>),
    Commits(u64, Result<Vec<Commit>, String>),
    Diff(u64, Result<Diff, String>),
    Builds(u64, Result<Vec<Build>, String>),
    Activity(u64, Result<Activity, String>),
    Mergeability(u64, Result<Mergeability, String>),
    Merged(u64, Result<(), String>),
    Declined(u64, Result<(), String>),
    CommitDiff(u64, String, Result<Diff, String>),
    Commented(u64, Result<(), String>),
}

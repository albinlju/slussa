use crate::{
    app::state::{ConfirmKind, DetailTab},
    domain::{activity::Activity, ci::Build, commit::Commit, diff::Diff, pr::PullRequest},
};

#[derive(Debug)]
pub enum Action {
    Quit,
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
    OpenConfirm(ConfirmKind),
    CloseConfirm,
    ConfirmMove(i16),
    SubmitConfirm,
    OpenComment,
    OpenReply,
    OverviewSubMove(i16),
    EditComment,
    DeleteComment,
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
    ExpandAtCursor,
    MovePaneCursor(i16),
    JumpMatch(i16),
    EnterPane,
    FocusTree,
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
    CommitDiff(u64, String, Result<Diff, String>),
    Commented(u64, Result<(), String>),
}

use crate::{
    clients::ActivityBundle,
    domain::{ci::Build, commit::Commit, diff::Diff, pr::PullRequest},
    tui::screens::pr_detail::DetailTab,
};

#[derive(Debug)]
pub enum Action {
    Quit,
    List(ListAction),
    Detail(DetailAction),
    Diff(DiffAction),
    Commits(CommitsAction),
    Loaded(LoadedAction),
}

#[derive(Debug)]
pub enum ListAction {
    MoveSelection(i16),
    OpenPr(u64),
    OpenFilterPicker,
    CloseFilterPicker,
    FilterPickerNext,
    FilterPickerPrev,
    ApplyFilter,
}

#[derive(Debug)]
pub enum DetailAction {
    Back,
    NextTab,
    PrevTab,
    SelectTab(DetailTab),
    DescriptionScroll(i16),
    OverviewScroll(i16),
}

#[derive(Debug)]
pub enum DiffAction {
    MoveCursor(i16),
    ToggleAtCursor,
    CollapseAtCursor,
    ExpandAtCursor,
    MovePaneCursor(i16),
    /// Focus the file under the tree cursor in the pane; on a directory row,
    /// toggles expansion instead.
    EnterPane,
    FocusTree,
}

#[derive(Debug)]
pub enum CommitsAction {
    MoveSelection(i16),
    Open,
    Back,
    /// Step to the previous/next commit while drilled into a commit's diff.
    StepCommit(i16),
}

#[derive(Debug)]
pub enum LoadedAction {
    Prs(Result<Vec<PullRequest>, String>),
    Commits(u64, Result<Vec<Commit>, String>),
    Diff(u64, Result<Diff, String>),
    Builds(u64, Result<Vec<Build>, String>),
    Activity(u64, Result<ActivityBundle, String>),
    CommitDiff(u64, String, Result<Diff, String>),
}

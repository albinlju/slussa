use crate::{
    app::state::DetailTab,
    clients::ActivityBundle,
    domain::{ci::Build, commit::Commit, diff::Diff, pr::PullRequest},
};

#[derive(Debug)]
pub enum Action {
    Quit,
    List(ListAction),
    Detail(DetailAction),
    Diff(DiffAction),
    Commits(CommitsAction),
    /// Generic `/` search input, routed by the reducer to whichever view's
    /// `SearchState` is active (PR list, commit list, file tree, …).
    Search(SearchInput),
    Loaded(LoadedAction),
}

#[derive(Debug)]
pub enum SearchInput {
    /// `/` — enter typing mode for the active view's search.
    Open,
    Type(char),
    Backspace,
    /// Enter — close the prompt but keep the query (the pane's highlight stays).
    Confirm,
    /// Esc — leave search mode and clear the query.
    Cancel,
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
    /// Jump the pane cursor to the next (+1) / previous (-1) search match.
    JumpMatch(i16),
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

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
    Loaded(LoadedAction),
}

#[derive(Debug)]
pub enum ListAction {
    /// Move the selection by a signed row delta (negative = up). Serves j/k
    /// (±1) and Ctrl+D/U / PageDown/Up (±half page).
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
    /// Scroll by a signed line delta (negative = up). Lets one variant serve
    /// j/k (±1) and Ctrl+D/U (±half page).
    DescriptionScroll(i16),
    OverviewScroll(i16),
}

#[derive(Debug)]
pub enum DiffAction {
    /// Move the tree cursor by a signed row delta (negative = up). Serves j/k
    /// (±1) and Ctrl+D/U / PageDown/Up (±half page).
    MoveCursor(i16),
    ToggleAtCursor,
    CollapseAtCursor,
    ExpandAtCursor,
    /// Scroll the diff pane by a signed line delta (negative = up).
    PaneScroll(i16),
    /// Enter on a file row: focus that file and hand the keyboard to the pane.
    /// On a directory row, falls back to toggling expansion.
    EnterPane,
    /// Return focus from the pane back to the tree.
    FocusTree,
}

#[derive(Debug)]
pub enum LoadedAction {
    Prs(Result<Vec<PullRequest>, String>),
    Commits(u64, Result<Vec<Commit>, String>),
    Diff(u64, Result<Diff, String>),
    Builds(u64, Result<Vec<Build>, String>),
    Activity(u64, Result<ActivityBundle, String>),
}

use crate::{
    domain::{
        ci::Build,
        comment::{Comment, ReviewThread},
        commit::Commit,
        diff::Diff,
        event::TimelineEvent,
        pr::PullRequest,
    },
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
    NextPr,
    PrevPr,
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
    DescriptionScrollDown,
    DescriptionScrollUp,
    OverviewScrollDown,
    OverviewScrollUp,
}

#[derive(Debug)]
pub enum DiffAction {
    CursorDown,
    CursorUp,
    ToggleAtCursor,
    CollapseAtCursor,
    ExpandAtCursor,
    PaneScrollDown,
    PaneScrollUp,
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
    Comments(u64, Result<Vec<Comment>, String>),
    ReviewThreads(u64, Result<Vec<ReviewThread>, String>),
    Builds(u64, Result<Vec<Build>, String>),
    Events(u64, Result<Vec<TimelineEvent>, String>),
}

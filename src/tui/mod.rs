pub mod pr_detail;
pub mod pr_list;
pub mod theme;

use ratatui::{Frame, crossterm::event::KeyCode};

use crate::{
    app::state::{AppState, Screen},
    domain::{
        comment::{Comment, ReviewThread},
        commit::Commit,
        diff::Diff,
        pr::PullRequest,
    },
    tui::pr_detail::DetailTab,
};

#[derive(Debug)]
pub enum Action {
    Quit,
    Back,
    NextPr,
    PrevPr,
    NextTab,
    PrevTab,
    SelectTab(DetailTab),
    OpenPr(u64),
    PrsLoaded(Vec<PullRequest>),
    CommitsLoaded(u64, Vec<Commit>),
    DiffLoaded(u64, Diff),
    CommentsLoaded(u64, Vec<Comment>),
    ReviewThreadsLoaded(u64, Vec<ReviewThread>),
    DiffCursorDown,
    DiffCursorUp,
    DiffToggleAtCursor,
    DiffCollapseAtCursor,
    DiffExpandAtCursor,
    DescriptionScrollDown,
    DescriptionScrollUp,
    OverviewScrollDown,
    OverviewScrollUp,
    OpenFilterPicker,
    CloseFilterPicker,
    FilterPickerNext,
    FilterPickerPrev,
    ApplyFilter,
}

pub fn render(frame: &mut Frame, state: &mut AppState) {
    match state.screen {
        Screen::List => pr_list::render(frame, state, frame.area()),
        Screen::Detail { pr_id, tab } => {
            pr_detail::render(frame, state, pr_id, tab, frame.area())
        }
    }
}

pub fn key_to_action(state: &AppState, key: KeyCode) -> Option<Action> {
    match state.screen {
        Screen::List => pr_list::key_to_action(state, key),
        Screen::Detail { .. } => pr_detail::key_to_action(state, key),
    }
}

const SPINNER_FRAMES: &[&str] = &[
    "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏",
];

pub fn spinner_frame() -> &'static str {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let idx = (now / 100) as usize % SPINNER_FRAMES.len();
    SPINNER_FRAMES[idx]
}

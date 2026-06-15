use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::{
    app::{
        action::{Action, CommitsAction, DetailAction, DiffAction, SearchAction},
        state::{AppState, DetailTab, DiffFocus, DiffViewState, Screen},
    },
    tui::screens::half_page,
};

pub(in crate::tui) fn key_to_action(state: &AppState, key: KeyEvent) -> Option<Action> {
    if key.code == KeyCode::Char('q') {
        return Some(Action::Quit);
    }
    let Screen::Detail { tab, .. } = state.screen else {
        return None;
    };
    let viewing_commit = state.ui.commits.open_commit.is_some();
    let code = key.code;

    if code == KeyCode::Char('?') {
        return Some(Action::Detail(DetailAction::ToggleHelp));
    }
    if state.ui.help_open {
        return (code == KeyCode::Esc).then_some(Action::Detail(DetailAction::ToggleHelp));
    }

    escape_action(state, tab, viewing_commit, code)
        .or_else(|| tab_select_key(code))
        .or_else(|| tab_key(state, tab, viewing_commit, code))
}

fn escape_action(
    state: &AppState,
    tab: DetailTab,
    viewing_commit: bool,
    code: KeyCode,
) -> Option<Action> {
    if code != KeyCode::Esc {
        return None;
    }
    let view = state.ui.active_diff_view();
    let in_diff_pane = view.focus == DiffFocus::Pane
        && (tab == DetailTab::Diff || (tab == DetailTab::Commits && viewing_commit));
    if in_diff_pane {
        if !view.pane_search.query.is_empty() {
            return Some(Action::Search(SearchAction::Cancel));
        }
        return Some(Action::Diff(DiffAction::FocusTree));
    }
    if tab == DetailTab::Commits && viewing_commit {
        return Some(Action::Commits(CommitsAction::Back));
    }
    Some(Action::Detail(DetailAction::Back))
}

fn tab_select_key(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Tab => Some(Action::Detail(DetailAction::NextTab)),
        KeyCode::BackTab => Some(Action::Detail(DetailAction::PrevTab)),
        KeyCode::Char(c @ '1'..='5') => {
            let idx = (c as u8 - b'1') as usize;
            DetailTab::ALL
                .get(idx)
                .map(|&t| Action::Detail(DetailAction::SelectTab(t)))
        }
        _ => None,
    }
}

fn tab_key(
    state: &AppState,
    tab: DetailTab,
    viewing_commit: bool,
    code: KeyCode,
) -> Option<Action> {
    match tab {
        DetailTab::Diff => diff_nav_action(code, state.ui.active_diff_view()).map(Action::Diff),
        DetailTab::Commits if viewing_commit => match code {
            KeyCode::Char('[') => Some(Action::Commits(CommitsAction::StepCommit(-1))),
            KeyCode::Char(']') => Some(Action::Commits(CommitsAction::StepCommit(1))),
            _ => diff_nav_action(code, state.ui.active_diff_view()).map(Action::Diff),
        },
        DetailTab::Commits => scroll_delta(code, state.ui.commits.viewport)
            .map(|d| Action::Commits(CommitsAction::MoveSelection(d)))
            .or_else(|| match code {
                KeyCode::Enter => Some(Action::Commits(CommitsAction::Open)),
                _ => tab_nav(code),
            }),
        DetailTab::Overview => scroll_delta(code, state.ui.overview_viewport)
            .map(|d| Action::Detail(DetailAction::OverviewScroll(d)))
            .or_else(|| tab_nav(code)),
        DetailTab::Description => scroll_delta(code, state.ui.description_viewport)
            .map(|d| Action::Detail(DetailAction::DescriptionScroll(d)))
            .or_else(|| tab_nav(code)),
        DetailTab::Builds => tab_nav(code),
    }
}

fn scroll_delta(code: KeyCode, viewport: u16) -> Option<i16> {
    match code {
        KeyCode::Down | KeyCode::Char('j') => Some(1),
        KeyCode::Up | KeyCode::Char('k') => Some(-1),
        KeyCode::PageDown => Some(half_page(viewport)),
        KeyCode::PageUp => Some(-half_page(viewport)),
        _ => None,
    }
}

fn tab_nav(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Right | KeyCode::Char('l') => Some(Action::Detail(DetailAction::NextTab)),
        KeyCode::Left | KeyCode::Char('h') => Some(Action::Detail(DetailAction::PrevTab)),
        _ => None,
    }
}

fn diff_nav_action(code: KeyCode, view: &DiffViewState) -> Option<DiffAction> {
    match view.focus {
        DiffFocus::Tree => tree_key(code, view),
        DiffFocus::Pane => pane_key(code, view),
    }
}

fn tree_key(code: KeyCode, view: &DiffViewState) -> Option<DiffAction> {
    if let Some(delta) = scroll_delta(code, view.tree_viewport) {
        return Some(DiffAction::MoveCursor(delta));
    }
    match code {
        KeyCode::Enter => Some(DiffAction::EnterPane),
        KeyCode::Char(' ') => Some(DiffAction::ToggleAtCursor),
        KeyCode::Left | KeyCode::Char('h') => Some(DiffAction::CollapseAtCursor),
        KeyCode::Right | KeyCode::Char('l') => Some(DiffAction::ExpandAtCursor),
        _ => None,
    }
}

fn pane_key(code: KeyCode, view: &DiffViewState) -> Option<DiffAction> {
    if let Some(delta) = scroll_delta(code, view.pane_viewport) {
        return Some(DiffAction::MovePaneCursor(delta));
    }
    let searching = !view.pane_search.query.is_empty();
    match code {
        KeyCode::Char('n') if searching => Some(DiffAction::JumpMatch(1)),
        KeyCode::Char('N') if searching => Some(DiffAction::JumpMatch(-1)),
        KeyCode::Enter | KeyCode::Left | KeyCode::Char('h') => Some(DiffAction::FocusTree),
        _ => None,
    }
}

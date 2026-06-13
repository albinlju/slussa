use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
    let drilled = state.ui.commits.drilled.is_some();
    let diff_focus = state.ui.active_diff_view().focus;

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('d') => return half_page_scroll(state, tab, true),
            KeyCode::Char('u') => return half_page_scroll(state, tab, false),
            _ => {}
        }
    }

    if key.code == KeyCode::Esc {
        let in_diff_pane = diff_focus == DiffFocus::Pane
            && (tab == DetailTab::Diff || (tab == DetailTab::Commits && drilled));
        if in_diff_pane {
            if !state.ui.active_diff_view().pane_search.query.is_empty() {
                return Some(Action::Search(SearchAction::Cancel));
            }
            return Some(Action::Diff(DiffAction::FocusTree));
        }
        if tab == DetailTab::Commits && drilled {
            return Some(Action::Commits(CommitsAction::Back));
        }
        return Some(Action::Detail(DetailAction::Back));
    }

    match key.code {
        KeyCode::Tab => return Some(Action::Detail(DetailAction::NextTab)),
        KeyCode::BackTab => return Some(Action::Detail(DetailAction::PrevTab)),
        KeyCode::Char(c @ '1'..='5') => {
            let idx = (c as u8 - b'1') as usize;
            if let Some(&t) = DetailTab::ALL.get(idx) {
                return Some(Action::Detail(DetailAction::SelectTab(t)));
            }
        }
        _ => {}
    }

    match tab {
        DetailTab::Diff => diff_nav_action(key.code, state.ui.active_diff_view()).map(Action::Diff),
        DetailTab::Commits if drilled => match key.code {
            KeyCode::Char('[') => Some(Action::Commits(CommitsAction::StepCommit(-1))),
            KeyCode::Char(']') => Some(Action::Commits(CommitsAction::StepCommit(1))),
            _ => diff_nav_action(key.code, state.ui.active_diff_view()).map(Action::Diff),
        },
        DetailTab::Commits => scroll_delta(key.code, state.ui.commits.viewport)
            .map(|d| Action::Commits(CommitsAction::MoveSelection(d)))
            .or_else(|| match key.code {
                KeyCode::Enter => Some(Action::Commits(CommitsAction::Open)),
                _ => tab_nav(key.code),
            }),
        DetailTab::Overview => scroll_delta(key.code, state.ui.overview_viewport)
            .map(|d| Action::Detail(DetailAction::OverviewScroll(d)))
            .or_else(|| tab_nav(key.code)),
        DetailTab::Description => scroll_delta(key.code, state.ui.description_viewport)
            .map(|d| Action::Detail(DetailAction::DescriptionScroll(d)))
            .or_else(|| tab_nav(key.code)),
        DetailTab::Builds => tab_nav(key.code),
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
        DiffFocus::Tree => match code {
            KeyCode::Down | KeyCode::Char('j') => Some(DiffAction::MoveCursor(1)),
            KeyCode::Up | KeyCode::Char('k') => Some(DiffAction::MoveCursor(-1)),
            KeyCode::PageDown => Some(DiffAction::MoveCursor(half_page(view.tree_viewport))),
            KeyCode::PageUp => Some(DiffAction::MoveCursor(-half_page(view.tree_viewport))),
            KeyCode::Enter => Some(DiffAction::EnterPane),
            KeyCode::Char(' ') => Some(DiffAction::ToggleAtCursor),
            KeyCode::Left | KeyCode::Char('h') => Some(DiffAction::CollapseAtCursor),
            KeyCode::Right | KeyCode::Char('l') => Some(DiffAction::ExpandAtCursor),
            _ => None,
        },
        DiffFocus::Pane => match code {
            KeyCode::Down | KeyCode::Char('j') => Some(DiffAction::MovePaneCursor(1)),
            KeyCode::Up | KeyCode::Char('k') => Some(DiffAction::MovePaneCursor(-1)),
            KeyCode::PageDown => Some(DiffAction::MovePaneCursor(half_page(view.pane_viewport))),
            KeyCode::PageUp => Some(DiffAction::MovePaneCursor(-half_page(view.pane_viewport))),
            KeyCode::Char('n') if !view.pane_search.query.is_empty() => {
                Some(DiffAction::JumpMatch(1))
            }
            KeyCode::Char('N') if !view.pane_search.query.is_empty() => {
                Some(DiffAction::JumpMatch(-1))
            }
            KeyCode::Enter | KeyCode::Left | KeyCode::Char('h') => Some(DiffAction::FocusTree),
            _ => None,
        },
    }
}

fn diff_half_page(view: &DiffViewState, down: bool) -> DiffAction {
    let step = |v| if down { half_page(v) } else { -half_page(v) };
    match view.focus {
        DiffFocus::Pane => DiffAction::MovePaneCursor(step(view.pane_viewport)),
        DiffFocus::Tree => DiffAction::MoveCursor(step(view.tree_viewport)),
    }
}

fn half_page_scroll(state: &AppState, tab: DetailTab, down: bool) -> Option<Action> {
    let step = |viewport| {
        let h = half_page(viewport);
        if down { h } else { -h }
    };
    match tab {
        DetailTab::Description => Some(Action::Detail(DetailAction::DescriptionScroll(step(
            state.ui.description_viewport,
        )))),
        DetailTab::Overview => Some(Action::Detail(DetailAction::OverviewScroll(step(
            state.ui.overview_viewport,
        )))),
        DetailTab::Diff => Some(Action::Diff(diff_half_page(&state.ui.diff, down))),
        DetailTab::Commits if state.ui.commits.drilled.is_some() => {
            Some(Action::Diff(diff_half_page(&state.ui.commits.diff, down)))
        }
        DetailTab::Commits => Some(Action::Commits(CommitsAction::MoveSelection(step(
            state.ui.commits.viewport,
        )))),
        DetailTab::Builds => None,
    }
}

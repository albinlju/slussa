use crate::{
    app::action::DiffAction,
    tui::{components::diff_viewer::DiffFocus, screens::half_page},
};
use ratatui::crossterm::event::KeyCode;

pub(super) fn key_to_action(code: KeyCode, view: &super::DiffViewer) -> Option<DiffAction> {
    match view.focus {
        DiffFocus::Tree => tree_key(code, view),
        DiffFocus::Pane => pane_key(code, view),
    }
}

fn tree_key(code: KeyCode, view: &super::DiffViewer) -> Option<DiffAction> {
    if let Some(delta) = scroll_delta(code, view.tree_viewport) {
        return Some(DiffAction::MoveCursor(delta));
    }
    match code {
        // Enter steps right into the pane (a file) or drills into a folder. `h`
        // and `l` are not used here: on the PR page they always change tab.
        KeyCode::Enter | KeyCode::Right => Some(DiffAction::EnterPane),
        KeyCode::Char(' ') => Some(DiffAction::ToggleAtCursor),
        KeyCode::Left => Some(DiffAction::CollapseAtCursor),
        _ => None,
    }
}

fn pane_key(code: KeyCode, view: &super::DiffViewer) -> Option<DiffAction> {
    if let Some(delta) = scroll_delta(code, view.pane_viewport) {
        return Some(DiffAction::MovePaneCursor(delta));
    }
    let searching = !view.pane_search.query.is_empty();
    match code {
        KeyCode::Char('n') if searching => Some(DiffAction::JumpMatch(1)),
        KeyCode::Char('N') if searching => Some(DiffAction::JumpMatch(-1)),
        // Expand/collapse the focused resolved thread.
        KeyCode::Char(' ') if view.focused_thread().is_some() => {
            Some(DiffAction::ToggleThreadExpand)
        }
        KeyCode::Enter | KeyCode::Left => Some(DiffAction::FocusTree),
        _ => None,
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

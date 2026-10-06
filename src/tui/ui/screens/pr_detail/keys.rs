use super::bindings;
use crate::tui::{
    app::effect::Effect,
    ui::{
        action::{
            Action, BuildsAction, CommitsAction, DetailAction, DiffAction, NavAction, PrAction,
            SearchAction,
        },
        component::Component,
        components::diff_viewer::DiffFocus,
        screens::pr_detail::{
            Overlay, Surface,
            tabs::{DetailTab, builds::BuildsInput},
        },
    },
};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(in crate::tui::ui) fn key_to_action(
    state: &super::DetailView<'_>,
    key: KeyEvent,
) -> Option<Action> {
    // Errors capture input; navigation reads the message rather than acting on the PR.
    if state.error().is_some() {
        return state.detail.error.handle_key(key, &());
    }
    if state.operation_pending() && state.detail.editor.is_open() {
        return match key.code {
            KeyCode::Esc => Some(Action::from(NavAction::Back)),
            KeyCode::Char('q') => Some(Action::Effect(Effect::Quit)),
            _ => None,
        };
    }
    if state.detail.editor.is_open() {
        return state.detail.editor.handle_key(key, &());
    }
    if key.code == KeyCode::Char('q') {
        return Some(Action::Effect(Effect::Quit));
    }
    let (tab, surface) = (state.tab, state.surface());
    let code = key.code;

    match &state.detail.overlay {
        Some(Overlay::Confirm(dialog)) => return dialog.handle_key(key, &()),
        Some(Overlay::Review(dialog)) => return dialog.handle_key(key, &state.review_context()),
        Some(Overlay::Merge(dialog)) => {
            return dialog.handle_key(key, &state.store.capabilities.merge_strategies.as_slice());
        }
        Some(Overlay::Issues(dialog)) => {
            return dialog.handle_key(key, &state.issues_to_open().len());
        }
        Some(Overlay::Help(help)) => {
            return if matches!(code, KeyCode::Esc | KeyCode::Char('?')) {
                Some(Action::from(NavAction::ToggleHelp))
            } else {
                help.handle_key(key, &())
            };
        }
        None => {}
    }
    if code == KeyCode::Char('?') {
        return Some(Action::from(NavAction::ToggleHelp));
    }

    if let Some(action) = bindings::route(state, key) {
        return Some(action);
    }
    // Overview-only: step individual comments within the focused block (Ctrl-j/k).
    if tab == DetailTab::Overview
        && key.modifiers.contains(KeyModifiers::CONTROL)
        && let Some(action) = state.detail.overview.handle_key(key, &())
    {
        return Some(action);
    }

    escape_action(surface, code)
        .or_else(|| tab_select_key(code))
        .or_else(|| tab_key(state, surface, code))
        // `[`/`]` switch tabs, but only after tab_key so the commit-diff view
        // keeps them for stepping commits.
        .or_else(|| tab_bracket_key(code))
}

/// Esc steps out one level: the pane's search, the pane, the open commit, and
/// then the PR.
fn escape_action(surface: Surface<'_>, code: KeyCode) -> Option<Action> {
    if code != KeyCode::Esc {
        return None;
    }
    if let Some(view) = surface.diff_viewer()
        && view.focus == DiffFocus::Pane
    {
        if !view.pane_search.query.is_empty() {
            return Some(Action::Search(SearchAction::Cancel));
        }
        return Some(Action::Diff(DiffAction::FocusTree));
    }
    Some(match surface {
        Surface::CommitDiff(_) => Action::Commits(CommitsAction::Back),
        Surface::SinceDiff(_) => Action::from(PrAction::ToggleSince),
        Surface::BuildLog => Action::from(BuildsAction::Close),
        Surface::Description
        | Surface::Overview
        | Surface::Diff(_)
        | Surface::CommitList
        | Surface::Builds => Action::from(NavAction::Back),
    })
}

fn tab_select_key(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Char(c @ '1'..='5') => {
            let idx = (c as u8 - b'1') as usize;
            DetailTab::ALL
                .get(idx)
                .map(|&t| Action::Detail(DetailAction::Nav(NavAction::SelectTab(t))))
        }
        _ => None,
    }
}

fn tab_bracket_key(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Char('[') => Some(Action::from(NavAction::PrevTab)),
        KeyCode::Char(']') => Some(Action::from(NavAction::NextTab)),
        _ => None,
    }
}

fn tab_key(state: &super::DetailView<'_>, surface: Surface<'_>, code: KeyCode) -> Option<Action> {
    match surface {
        Surface::Diff(viewer) | Surface::SinceDiff(viewer) => viewer
            .handle_key(KeyEvent::new(code, KeyModifiers::NONE), &state.diff_files())
            .or_else(|| tab_letters(code)),
        Surface::CommitDiff(viewer) => match code {
            KeyCode::Char('[') => Some(Action::Commits(CommitsAction::StepCommit(-1))),
            KeyCode::Char(']') => Some(Action::Commits(CommitsAction::StepCommit(1))),
            _ => viewer
                .handle_key(KeyEvent::new(code, KeyModifiers::NONE), &state.diff_files())
                .or_else(|| tab_letters(code)),
        },
        Surface::CommitList => state
            .detail
            .commits
            .handle_key(
                KeyEvent::new(code, KeyModifiers::NONE),
                &super::tabs::commits::CommitInput::new(state.pr_id, state.data),
            )
            .or_else(|| tab_nav(code)),
        Surface::Overview => state
            .detail
            .overview
            .handle_key(KeyEvent::new(code, KeyModifiers::NONE), &())
            .or_else(|| tab_nav(code)),
        Surface::Description => state
            .detail
            .description
            .handle_key(KeyEvent::new(code, KeyModifiers::NONE), &())
            .or_else(|| tab_nav(code)),
        Surface::Builds | Surface::BuildLog => state
            .detail
            .builds
            .handle_key(
                KeyEvent::new(code, KeyModifiers::NONE),
                &BuildsInput::new(state.pr_id, state.data),
            )
            .or_else(|| tab_nav(code)),
    }
}

/// `h` and `l` change tab on every tab. The arrow keys do too, except in the
/// diff, where they move between the file tree and the code.
fn tab_letters(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Char('l') => Some(Action::from(NavAction::NextTab)),
        KeyCode::Char('h') => Some(Action::from(NavAction::PrevTab)),
        _ => None,
    }
}

fn tab_nav(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Right | KeyCode::Char('l') => Some(Action::from(NavAction::NextTab)),
        KeyCode::Left | KeyCode::Char('h') => Some(Action::from(NavAction::PrevTab)),
        _ => None,
    }
}

use crate::{
    app::{action::Action, navigation::Screen, state::AppState},
    tui::components::search_input::SearchInput,
};
use component::Component;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};
use screens::{pr_detail, pr_list};

pub mod component;
pub mod components;
pub mod format;
pub mod icons;
pub mod layout;
pub mod screens;
pub mod theme;
pub mod widgets;

pub fn render(frame: &mut Frame, state: &mut AppState) {
    match state.screen {
        Screen::List => state.ui.list.render(
            frame,
            frame.area(),
            &pr_list::ListContext {
                prs: &state.store.cache.prs,
                refreshing: state.ui.refreshing,
            },
        ),
        Screen::Detail { .. } => state.ui.detail.render(
            frame,
            frame.area(),
            &pr_detail::DetailContext {
                store: &state.store,
                screen: state.screen,
                refreshing: state.ui.refreshing,
            },
        ),
    }
}

pub fn key_to_action(state: &AppState, key: KeyEvent) -> Option<Action> {
    let key = normalize_key(key);

    if state.ui.detail.editor.draft.is_some() {
        return state.ui.detail.editor.handle_key(key, &());
    }

    if matches!(state.screen, Screen::Detail { .. }) && state.ui.detail.modal_open() {
        return state.ui.detail.handle_key(
            key,
            &pr_detail::DetailContext {
                store: &state.store,
                screen: state.screen,
                refreshing: state.ui.refreshing,
            },
        );
    }

    if let Some((search, highlight)) = active_search(state)
        && let Some(action) = search.handle_key(
            key,
            &components::search_input::SearchContext {
                highlight,
                matches: 0,
            },
        )
    {
        return Some(action);
    }

    match state.screen {
        Screen::List => state.ui.list.handle_key(
            key,
            &pr_list::ListContext {
                prs: &state.store.cache.prs,
                refreshing: state.ui.refreshing,
            },
        ),
        Screen::Detail { .. } => state.ui.detail.handle_key(
            key,
            &pr_detail::DetailContext {
                store: &state.store,
                screen: state.screen,
                refreshing: state.ui.refreshing,
            },
        ),
    }
}

fn normalize_key(mut key: KeyEvent) -> KeyEvent {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('d') => key.code = KeyCode::PageDown,
            KeyCode::Char('u') => key.code = KeyCode::PageUp,
            _ => return key,
        }
        key.modifiers.remove(KeyModifiers::CONTROL);
    }
    key
}

fn active_search(state: &AppState) -> Option<(&SearchInput, bool)> {
    match state.screen {
        Screen::List => {
            (!state.ui.list.filter_picker_open).then_some((&state.ui.list.search, false))
        }
        Screen::Detail { tab, .. } => state.ui.detail.active_search(tab),
    }
}

#[cfg(test)]
pub(crate) mod regression_tests;
#[derive(Debug, Default)]
pub struct Ui {
    pub list: crate::tui::screens::pr_list::PrListScreen,
    pub detail: crate::tui::screens::pr_detail::PrDetailScreen,
    pub refreshing: bool,
}

impl Ui {
    pub fn update(
        &mut self,
        action: Action,
        store: &crate::app::store::Store,
        screen: Screen,
    ) -> Option<Action> {
        match action {
            Action::List(action) => self.list.update(
                action,
                &pr_list::ListContext {
                    prs: &store.cache.prs,
                    refreshing: self.refreshing,
                },
            ),
            Action::Search(action) if screen == Screen::List => {
                if !self.list.filter_picker_open {
                    self.list.update_search(action);
                }
                None
            }
            Action::Detail(_) | Action::Diff(_) | Action::Commits(_) | Action::Search(_) => {
                self.detail.update_action(
                    action,
                    &pr_detail::DetailContext {
                        store,
                        screen,
                        refreshing: self.refreshing,
                    },
                )
            }
            effect => Some(effect),
        }
    }
}

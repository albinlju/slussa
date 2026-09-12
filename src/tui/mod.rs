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
                refreshing: state.store.refreshing(state.screen),
            },
        ),
        Screen::Detail { .. } => state.ui.detail.render(
            frame,
            frame.area(),
            &pr_detail::DetailContext {
                store: &state.store,
                screen: state.screen,
                refreshing: state.store.refreshing(state.screen),
            },
        ),
    }
}

pub fn key_to_action(state: &AppState, key: KeyEvent) -> Option<Action> {
    let key = normalize_key(key);

    if matches!(state.screen, Screen::Detail { .. })
        && state.ui.modal_open(&state.store, state.screen)
    {
        return state.ui.detail.handle_key(
            key,
            &pr_detail::DetailContext {
                store: &state.store,
                screen: state.screen,
                refreshing: state.store.refreshing(state.screen),
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
                refreshing: state.store.refreshing(state.screen),
            },
        ),
        Screen::Detail { .. } => state.ui.detail.handle_key(
            key,
            &pr_detail::DetailContext {
                store: &state.store,
                screen: state.screen,
                refreshing: state.store.refreshing(state.screen),
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
        Screen::List => (!state.ui.list.filter_picker_open && !state.ui.list.help_open)
            .then_some((&state.ui.list.search, false)),
        Screen::Detail { tab, .. } => state.ui.detail.active_search(tab),
    }
}

#[cfg(test)]
pub(crate) mod regression_tests;
#[derive(Debug, Default)]
pub struct Ui {
    pub list: crate::tui::screens::pr_list::PrListScreen,
    pub detail: crate::tui::screens::pr_detail::PrDetailScreen,
}

impl Ui {
    pub fn open_pr(&mut self, pr_id: u64) {
        self.list.search = SearchInput::default();
        self.detail.open(pr_id);
    }

    pub fn modal_open(&self, store: &crate::app::store::Store, screen: Screen) -> bool {
        match screen {
            Screen::List => self.list.filter_picker_open || self.list.help_open,
            Screen::Detail { pr_id, .. } => {
                self.detail.modal_open() || store.errors.contains_key(&pr_id)
            }
        }
    }

    pub fn update(
        &mut self,
        action: Action,
        store: &crate::app::store::Store,
        screen: Screen,
    ) -> Option<Action> {
        match action {
            Action::HelpScroll(delta) => {
                let (open, help) = match screen {
                    Screen::List => (self.list.help_open, &mut self.list.help),
                    Screen::Detail { .. } => (self.detail.help_open, &mut self.detail.help),
                };
                if open {
                    help.update(delta, &&[][..]);
                }
                None
            }
            Action::List(action) => self.list.update(
                action,
                &pr_list::ListContext {
                    prs: &store.cache.prs,
                    refreshing: store.refreshing(screen),
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
                        refreshing: store.refreshing(screen),
                    },
                )
            }
            effect => Some(effect),
        }
    }
}

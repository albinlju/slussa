use crate::{
    domain::pr::PrId,
    tui::{
        app::{effect::Effect, navigation::Screen, state::AppState},
        ui::{
            action::Action,
            components::search_input::{SearchInput, SearchKind},
        },
    },
};
use component::Component;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};
use screens::{pr_detail, pr_list};

pub mod action;
pub mod component;
pub mod components;
pub mod format;
pub mod icons;
pub mod layout;
pub mod screens;
pub mod theme;
pub mod widgets;

pub fn render(frame: &mut Frame<'_>, state: &mut AppState) {
    match state.screen {
        Screen::List => state.ui.list.render(
            frame,
            frame.area(),
            &pr_list::ListContext::from_store(&state.store, state.ui.list.filter, state.screen),
        ),
        Screen::Detail { pr_id, tab } => {
            if let Some(ctx) = pr_detail::DetailContext::new(&state.store, pr_id, tab) {
                state.ui.detail.render(frame, frame.area(), &ctx);
            }
        }
    }
    if state.store.refresh_failed(state.screen)
        && !state.ui.modal_open(&state.store, state.screen)
        && !active_search(state).is_some_and(|(search, _)| search.open)
    {
        let area = frame.area();
        if area.height > 0 {
            let footer =
                ratatui::layout::Rect::new(area.x, area.y + area.height - 1, area.width, 1);
            frame.render_widget(ratatui::widgets::Clear, footer);
            frame.render_widget(
                ratatui::widgets::Paragraph::new("Refresh failed · cached data · F retry")
                    .style(ratatui::style::Style::default().fg(theme::current().warning)),
                footer,
            );
        }
    }
    if state.store.draft_error.is_none()
        && !state.ui.modal_open(&state.store, state.screen)
        && !active_search(state).is_some_and(|(search, _)| search.open)
        && let Some(notice) = state
            .store
            .notice
            .as_ref()
            .filter(|notice| notice.visible() || state.store.link_pending)
    {
        let area = frame.area();
        if area.height > 0 {
            let footer =
                ratatui::layout::Rect::new(area.x, area.y + area.height - 1, area.width, 1);
            frame.render_widget(ratatui::widgets::Clear, footer);
            frame.render_widget(
                ratatui::widgets::Paragraph::new(format!("  {}", notice.message)).style(
                    ratatui::style::Style::default().fg(match notice.kind {
                        crate::tui::app::store::NoticeKind::Error => theme::current().error,
                        crate::tui::app::store::NoticeKind::Info => theme::current().accent,
                    }),
                ),
                footer,
            );
        }
    }
    if let Some(error) = &state.store.draft_error {
        let area = frame.area();
        if area.height > 0 {
            let footer =
                ratatui::layout::Rect::new(area.x, area.y + area.height - 1, area.width, 1);
            frame.render_widget(ratatui::widgets::Clear, footer);
            frame.render_widget(
                ratatui::widgets::Paragraph::new(error.as_str())
                    .style(ratatui::style::Style::default().fg(theme::current().error)),
                footer,
            );
        }
    }
}

pub fn key_to_action(state: &AppState, key: KeyEvent) -> Option<Action> {
    let key = normalize_key(key);

    let detail = match state.screen {
        Screen::List => None,
        Screen::Detail { pr_id, tab } => {
            let Some(ctx) = pr_detail::DetailContext::new(&state.store, pr_id, tab) else {
                return leave_key(key);
            };
            Some(ctx)
        }
    };
    if let Some(ctx) = &detail
        && state.ui.modal_open(&state.store, state.screen)
    {
        return state.ui.detail.handle_key(key, ctx);
    }

    if let Some((search, kind)) = active_search(state)
        && let Some(action) = search.handle_key(key, &kind)
    {
        return Some(action);
    }

    match &detail {
        None => state.ui.list.handle_key(
            key,
            &pr_list::ListContext::from_store(&state.store, state.ui.list.filter, state.screen),
        ),
        Some(ctx) => state.ui.detail.handle_key(key, ctx),
    }
}

/// The keys that work on a PR screen with no PR to show: the way out. The list
/// keeps the PR that is open, so this is not a state a user gets into.
const fn leave_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('q') => Some(Action::Effect(Effect::Quit)),
        KeyCode::Esc => Some(Action::Effect(Effect::Navigate(Screen::List))),
        _ => None,
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

fn active_search(state: &AppState) -> Option<(&SearchInput, SearchKind)> {
    match state.screen {
        Screen::List => state
            .ui
            .list
            .overlay
            .is_none()
            .then_some((&state.ui.list.search, SearchKind::Filter)),
        Screen::Detail { tab, .. } => state.ui.detail.active_search(tab),
    }
}

#[cfg(test)]
pub(crate) mod regression_tests;
#[derive(Debug, Default)]
pub struct Ui {
    pub list: pr_list::PrListScreen,
    pub detail: pr_detail::PrDetailScreen,
}

impl Ui {
    pub fn open_pr(&mut self, pr_id: PrId) {
        self.list.search = SearchInput::default();
        self.detail.open(pr_id);
    }

    pub fn modal_open(&self, store: &crate::tui::app::store::Store, screen: Screen) -> bool {
        match screen {
            Screen::List => self.list.overlay.is_some(),
            Screen::Detail { pr_id, .. } => {
                self.detail.modal_open() || store.errors.contains_key(&pr_id)
            }
        }
    }

    pub fn update(
        &mut self,
        action: Action,
        store: &crate::tui::app::store::Store,
        screen: Screen,
    ) -> Option<Effect> {
        let detail = match screen {
            Screen::List => None,
            Screen::Detail { pr_id, tab } => pr_detail::DetailContext::new(store, pr_id, tab),
        };
        match action {
            Action::Effect(effect) => Some(effect),
            Action::Paste(text) => {
                if let Screen::Detail { pr_id, .. } = screen
                    && !store.operations.contains_key(&pr_id)
                    && !store.errors.contains_key(&pr_id)
                    && self.detail.editor.is_open()
                {
                    self.detail.editor.insert_text(&text);
                }
                None
            }
            Action::HelpScroll(delta) => {
                let help = match screen {
                    Screen::List => match &mut self.list.overlay {
                        Some(pr_list::ListOverlay::Help(help)) => Some(help),
                        Some(
                            pr_list::ListOverlay::FilterPicker { .. }
                            | pr_list::ListOverlay::SortPicker { .. },
                        )
                        | None => None,
                    },
                    Screen::Detail { .. } => match &mut self.detail.overlay {
                        Some(pr_detail::Overlay::Help(help)) => Some(help),
                        Some(
                            pr_detail::Overlay::Confirm(_)
                            | pr_detail::Overlay::Review(_)
                            | pr_detail::Overlay::Merge(_),
                        )
                        | None => None,
                    },
                };
                if let Some(help) = help {
                    help.update(delta, &());
                }
                None
            }
            Action::List(action) => self.list.update(
                action,
                &pr_list::ListContext::from_store(store, self.list.filter, screen),
            ),
            Action::Search(action) => {
                match screen {
                    Screen::List => {
                        let picking = matches!(
                            self.list.overlay,
                            Some(
                                pr_list::ListOverlay::FilterPicker { .. }
                                    | pr_list::ListOverlay::SortPicker { .. }
                            )
                        );
                        if !picking {
                            self.list.update_search(action);
                        }
                    }
                    Screen::Detail { .. } => {
                        if let Some(ctx) = &detail {
                            self.detail.update_search(action, ctx);
                        }
                    }
                }
                None
            }
            // A message for the PR screen with no PR on screen has nothing to act on.
            Action::Detail(action) => self.detail.update(action, &detail?),
            Action::Diff(action) => self.detail.update_diff(action, &detail?),
            Action::Commits(action) => self.detail.update_commits(action, &detail?),
        }
    }
}

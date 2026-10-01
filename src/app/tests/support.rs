//! Imports and helpers shared by the files in this directory.

pub(super) use crate::app::{App, action::*, event_loop::Next};
pub(super) use crate::{
    app::{
        navigation::Screen,
        reviews::{CommentAnchor, CommentTarget},
        store::LoadState,
    },
    providers::Provider,
    tui::{
        self,
        component::Component,
        components::diff_viewer::DiffFocus,
        screens::pr_detail::{
            dialogs::{
                confirm::{ConfirmDialog, ConfirmKind},
                review::ReviewDialog,
            },
            tabs::DetailTab,
        },
    },
};
pub(super) use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(super) fn app() -> App {
    let mut app = App::new(Provider::GitHub, "reviewer".into());
    app.state = tui::regression_tests::fixture();
    app
}

pub(super) fn press(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
}

pub(super) fn detail(app: &mut App, tab: DetailTab) {
    app.apply(Action::List(ListAction::OpenPr(42)));
    app.apply(Action::Detail(DetailAction::Nav(NavAction::SelectTab(tab))));
}

pub(super) fn send_comment(app: &mut App) {
    app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
}

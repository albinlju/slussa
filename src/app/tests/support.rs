//! Imports and helpers shared by the files in this directory.

pub(super) use crate::app::{App, action::*, drafts::Drafts, event_loop::Next, preflight::Session};
pub(super) use crate::{
    app::{
        navigation::Screen,
        reviews::{CommentAnchor, CommentTarget},
        store::{LoadState, WriteTicket},
    },
    domain::comment::{CommentKey, CommentKind},
    providers::{FetchError, Provider},
    tui::{
        self,
        component::Component,
        components::diff_viewer::{DiffFocus, FocusedNav, NavTarget},
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

/// End the write pending for `pr_id` with `result`, as its worker would. With
/// none pending there is no write to end.
pub(super) fn finish_write(app: &mut App, pr_id: u64, result: Result<(), WriteError>) {
    let Some(&operation) = app.state.store.operations.get(&pr_id) else {
        return;
    };
    app.apply_result(TaskResult::Written {
        ticket: WriteTicket::pending(pr_id, operation),
        result,
    });
}

/// A failure whose message for the user is `message`, of the kind that leaves
/// open whether the server was reached.
pub(super) fn failed(message: &str) -> FetchError {
    FetchError::GhFailed {
        code: Some(1),
        stderr: message.into(),
    }
}

pub(super) fn app() -> App {
    let mut app = App::new(
        Session::for_test(Provider::GitHub, "reviewer"),
        Drafts::Nowhere,
    );
    app.state = tui::regression_tests::fixture();
    app
}

/// Put a second PR in the list, a copy of the fixture's with another id. A PR
/// can only be opened from the list, so a test that opens one adds it first.
pub(super) fn add_pr(app: &mut App, id: u64) {
    let LoadState::Loaded(prs) = &mut app.state.store.cache.prs else {
        panic!("the fixture's list is loaded");
    };
    let mut pr = prs[0].clone();
    pr.id = id;
    prs.push(pr);
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

use crate::{
    app::{
        action::{Action, DetailAction, NavAction, PrAction},
        navigation::Screen,
        state::AppState,
        store::LoadState,
    },
    domain::{
        capabilities::Feature,
        comment::CommentId,
        pr::{PrId, PrStatus},
    },
    tui::{
        components::diff_viewer::{DiffFocus, FocusedNav, NavTarget},
        key_to_action,
        regression_tests::fixture,
        render,
        screens::pr_detail::tabs::DetailTab,
    },
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};

/// The fixture's PR 42 on its Overview, in the given status.
fn overview_of(status: PrStatus) -> AppState {
    let mut state = fixture();
    state.screen = Screen::Detail {
        pr_id: PrId(42),
        tab: DetailTab::Overview,
    };
    let LoadState::Loaded(prs) = &mut state.store.cache.prs else {
        panic!("fixture lists PRs");
    };
    prs[0].status = status;
    state
}

fn x(state: &AppState) -> Option<Action> {
    key_to_action(state, KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE))
}

fn footer_of(state: &mut AppState) -> String {
    let (width, height) = (160, 30);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render(frame, state)).unwrap();
    let buffer = terminal.backend().buffer();
    (height - 2..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn x_declines_an_open_pr_and_reopens_a_declined_one() {
    for status in [PrStatus::Open, PrStatus::Draft] {
        assert!(
            matches!(
                x(&overview_of(status)),
                Some(Action::Detail(DetailAction::Pr(PrAction::OpenDecline)))
            ),
            "an open PR is declined"
        );
    }
    assert!(matches!(
        x(&overview_of(PrStatus::Declined)),
        Some(Action::Detail(DetailAction::Pr(PrAction::OpenReopen)))
    ));
    assert!(
        x(&overview_of(PrStatus::Merged)).is_none(),
        "a merged PR cannot be reopened"
    );
}

#[test]
fn x_does_not_reopen_where_the_provider_cannot() {
    let mut state = overview_of(PrStatus::Declined);
    state.store.capabilities.features.remove(&Feature::ReopenPr);
    assert!(x(&state).is_none());
}

fn on_tab(mut state: AppState, tab: DetailTab) -> AppState {
    state.screen = Screen::Detail {
        pr_id: PrId(42),
        tab,
    };
    state
}

fn key(state: &AppState, c: char) -> Option<Action> {
    key_to_action(state, KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
}

#[test]
fn the_pr_level_actions_work_from_the_description_too() {
    let state = on_tab(overview_of(PrStatus::Open), DetailTab::Description);
    assert!(matches!(
        key(&state, 'x'),
        Some(Action::Detail(DetailAction::Pr(PrAction::OpenDecline)))
    ));
    assert!(matches!(
        key(&state, 'a'),
        Some(Action::Detail(DetailAction::Pr(PrAction::OpenReviewPicker)))
    ));
    assert!(matches!(
        key(&state, 'v'),
        Some(Action::Detail(DetailAction::Pr(PrAction::StartReview)))
    ));
    let declined = on_tab(overview_of(PrStatus::Declined), DetailTab::Description);
    assert!(matches!(
        key(&declined, 'x'),
        Some(Action::Detail(DetailAction::Pr(PrAction::OpenReopen)))
    ));
}

#[test]
fn the_pr_level_actions_stay_off_the_builds_tab() {
    let state = on_tab(overview_of(PrStatus::Open), DetailTab::Builds);
    assert!(key(&state, 'x').is_none());
    assert!(key(&state, 'a').is_none());
}

#[test]
fn the_description_footer_lists_the_pr_actions() {
    let mut state = on_tab(overview_of(PrStatus::Open), DetailTab::Description);
    let text = footer_of(&mut state);
    assert!(text.contains("a: submit review"), "{text}");
    assert!(text.contains("x: decline"), "{text}");
    assert!(text.contains("j/k: scroll"), "{text}");
}

#[test]
fn h_and_l_change_tab_on_every_tab_including_the_diff_panes() {
    for tab in DetailTab::ALL {
        for focus in [DiffFocus::Tree, DiffFocus::Pane] {
            let mut state = on_tab(overview_of(PrStatus::Open), tab);
            state.ui.detail.diff.focus = focus;
            assert!(
                matches!(
                    key(&state, 'l'),
                    Some(Action::Detail(DetailAction::Nav(NavAction::NextTab)))
                ),
                "l on {tab:?} with {focus:?} focus"
            );
            assert!(
                matches!(
                    key(&state, 'h'),
                    Some(Action::Detail(DetailAction::Nav(NavAction::PrevTab)))
                ),
                "h on {tab:?} with {focus:?} focus"
            );
        }
    }
}

#[test]
fn the_diff_footer_says_how_to_go_back_and_change_tab() {
    let mut state = on_tab(overview_of(PrStatus::Open), DetailTab::Diff);
    let text = footer_of(&mut state);
    assert!(text.contains("h/l: tabs"), "{text}");
}

#[test]
fn the_footer_offers_reopen_only_for_a_declined_pr() {
    let open = footer_of(&mut overview_of(PrStatus::Open));
    assert!(open.contains("x: decline"), "{open}");
    assert!(!open.contains("x: reopen"), "{open}");

    let declined = footer_of(&mut overview_of(PrStatus::Declined));
    assert!(declined.contains("x: reopen"), "{declined}");
    assert!(!declined.contains("x: decline"), "{declined}");

    let merged = footer_of(&mut overview_of(PrStatus::Merged));
    assert!(merged.contains("x: decline (merged)"), "{merged}");

    let mut unsupported = overview_of(PrStatus::Declined);
    unsupported
        .store
        .capabilities
        .features
        .remove(&Feature::ReopenPr);
    let text = footer_of(&mut unsupported);
    assert!(!text.contains("x: reopen"), "{text}");
}

#[test]
fn a_pr_that_is_not_in_the_list_gives_no_context_and_only_the_way_out() {
    use crate::{app::action::Effect, tui::screens::pr_detail::DetailContext};
    let mut state = overview_of(PrStatus::Open);
    assert!(DetailContext::new(&state.store, PrId(42), DetailTab::Overview).is_some());

    state.screen = Screen::Detail {
        pr_id: PrId(7),
        tab: DetailTab::Overview,
    };
    assert!(DetailContext::new(&state.store, PrId(7), DetailTab::Overview).is_none());
    let key = |code| key_to_action(&state, KeyEvent::new(code, KeyModifiers::NONE));
    assert!(matches!(
        key(KeyCode::Char('q')),
        Some(Action::Effect(Effect::Quit))
    ));
    assert!(matches!(
        key(KeyCode::Esc),
        Some(Action::Effect(Effect::Navigate(Screen::List)))
    ));
    assert!(key(KeyCode::Char('m')).is_none());
    assert!(key(KeyCode::Char('c')).is_none());
    // A message that still arrives has no PR to act on.
    let effect = state.ui.update(
        Action::Detail(DetailAction::Pr(PrAction::OpenComment)),
        &state.store,
        state.screen,
    );
    assert!(effect.is_none());
    assert!(!state.ui.detail.editor.has_draft());
}

/// The Commits list, reached after the Diff tab was left with its pane on
/// `focused`, and with a review in progress.
fn commit_list_after_the_diff_pane(focused: FocusedNav) -> AppState {
    let mut state = overview_of(PrStatus::Open);
    state.ui.detail.diff.focus = DiffFocus::Pane;
    state.ui.detail.diff.pane.focused = Some(focused);
    state.store.reviews.entry(PrId(42)).or_default();
    state.screen = Screen::Detail {
        pr_id: PrId(42),
        tab: DetailTab::Commits,
    };
    state
}

#[test]
fn the_commit_list_does_not_reply_to_the_thread_the_diff_tab_left_focused() {
    let state = commit_list_after_the_diff_pane(FocusedNav::on_thread(CommentId(7)));
    let r = key_to_action(
        &state,
        KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
    );
    assert!(r.is_none(), "{r:?}");
    let c = key_to_action(
        &state,
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
    );
    assert!(c.is_none(), "{c:?}");
}

#[test]
fn the_commit_list_footer_offers_only_keys_that_work_there() {
    let mut state = commit_list_after_the_diff_pane(FocusedNav::on_thread(CommentId(7)));
    let footer = footer_of(&mut state);
    assert!(footer.contains("v: finish draft"), "{footer}");
    assert!(!footer.contains("r: reply"), "{footer}");
    let mut state = commit_list_after_the_diff_pane(FocusedNav::on(NavTarget::Pending(0)));
    let footer = footer_of(&mut state);
    assert!(footer.contains("v: finish draft"), "{footer}");
    assert!(!footer.contains("d: remove pending"), "{footer}");

    // On the diff itself the queued comment can be removed, as before.
    state.screen = Screen::Detail {
        pr_id: PrId(42),
        tab: DetailTab::Diff,
    };
    let d = key_to_action(
        &state,
        KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE),
    );
    assert!(
        matches!(
            d,
            Some(Action::Detail(DetailAction::Pr(
                PrAction::RemovePendingComment
            )))
        ),
        "{d:?}"
    );
}

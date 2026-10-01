use crate::{
    app::{
        action::{Action, DetailAction, NavAction, PrAction},
        navigation::Screen,
        state::AppState,
        store::LoadState,
    },
    domain::{capabilities::Feature, pr::PrStatus},
    tui::{
        components::diff_viewer::DiffFocus, key_to_action, regression_tests::fixture, render,
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
        pr_id: 42,
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
    state.screen = Screen::Detail { pr_id: 42, tab };
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

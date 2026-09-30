use crate::{
    app::{
        action::{Action, DetailAction},
        navigation::Screen,
        state::AppState,
        store::LoadState,
    },
    domain::{capabilities::Feature, pr::PrStatus},
    tui::{key_to_action, regression_tests::fixture, render, screens::pr_detail::tabs::DetailTab},
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
                Some(Action::Detail(DetailAction::OpenDecline))
            ),
            "an open PR is declined"
        );
    }
    assert!(matches!(
        x(&overview_of(PrStatus::Declined)),
        Some(Action::Detail(DetailAction::OpenReopen))
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

#[test]
fn x_only_acts_on_the_overview() {
    let mut state = overview_of(PrStatus::Declined);
    state.screen = Screen::Detail {
        pr_id: 42,
        tab: DetailTab::Description,
    };
    assert!(x(&state).is_none());
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

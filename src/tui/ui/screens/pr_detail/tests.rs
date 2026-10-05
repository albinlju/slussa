use crate::{
    domain::{
        capabilities::Feature,
        comment::CommentId,
        pr::{PrId, PrStatus},
    },
    tui::{
        app::{navigation::Screen, state::AppState, store::LoadState},
        ui::{
            action::{Action, DetailAction, NavAction, PrAction},
            components::diff_viewer::{DiffFocus, FocusedNav, NavTarget},
            key_to_action,
            regression_tests::fixture,
            render,
            screens::pr_detail::tabs::DetailTab,
        },
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
    for status in [PrStatus::open(), PrStatus::draft()] {
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
    let state = on_tab(overview_of(PrStatus::open()), DetailTab::Description);
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

/// The footer and the key read the same row, so what the footer dims does
/// nothing and what it lights works, for every state the PR can be in.
#[test]
fn a_hint_dimmed_in_the_footer_does_nothing_and_a_lit_one_works() {
    for tab in [DetailTab::Overview, DetailTab::Description] {
        for status in [
            PrStatus::open(),
            PrStatus::draft(),
            PrStatus::Declined,
            PrStatus::Merged,
        ] {
            let mut state = on_tab(overview_of(status.clone()), tab);
            let footer = footer_of(&mut state);
            for (typed, label) in [('m', "m: merge"), ('x', "x: decline"), ('x', "x: reopen")] {
                let dimmed = footer.contains(&format!("{label} ("));
                let lit = !dimmed && footer.contains(label);
                let acts = key(&state, typed).is_some();
                assert!(
                    !dimmed || !acts,
                    "{label} is dimmed on {tab:?} for {status:?} and still acts"
                );
                assert!(
                    !lit || acts,
                    "{label} is lit on {tab:?} for {status:?} and does nothing"
                );
            }
        }
    }
}

#[test]
fn the_pr_level_actions_stay_off_the_builds_tab() {
    let state = on_tab(overview_of(PrStatus::open()), DetailTab::Builds);
    assert!(key(&state, 'x').is_none());
    assert!(key(&state, 'a').is_none());
}

#[test]
fn the_description_footer_lists_the_pr_actions() {
    let mut state = on_tab(overview_of(PrStatus::open()), DetailTab::Description);
    let text = footer_of(&mut state);
    assert!(text.contains("a: submit review"), "{text}");
    assert!(text.contains("x: decline"), "{text}");
    assert!(text.contains("j/k: scroll"), "{text}");
}

#[test]
fn h_and_l_change_tab_on_every_tab_including_the_diff_panes() {
    for tab in DetailTab::ALL {
        for focus in [DiffFocus::Tree, DiffFocus::Pane] {
            let mut state = on_tab(overview_of(PrStatus::open()), tab);
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
    let mut state = on_tab(overview_of(PrStatus::open()), DetailTab::Diff);
    let text = footer_of(&mut state);
    assert!(text.contains("h/l: tabs"), "{text}");
}

#[test]
fn the_footer_offers_reopen_only_for_a_declined_pr() {
    let open = footer_of(&mut overview_of(PrStatus::open()));
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
    use crate::tui::{app::effect::Effect, ui::screens::pr_detail::DetailContext};
    let mut state = overview_of(PrStatus::open());
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
    let mut state = overview_of(PrStatus::open());
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

#[test]
fn the_builds_footer_offers_run_again_for_a_failed_build_and_says_why_not_on_a_closed_pr() {
    use crate::domain::ci::{Build, BuildState};
    let failing = |status| {
        let mut state = on_tab(overview_of(status), DetailTab::Builds);
        if let Some(data) = state.store.cache.details.get_mut(&PrId(42)) {
            data.builds = LoadState::Loaded(vec![Build {
                name: "ci".into(),
                state: BuildState::Failed,
                duration_ms: None,
            }]);
        }
        state
    };
    let open = footer_of(&mut failing(PrStatus::open()));
    assert!(open.contains("b: run failed again"), "{open}");
    assert!(!open.contains("run failed again ("), "{open}");

    let merged = footer_of(&mut failing(PrStatus::Merged));
    assert!(merged.contains("b: run failed again (merged)"), "{merged}");

    let passing = footer_of(&mut on_tab(
        overview_of(PrStatus::open()),
        DetailTab::Builds,
    ));
    assert!(!passing.contains("run failed again"), "{passing}");
}

#[test]
fn the_footer_names_who_p_asks_again_and_says_why_not_on_a_closed_pr() {
    use crate::domain::review::{Reviewer, ReviewerState};
    let asked = |status, names: &[&str]| {
        let mut state = on_tab(overview_of(status), DetailTab::Overview);
        if let LoadState::Loaded(prs) = &mut state.store.cache.prs {
            prs[0].reviewers = names
                .iter()
                .map(|name| Reviewer {
                    author: crate::domain::user::User {
                        username: (*name).into(),
                    },
                    state: ReviewerState::ChangesRequested,
                })
                .collect();
        }
        state
    };
    let one = footer_of(&mut asked(PrStatus::open(), &["alice"]));
    assert!(one.contains("p: ask alice again"), "{one}");

    let several = footer_of(&mut asked(PrStatus::open(), &["alice", "erin", "frank"]));
    assert!(several.contains("p: ask alice +2 again"), "{several}");

    let merged = footer_of(&mut asked(PrStatus::Merged, &["alice"]));
    assert!(merged.contains("p: ask alice again (merged)"), "{merged}");

    let nobody = footer_of(&mut asked(PrStatus::open(), &[]));
    assert!(!nobody.contains("ask"), "{nobody}");
}

fn with_issues(issues: Vec<crate::domain::pr::LinkedIssue>) -> AppState {
    use crate::domain::pr::PrInfo;
    let mut state = on_tab(overview_of(PrStatus::open()), DetailTab::Overview);
    if let Some(data) = state.store.cache.details.get_mut(&PrId(42)) {
        data.info = LoadState::Loaded(PrInfo {
            description: None,
            labels: vec![],
            issues,
        });
    }
    state
}

fn issue(number: u64, url: Option<&str>) -> crate::domain::pr::LinkedIssue {
    crate::domain::pr::LinkedIssue {
        number,
        title: format!("Issue {number}"),
        url: url.map(str::to_owned),
    }
}

#[test]
fn i_opens_the_issue_the_pr_closes_and_the_footer_names_it() {
    let one = footer_of(&mut with_issues(vec![issue(
        12,
        Some("https://example.com/o/r/issues/12"),
    )]));
    assert!(one.contains("i: open #12"), "{one}");
    assert!(!one.contains("(+"), "{one}");

    let state = with_issues(vec![
        issue(12, Some("https://example.com/o/r/issues/12")),
        issue(31, Some("https://example.com/o/other/issues/31")),
    ]);
    let key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE);
    assert!(
        matches!(
            key_to_action(&state, key),
            Some(Action::Effect(crate::tui::app::effect::Effect::IssueLink { number: 12, ref url }))
                if url == "https://example.com/o/r/issues/12"
        ),
        "the first one is opened"
    );
    let several = footer_of(&mut with_issues(vec![
        issue(12, Some("https://example.com/o/r/issues/12")),
        issue(31, Some("https://example.com/o/other/issues/31")),
    ]));
    assert!(several.contains("i: open #12 (+1)"), "{several}");
}

#[test]
fn i_is_not_offered_without_an_issue_to_open() {
    for issues in [vec![], vec![issue(12, None)]] {
        let mut state = with_issues(issues);
        let key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE);
        assert!(key_to_action(&state, key).is_none());
        assert!(!footer_of(&mut state).contains("i: open"));
    }
    // Before the description and issues are read.
    let mut unread = on_tab(overview_of(PrStatus::open()), DetailTab::Overview);
    assert!(!footer_of(&mut unread).contains("i: open"));
}

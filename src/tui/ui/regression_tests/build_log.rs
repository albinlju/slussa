//! The Builds tab: a cursor on the builds, and the log of one opened in the tab.

use super::support::*;
use crate::domain::{
    build_log::BuildLog,
    ci::{Build, BuildState, JobId},
};

fn build(name: &str, state: BuildState, log: Option<u64>) -> Build {
    Build {
        name: name.into(),
        state,
        duration_ms: None,
        log: log.map(JobId),
    }
}

/// A failing log of `lines` lines whose only error is at line `error`.
fn log_with_error_at(lines: usize, error: usize) -> BuildLog {
    let text: Vec<String> = (0..lines)
        .map(|n| {
            if n == error {
                "##[error]the failure".to_owned()
            } else {
                format!("output {n}")
            }
        })
        .collect();
    BuildLog::parse(&text.join("\n"))
}

fn builds_tab(builds: Vec<Build>) -> AppState {
    let mut state = pr_on(DetailTab::Builds, Activity::default());
    if let Some(data) = state.store.cache.details.get_mut(&PrId(42)) {
        data.builds = LoadState::Loaded(builds);
    }
    state
}

fn give_log(state: &mut AppState, job: u64, log: LoadState<BuildLog>) {
    if let Some(data) = state.store.cache.details.get_mut(&PrId(42)) {
        data.build_logs.insert(JobId(job), log);
    }
}

fn open_second_build(state: &mut AppState) {
    local_key(state, KeyCode::Char('j'));
    let action = key(state, KeyCode::Enter);
    let effect = state.ui.update(action, &state.store, state.screen);
    assert!(
        matches!(
            effect,
            Some(Effect::LoadBuildLog {
                pr_id: PrId(42),
                job: JobId(99)
            })
        ),
        "{effect:?}"
    );
}

#[test]
fn enter_opens_the_log_of_the_build_under_the_cursor_and_asks_for_it() {
    let mut state = builds_tab(vec![
        build("lint", BuildState::Successful, Some(98)),
        build("test", BuildState::Failed, Some(99)),
    ]);
    open_second_build(&mut state);
    let text = screen(&mut state);
    assert!(text.contains("test"), "the build's name heads the log");
    assert!(!text.contains("lint"), "the list is replaced by the log");
    assert!(
        footer_of(&text).contains("esc: back"),
        "{}",
        footer_of(&text)
    );
}

#[test]
fn a_check_that_is_not_an_action_has_no_log_to_open() {
    let mut state = builds_tab(vec![build("coverage", BuildState::Failed, None)]);
    assert!(key_to_action(&state, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).is_none());
    let text = screen(&mut state);
    assert!(!footer_of(&text).contains("enter: log"));
}

#[test]
fn the_log_opens_on_its_first_error_and_n_steps_between_errors() {
    let mut state = builds_tab(vec![
        build("lint", BuildState::Successful, Some(98)),
        build("test", BuildState::Failed, Some(99)),
    ]);
    let mut lines: Vec<String> = (0..200).map(|n| format!("output {n}")).collect();
    lines[120] = "##[error]first failure".into();
    lines[180] = "##[error]second failure".into();
    give_log(
        &mut state,
        99,
        LoadState::Loaded(BuildLog::parse(&lines.join("\n"))),
    );
    open_second_build(&mut state);

    let text = draw(&mut state, 100, 20);
    assert!(text.contains("first failure"), "landed on the first error");
    assert!(!text.contains("output 0 "), "not at the top");
    assert!(text.contains("2 errors"));

    local_key(&mut state, KeyCode::Char('n'));
    let text = draw(&mut state, 100, 20);
    assert!(text.contains("second failure"));
    // From the last error `n` goes round to the first.
    local_key(&mut state, KeyCode::Char('n'));
    assert!(draw(&mut state, 100, 20).contains("first failure"));
    local_key(&mut state, KeyCode::Char('N'));
    assert!(draw(&mut state, 100, 20).contains("second failure"));
}

#[test]
fn a_log_without_a_marked_error_opens_at_its_end() {
    let mut state = builds_tab(vec![
        build("lint", BuildState::Successful, Some(98)),
        build("test", BuildState::Failed, Some(99)),
    ]);
    give_log(
        &mut state,
        99,
        LoadState::Loaded(log_with_error_at(100, 1000)),
    );
    open_second_build(&mut state);
    let text = draw(&mut state, 100, 20);
    assert!(text.contains("output 99"));
    assert!(text.contains("no errors marked"));
}

#[test]
fn a_log_still_being_read_or_that_failed_says_so_and_esc_goes_back_to_the_builds() {
    let mut state = builds_tab(vec![
        build("lint", BuildState::Successful, Some(98)),
        build("test", BuildState::Failed, Some(99)),
    ]);
    give_log(&mut state, 99, LoadState::Loading);
    open_second_build(&mut state);
    assert!(screen(&mut state).to_lowercase().contains("loading"));

    give_log(
        &mut state,
        99,
        LoadState::Failed(crate::providers::FetchError::Timeout),
    );
    assert!(screen(&mut state).contains("Couldn't load the build log"));

    local_key(&mut state, KeyCode::Esc);
    let text = screen(&mut state);
    assert!(
        text.contains("lint") && text.contains("test"),
        "the list is back"
    );
    assert!(
        matches!(
            state.screen,
            Screen::Detail {
                tab: DetailTab::Builds,
                ..
            }
        ),
        "esc leaves the log, not the PR"
    );
}

#[test]
fn leaving_the_tab_closes_the_log() {
    let mut state = builds_tab(vec![
        build("lint", BuildState::Successful, Some(98)),
        build("test", BuildState::Failed, Some(99)),
    ]);
    give_log(&mut state, 99, LoadState::Loaded(log_with_error_at(10, 2)));
    open_second_build(&mut state);
    let action = Action::Detail(DetailAction::Nav(NavAction::SelectTab(DetailTab::Overview)));
    state.ui.update(action, &state.store, state.screen);
    let action = Action::Detail(DetailAction::Nav(NavAction::SelectTab(DetailTab::Builds)));
    state.ui.update(action, &state.store, state.screen);
    assert!(
        screen(&mut state).contains("lint"),
        "the builds, not the log"
    );
}

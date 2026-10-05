use super::*;
use crate::{
    domain::{
        ci::CiSummary,
        pr::{AiReview, PrGroup, PrId, PrStatus, PullRequest},
        review::{Reviewer, ReviewerState},
        seen::Seen,
        user::{User, Username},
    },
    tui::{
        app::{
            effect::Effect,
            navigation::Screen,
            pr_groups::OpenChain,
            state::AppState,
            store::{FetchKey, LoadState},
        },
        ui::{
            action::{Action, ListAction},
            component::Component,
            components::help_dialog::HelpDialog,
            key_to_action, render,
        },
    },
};
use chrono::Utc;
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};

fn reviewer(name: &str, state: ReviewerState) -> Reviewer {
    Reviewer {
        author: User {
            username: name.into(),
        },
        state,
    }
}

fn pr(id: u64, author: &str, ci: CiSummary, reviewers: Vec<Reviewer>) -> PullRequest {
    PullRequest {
        url: None,
        id: PrId(id),
        title: format!("Change {id}"),
        description: None,
        author: User {
            username: author.into(),
        },
        ci,
        status: PrStatus::open(),
        reviewers,
        labels: Vec::new(),
        comment_count: 0,
        source_branch: "feature".into(),
        source_repo: crate::domain::pr::SourceRepo::Unknown,
        head_oid: None,
        target_branch: "main".into(),
        additions: 1,
        deletions: 1,
        changed_files: 1,
        created: Utc::now(),
        updated: Utc::now(),
        ai_review: AiReview::None,
    }
}

/// In the provider's order (newest first): 5 asks nothing of "me", 4 asks for
/// my review, 3 is mine with failed CI, 2 asks nothing, 1 is mine with
/// changes requested.
fn prs() -> LoadState<Vec<PullRequest>> {
    LoadState::Loaded(vec![
        pr(5, "alice", CiSummary::Success, Vec::new()),
        pr(
            4,
            "alice",
            CiSummary::Success,
            vec![reviewer("me", ReviewerState::Requested)],
        ),
        pr(3, "me", CiSummary::Failed, Vec::new()),
        pr(2, "bob", CiSummary::Success, Vec::new()),
        pr(
            1,
            "me",
            CiSummary::Success,
            vec![reviewer("carol", ReviewerState::ChangesRequested)],
        ),
    ])
}

/// Mark whether older PRs remain to be read in a group.
fn set_more(state: &mut AppState, group: PrGroup, more: bool) {
    state.store.groups.entry(group).or_default().more = more.then(|| "x".to_owned());
}

static NOTHING_SEEN: Seen = Seen::new();

/// What the list is drawn from: `prs` as `viewer` sees them, with nothing
/// loading unless `arriving` says the open pages are still coming in.
fn context<'a>(
    prs: &'a LoadState<Vec<PullRequest>>,
    viewer: &'a Username,
    arriving: bool,
) -> ListContext<'a> {
    ListContext {
        prs,
        refreshing: false,
        viewer,
        seen: &NOTHING_SEEN,
        more: false,
        loading_more: arriving,
        view_loading: false,
        arriving,
    }
}

fn ids(list: &PrListScreen, prs: &LoadState<Vec<PullRequest>>, viewer: &str) -> Vec<u64> {
    ids_while(list, prs, viewer, false)
}

fn ids_while(
    list: &PrListScreen,
    prs: &LoadState<Vec<PullRequest>>,
    viewer: &str,
    arriving: bool,
) -> Vec<u64> {
    list.filtered_prs(&context(prs, &viewer.into(), arriving))
        .iter()
        .map(|pr| pr.id.0)
        .collect()
}

fn state(viewer: &str) -> AppState {
    let mut state = AppState {
        screen: Screen::List,
        ..AppState::default()
    };
    state.store.current_user = viewer.into();
    state.store.cache.prs = prs();
    state
}

fn drawn(state: &mut AppState, width: u16) -> String {
    drawn_high(state, width, 12)
}

fn drawn_high(state: &mut AppState, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render(frame, state)).unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn attention_order_puts_what_needs_you_first_and_keeps_provider_order_otherwise() {
    let list = PrListScreen::default();
    assert_eq!(list.sort, Sort::Attention, "attention is the default");
    assert_eq!(ids(&list, &prs(), "me"), [1, 3, 4, 5, 2]);
}

#[test]
fn the_order_is_held_while_the_open_prs_are_read_and_applied_after() {
    let list = PrListScreen::default();
    assert_eq!(
        ids_while(&list, &prs(), "me", true),
        [5, 4, 3, 2, 1],
        "rows stay where they arrived"
    );
    assert_eq!(ids_while(&list, &prs(), "me", false), [1, 3, 4, 5, 2]);
}

#[test]
fn the_heading_says_more_is_coming_while_the_open_prs_are_read() {
    let mut reading = state("me");
    reading.store.open_chain = OpenChain::Appending;
    let text = drawn(&mut reading, 100);
    assert!(text.contains("loading more..."), "{text}");
    assert!(
        !text.contains("L: more"),
        "L waits until the reading ends: {text}"
    );

    reading.store.open_chain = OpenChain::Idle;
    set_more(&mut reading, PrGroup::Open, true);
    let text = drawn(&mut reading, 100);
    assert!(text.contains("Open, more unread"), "{text}");
    assert!(text.contains("L: more"), "{text}");
}

#[test]
fn recent_order_and_a_viewer_no_pr_involves_leave_the_provider_order_alone() {
    let recent = PrListScreen {
        sort: Sort::Recent,
        ..PrListScreen::default()
    };
    assert_eq!(ids(&recent, &prs(), "me"), [5, 4, 3, 2, 1]);
    let attention = PrListScreen::default();
    assert_eq!(ids(&attention, &prs(), "nobody"), [5, 4, 3, 2, 1]);
}

#[test]
fn filters_apply_before_ordering() {
    let mut merged = pr(6, "me", CiSummary::Failed, Vec::new());
    merged.status = PrStatus::Merged;
    let LoadState::Loaded(mut all) = prs() else {
        panic!("fixture is loaded");
    };
    all.push(merged);
    let prs = LoadState::Loaded(all);
    let open = PrListScreen::default();
    assert_eq!(ids(&open, &prs, "me"), [1, 3, 4, 5, 2]);
    let everything = PrListScreen {
        filter: StatusFilter::All,
        ..PrListScreen::default()
    };
    assert_eq!(
        ids(&everything, &prs, "me"),
        [1, 3, 4, 5, 2, 6],
        "a merged PR never asks for attention"
    );
}

#[test]
fn toggling_the_sort_follows_the_highlighted_pr() {
    let prs = prs();
    let viewer = "me".into();
    let ctx = context(&prs, &viewer, false);
    let mut list = PrListScreen {
        selected: 2,
        ..PrListScreen::default()
    };
    assert_eq!(ids(&list, &prs, "me")[list.selected], 4);

    list.update(ListAction::OpenSortPicker, &ctx);
    list.update(ListAction::PickerNext, &ctx);
    list.update(ListAction::ApplyPicker, &ctx);
    assert_eq!(list.sort, Sort::Recent);
    assert_eq!(list.selected, 1, "PR 4 is second when newest first");

    list.update(ListAction::OpenSortPicker, &ctx);
    list.update(ListAction::PickerPrev, &ctx);
    list.update(ListAction::ApplyPicker, &ctx);
    assert_eq!(list.sort, Sort::Attention);
    assert_eq!(list.selected, 2);
}

#[test]
fn s_opens_the_sort_picker_and_help_lists_it() {
    let state = state("me");
    let action = key_to_action(
        &state,
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
    );
    assert!(matches!(
        action,
        Some(Action::List(ListAction::OpenSortPicker))
    ));
    assert!(render::HELP_KEYS.iter().any(|(key, _)| *key == "s"));
}

#[test]
fn the_reason_column_appears_only_with_a_reason_and_room() {
    let wide = drawn(&mut state("me"), 100);
    assert!(wide.contains("Needs you"), "{wide}");
    for reason in ["changes requested", "CI failed", "review requested"] {
        assert!(wide.contains(reason), "{reason} in\n{wide}");
    }

    let nothing_for_them = drawn(&mut state("nobody"), 100);
    assert!(
        !nothing_for_them.contains("Needs you"),
        "{nothing_for_them}"
    );

    let narrow = drawn(&mut state("me"), 80);
    assert!(!narrow.contains("Needs you"), "{narrow}");
    assert!(narrow.contains("Change 1"), "the title stays: {narrow}");
}

fn state_with_ai(reviews: &[AiReview]) -> AppState {
    let mut state = state("me");
    if let LoadState::Loaded(prs) = &mut state.store.cache.prs {
        for (pr, review) in prs.iter_mut().zip(reviews) {
            pr.ai_review = *review;
        }
    }
    state
}

#[test]
fn the_ai_column_is_there_only_while_some_pr_has_been_reviewed_by_an_agent() {
    let none = drawn(&mut state("me"), 130);
    assert!(!none.contains("◇") && !none.contains("◆"), "{none}");

    let mut mixed = state_with_ai(&[
        AiReview::Current,
        AiReview::Stale,
        AiReview::ChangesRequested,
    ]);
    for width in [100, 130] {
        let text = drawn(&mut mixed, width);
        assert!(text.contains("◆") && text.contains("◈"), "{width}: {text}");
        assert!(
            text.contains("◇"),
            "an unreviewed PR is a hollow one: {text}"
        );
    }
    // Too narrow for the column, as for the others.
    let narrow = drawn(&mut mixed, 80);
    assert!(!narrow.contains("◆"), "{narrow}");
}

#[test]
fn the_help_explains_the_ai_column_only_while_it_is_there() {
    let help = |mut state: AppState, width: u16| {
        state.ui.list.overlay = Some(ListOverlay::Help(HelpDialog::default()));
        drawn_high(&mut state, width, 40)
    };
    let with = help(state_with_ai(&[AiReview::Current]), 100);
    assert!(with.contains("AI review: of an older version"), "{with}");
    assert!(!help(state("me"), 100).contains("AI review:"));
    // Too narrow for the column: nothing to explain.
    let narrow = help(state_with_ai(&[AiReview::Current]), 80);
    assert!(!narrow.contains("AI review:"), "{narrow}");
}

#[test]
fn rows_render_in_attention_order() {
    let text = drawn(&mut state("me"), 100);
    let position = |id: u64| text.find(&format!("Change {id}")).unwrap();
    let order: Vec<u64> = {
        let mut found = [1, 3, 4, 5, 2];
        found.sort_by_key(|&id| position(id));
        found.to_vec()
    };
    assert_eq!(order, [1, 3, 4, 5, 2], "{text}");
}

#[test]
fn the_sort_setting_reads_config_and_falls_back_to_attention() {
    assert_eq!(Sort::from_config(None), Sort::Attention);
    assert_eq!(Sort::from_config(Some("attention")), Sort::Attention);
    assert_eq!(Sort::from_config(Some("recent")), Sort::Recent);
    assert_eq!(Sort::from_config(Some("newest")), Sort::Attention);
}

#[test]
fn headings_say_so_only_while_more_remain_unread() {
    for (filter, plain) in [
        (StatusFilter::Open, "Open"),
        (StatusFilter::Draft, "Draft"),
        (StatusFilter::Merged, "Merged"),
        (StatusFilter::Declined, "Declined"),
        (StatusFilter::All, "All"),
    ] {
        assert_eq!(filter.title(false), plain);
        let more = filter.title(true);
        assert!(
            more.contains("recent") || more.contains("more unread"),
            "{more}"
        );
    }

    let mut merged_view = state("me");
    merged_view.ui.list.filter = StatusFilter::Merged;
    set_more(&mut merged_view, PrGroup::Merged, true);
    let text = drawn(&mut merged_view, 100);
    assert!(text.contains("Merged, recent (0)"), "{text}");
    assert!(text.contains("L: more"), "{text}");

    set_more(&mut merged_view, PrGroup::Merged, false);
    let text = drawn(&mut merged_view, 100);
    assert!(text.contains("Merged (0)"), "{text}");
    assert!(!text.contains("L: more"), "{text}");
}

#[test]
fn l_loads_older_only_where_it_would_show_something() {
    let press = |state: &AppState| {
        key_to_action(state, KeyEvent::new(KeyCode::Char('L'), KeyModifiers::NONE))
    };
    let mut merged = state("me");
    merged.ui.list.filter = StatusFilter::Merged;
    set_more(&mut merged, PrGroup::Merged, true);
    assert!(matches!(
        press(&merged),
        Some(Action::List(ListAction::LoadOlder))
    ));

    let mut open_view = state("me");
    set_more(&mut open_view, PrGroup::Merged, true);
    assert!(
        press(&open_view).is_none(),
        "the Open view shows no closed PRs, so there is nothing for it to load"
    );

    let mut exhausted = state("me");
    exhausted.ui.list.filter = StatusFilter::All;
    assert!(press(&exhausted).is_none(), "nothing older is left");
}

#[test]
fn help_lists_load_older_only_when_it_applies() {
    let help = |older: bool| {
        let mut state = state("me");
        state.ui.list.filter = StatusFilter::All;
        set_more(&mut state, PrGroup::Merged, older);
        state.ui.list.overlay = Some(ListOverlay::Help(HelpDialog::default()));
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        terminal.draw(|frame| render(frame, &mut state)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..40)
            .map(|y| {
                (0..100)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert!(help(true).contains("load more"), "{}", help(true));
    assert!(!help(false).contains("load more"), "{}", help(false));
}

#[test]
fn each_view_names_the_groups_it_shows() {
    assert_eq!(StatusFilter::Open.groups(), [PrGroup::Open]);
    assert_eq!(
        StatusFilter::Draft.groups(),
        [PrGroup::Open],
        "drafts come with the open ones"
    );
    assert_eq!(StatusFilter::Merged.groups(), [PrGroup::Merged]);
    assert_eq!(StatusFilter::Declined.groups(), [PrGroup::Declined]);
    assert_eq!(StatusFilter::All.groups(), PrGroup::ALL);
}

#[test]
fn a_view_still_being_read_says_so_rather_than_claiming_to_be_empty() {
    let mut merged = state("me");
    merged.ui.list.filter = StatusFilter::Merged;
    let idle = drawn(&mut merged, 100);
    assert!(idle.contains("No PRs in this view"), "{idle}");

    merged.store.fetches.insert(FetchKey::Prs(PrGroup::Merged));
    let loading = drawn(&mut merged, 100);
    assert!(loading.contains("Loading merged PRs"), "{loading}");
    assert!(!loading.contains("No PRs in this view"), "{loading}");

    // Another group loading does not make this view look busy.
    let mut merged = state("me");
    merged.ui.list.filter = StatusFilter::Merged;
    merged.store.fetches.insert(FetchKey::Prs(PrGroup::Open));
    assert!(drawn(&mut merged, 100).contains("No PRs in this view"));
}

#[test]
fn choosing_another_view_asks_the_app_to_read_it_and_choosing_the_same_one_does_not() {
    let prs = prs();
    let viewer = "me".into();
    let ctx = context(&prs, &viewer, false);
    let mut list = PrListScreen {
        overlay: Some(ListOverlay::FilterPicker {
            highlighted: StatusFilter::Merged,
        }),
        ..PrListScreen::default()
    };
    let action = list.update(ListAction::ApplyPicker, &ctx);
    assert!(matches!(action, Some(Effect::LoadView)));
    assert_eq!(list.filter, StatusFilter::Merged);
    assert!(list.overlay.is_none(), "applying closes the picker");

    // The picker opens on the filter in use; applying that reads nothing.
    list.update(ListAction::OpenFilterPicker, &ctx);
    let again = list.update(ListAction::ApplyPicker, &ctx);
    assert!(again.is_none(), "an unchanged filter needs nothing read");

    // With no picker open there is nothing to apply.
    assert!(list.update(ListAction::ApplyPicker, &ctx).is_none());
    assert_eq!(list.filter, StatusFilter::Merged);

    // The picker stops at its ends instead of running past them.
    list.update(ListAction::OpenFilterPicker, &ctx);
    for _ in 0..9 {
        list.update(ListAction::PickerNext, &ctx);
    }
    list.update(ListAction::ApplyPicker, &ctx);
    assert_eq!(list.filter, StatusFilter::All);
    list.update(ListAction::OpenFilterPicker, &ctx);
    for _ in 0..9 {
        list.update(ListAction::PickerPrev, &ctx);
    }
    list.update(ListAction::ApplyPicker, &ctx);
    assert_eq!(list.filter, StatusFilter::Open);
}

#[test]
fn the_help_and_keys_md_name_the_same_keys_for_the_list() {
    crate::doc_contract::assert_keys_match(
        render::HELP_KEYS.iter().map(|(keys, _)| *keys),
        "The list",
    );
}

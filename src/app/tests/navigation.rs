//! Keys, search, selection and links: where input goes and what it keeps.

use super::support::*;

#[test]
fn navigation_and_search_keep_the_same_keyboard_flow() {
    let mut app = app();
    press(&mut app, KeyCode::Char('/'));
    for c in "missing".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    let ctx = tui::screens::pr_list::ListContext::from_store(
        &app.state.store,
        app.state.ui.list.filter,
        app.state.screen,
    );
    assert!(app.state.ui.list.filtered_prs(&ctx).is_empty());
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        app.state.screen,
        Screen::Detail {
            pr_id: 42,
            tab: DetailTab::Description
        }
    );
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(
        app.state.screen,
        Screen::Detail {
            pr_id: 42,
            tab: DetailTab::Overview
        }
    );
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.state.screen, Screen::List);
}

#[test]
fn filter_picker_applies_and_cancels_without_leaking_keys() {
    let mut app = app();
    press(&mut app, KeyCode::Char('f'));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Esc);
    assert_eq!(
        app.state.ui.list.filter,
        tui::screens::pr_list::StatusFilter::Open
    );
    press(&mut app, KeyCode::Char('f'));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        app.state.ui.list.filter,
        tui::screens::pr_list::StatusFilter::Draft
    );
    assert_eq!(app.state.ui.list.selected, 0);
}

#[test]
fn diff_search_focus_and_match_wrapping_are_local() {
    let mut app = app();
    detail(&mut app, DetailTab::Diff);
    press(&mut app, KeyCode::Char('/'));
    for c in "main".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    assert_eq!(app.state.ui.detail.diff.focused_file, 0);
    press(&mut app, KeyCode::Esc);
    // root directory first; move to the file before entering the pane
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.state.ui.detail.diff.focus, DiffFocus::Pane);
    app.state.ui.detail.diff.pane.matches = vec![2, 5];
    app.state.ui.detail.diff.pane_search.query = "new".into();
    press(&mut app, KeyCode::Char('n'));
    assert_eq!(app.state.ui.detail.diff.pane_cursor, 2);
    press(&mut app, KeyCode::Char('N'));
    assert_eq!(app.state.ui.detail.diff.pane_cursor, 5);
    press(&mut app, KeyCode::Esc);
    assert!(app.state.ui.detail.diff.pane_search.query.is_empty());
    assert_eq!(app.state.ui.detail.diff.focus, DiffFocus::Pane);
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.state.ui.detail.diff.focus, DiffFocus::Tree);
}

#[test]
fn commit_drilldown_uses_an_independent_diff_instance() {
    let mut app = app();
    detail(&mut app, DetailTab::Diff);
    app.state.ui.detail.diff.pane_cursor = 7;
    let data = app.state.store.cache.details.get_mut(&42).unwrap();
    let LoadState::Loaded(diff) = &data.diff else {
        panic!()
    };
    data.commit_diffs
        .insert("abcdef123456".into(), LoadState::Loaded(diff.clone()));
    app.apply(Action::Detail(DetailAction::Nav(NavAction::SelectTab(
        DetailTab::Commits,
    ))));
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        app.state.ui.detail.commits.open_commit(),
        Some("abcdef123456")
    );
    app.state
        .ui
        .detail
        .commits
        .diff_mut()
        .unwrap()
        .pane
        .item_count = 5;
    app.apply(Action::Diff(DiffAction::MovePaneCursor(2)));
    assert_eq!(app.state.ui.detail.commits.diff().unwrap().pane_cursor, 2);
    assert_eq!(app.state.ui.detail.diff.pane_cursor, 7);
    press(&mut app, KeyCode::Esc);
    assert!(app.state.ui.detail.commits.open_commit().is_none());
    assert_eq!(app.state.ui.detail.diff.pane_cursor, 7);
}

#[test]
fn commit_component_emits_a_pr_scoped_load_request() {
    let mut app = app();
    let ctx = tui::screens::pr_detail::tabs::commits::CommitInput::new(
        42,
        app.state.store.cache.details.get(&42),
    );
    let effect = app
        .state
        .ui
        .detail
        .commits
        .update(CommitsAction::Open, &ctx);
    assert!(
        matches!(effect, Some(Effect::LoadCommitDiff { pr_id: 42, oid }) if oid == "abcdef123456")
    );
    assert!(
        app.state
            .ui
            .detail
            .commits
            .update(CommitsAction::StepCommit(1), &ctx)
            .is_none()
    );
}

#[test]
fn overview_navigation_resets_reply_selection() {
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    app.state.ui.detail.overview.timeline.item_count = 3;
    app.state.ui.detail.overview.timeline.sub = 2;
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.state.ui.detail.overview.timeline.cursor, 1);
    assert_eq!(app.state.ui.detail.overview.timeline.sub, 0);
    app.state.ui.detail.overview.timeline.item_count = 1;
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.state.ui.detail.overview.timeline.scroll, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn rapid_keys_open_the_latest_selection_and_capture_editor_text() {
    let mut app = app();
    if let LoadState::Loaded(prs) = &mut app.state.store.cache.prs {
        let mut second = prs[0].clone();
        second.id = 43;
        let mut third = prs[0].clone();
        third.id = 44;
        prs.extend([second, third]);
    }
    for key in [
        KeyCode::Char('j'),
        KeyCode::Char('j'),
        KeyCode::Enter,
        KeyCode::Char('2'),
        KeyCode::Char('c'),
        KeyCode::Char('q'),
    ] {
        assert_eq!(
            app.handle_key(KeyEvent::new(key, KeyModifiers::NONE)),
            Next::Continue
        );
    }
    assert!(matches!(app.state.screen, Screen::Detail { pr_id: 44, .. }));
    assert_eq!(app.state.ui.detail.editor.text().unwrap(), "q");
}

#[test]
fn help_in_both_screens_captures_keys_and_restores_navigation() {
    for tab in [None, Some(DetailTab::Overview)] {
        let mut app = app();
        if let Some(tab) = tab {
            detail(&mut app, tab);
        }
        let screen = app.state.screen;
        press(&mut app, KeyCode::Char('?'));
        assert!(app.state.ui.modal_open(&app.state.store, screen));
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(40, 12)).unwrap();
        terminal.draw(|f| tui::render(f, &mut app.state)).unwrap();
        let selected = app.state.ui.list.selected;
        for ch in ['/', 'c', 'm', 'j'] {
            press(&mut app, KeyCode::Char(ch));
        }
        assert!(!app.state.ui.list.search.open);
        assert!(!app.state.ui.detail.editor.has_draft());
        assert!(app.state.ui.detail.merge_picker().is_none());
        assert_eq!(app.state.ui.list.selected, selected);
        press(&mut app, KeyCode::Esc);
        assert!(!app.state.ui.modal_open(&app.state.store, screen));
        assert_eq!(app.state.screen, screen);
    }
}

#[test]
fn invalid_link_is_rejected_before_starting_desktop_work() {
    let mut app = app();
    if let LoadState::Loaded(prs) = &mut app.state.store.cache.prs {
        prs[0].url = Some("file:///tmp/local".into());
    }
    app.apply(Action::Effect(Effect::PrLink {
        pr_id: 42,
        kind: LinkAction::Open,
    }));
    assert!(!app.state.store.link_pending);
    assert_eq!(
        app.state.store.notice.as_ref().unwrap().kind,
        crate::app::store::NoticeKind::Error
    );
    assert!(app.state.store.operations.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn link_completion_keeps_navigation_and_reports_failure_without_blocking_pr_work() {
    let mut app = app();
    app.apply(Action::Effect(Effect::PrLink {
        pr_id: 42,
        kind: LinkAction::Copy,
    }));
    assert!(app.state.store.link_pending);
    detail(&mut app, DetailTab::Overview);
    app.apply_result(TaskResult::LinkFinished(Err(
        "PR #42: clipboard unavailable".into(),
    )));
    assert!(!app.state.store.link_pending);
    assert_eq!(
        app.state.store.notice.as_ref().unwrap().kind,
        crate::app::store::NoticeKind::Error
    );
    assert!(app.state.store.errors.is_empty());
    assert!(app.state.store.operations.is_empty());
    assert_eq!(
        app.state.screen,
        Screen::Detail {
            pr_id: 42,
            tab: DetailTab::Overview
        }
    );
}

#[test]
fn refreshed_lists_keep_pr_and_commit_identity() {
    let mut app = app();
    let LoadState::Loaded(mut prs) = std::mem::take(&mut app.state.store.cache.prs) else {
        panic!()
    };
    let mut second = prs[0].clone();
    second.id = 43;
    prs.push(second);
    app.state.store.cache.prs = LoadState::Loaded(prs.clone());
    app.state.ui.list.selected = 1;
    let mut new = prs[0].clone();
    new.id = 44;
    prs.insert(0, new);
    app.apply_result(TaskResult::Read(Read::Prs {
        group: crate::domain::pr::PrGroup::Open,
        after: None,
        result: Ok(crate::domain::pr::PrBatch { prs, more: None }),
    }));
    assert_eq!(app.state.ui.list.selected, 2);
    detail(&mut app, DetailTab::Commits);
    let data = app.state.store.cache.details.get_mut(&42).unwrap();
    let LoadState::Loaded(mut commits) = std::mem::take(&mut data.commits) else {
        panic!()
    };
    let mut second = commits[0].clone();
    second.oid = "second".into();
    commits.push(second);
    data.commits = LoadState::Loaded(commits.clone());
    app.state.ui.detail.commits.selected = 1;
    let mut new = commits[0].clone();
    new.oid = "new".into();
    commits.insert(0, new);
    app.apply_result(TaskResult::Read(Read::Commits(42, Ok(commits))));
    assert_eq!(app.state.ui.detail.commits.selected, 2);
}

#[test]
fn returning_to_a_pr_restores_its_tab_focus_and_search() {
    let mut app = app();
    detail(&mut app, DetailTab::Diff);
    app.state.ui.detail.diff.focus = DiffFocus::Pane;
    app.state.ui.detail.diff.pane_search.query = "needle".into();
    app.state.ui.detail.diff.pane_scroll = 12;
    app.state.ui.detail.diff.pane_cursor = 5;
    app.state.ui.detail.overview.timeline.scroll = 7;
    let mut other = tui::regression_tests::fixture();
    app.state
        .store
        .cache
        .details
        .insert(43, other.store.cache.details.remove(&42).unwrap());
    app.apply(Action::Effect(Effect::Navigate(Screen::List)));
    add_pr(&mut app, 43);
    app.apply(Action::List(ListAction::OpenPr(43)));
    assert_eq!(
        app.state.screen,
        Screen::Detail {
            pr_id: 43,
            tab: DetailTab::Description
        }
    );
    app.apply(Action::List(ListAction::OpenPr(42)));
    assert_eq!(
        app.state.screen,
        Screen::Detail {
            pr_id: 42,
            tab: DetailTab::Diff
        }
    );
    assert_eq!(app.state.ui.detail.diff.focus, DiffFocus::Pane);
    assert_eq!(app.state.ui.detail.diff.pane_search.query, "needle");
    assert_eq!(app.state.ui.detail.diff.pane_scroll, 12);
    assert_eq!(app.state.ui.detail.overview.timeline.scroll, 7);
}

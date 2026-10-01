//! Reads: refresh bookkeeping, failed reloads and what capabilities switch off.

use super::support::*;

#[test]
fn refresh_failure_preserves_visible_data() {
    let mut app = app();
    app.apply(Action::Loaded(LoadedAction::Prs {
        group: crate::domain::pr::PrGroup::Open,
        after: None,
        result: Err("offline".into()),
    }));
    assert!(matches!(&app.state.store.cache.prs, LoadState::Loaded(prs) if prs.len() == 1));
}

#[tokio::test(flavor = "current_thread")]
async fn refresh_tracks_each_resource_and_refetches_after_mutation() {
    use crate::app::store::{FetchKey, Operation};
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    app.apply(Action::Refresh);
    app.apply(Action::Refresh);
    assert_eq!(app.state.store.fetches.len(), 4);
    assert!(app.state.store.refreshing(app.state.screen));
    app.apply(Action::Loaded(LoadedAction::Prs {
        group: crate::domain::pr::PrGroup::Open,
        after: None,
        result: Err("offline".into()),
    }));
    assert!(app.state.store.refreshing(app.state.screen));
    app.state.store.operations.insert(42, Operation::Moderation);
    app.apply(Action::Loaded(LoadedAction::Commented(42, Ok(()))));
    assert!(
        app.state
            .store
            .reload_after_fetch
            .contains(&FetchKey::Activity(42))
    );
    app.apply(Action::Loaded(LoadedAction::Activity(
        42,
        Err("old response".into()),
    )));
    assert!(app.state.store.fetches.contains(&FetchKey::Activity(42)));
    assert!(
        !app.state
            .store
            .reload_after_fetch
            .contains(&FetchKey::Activity(42))
    );
    app.apply(Action::Loaded(LoadedAction::Activity(
        42,
        Err("new response".into()),
    )));
    assert!(!app.state.store.fetches.contains(&FetchKey::Activity(42)));
    app.apply(Action::Loaded(LoadedAction::Mergeability(
        42,
        Err("old response".into()),
    )));
    app.apply(Action::Loaded(LoadedAction::Mergeability(
        42,
        Err("new response".into()),
    )));
    app.apply(Action::Loaded(LoadedAction::Prs {
        group: crate::domain::pr::PrGroup::Open,
        after: None,
        result: Err("offline".into()),
    }));
    app.apply(Action::Loaded(LoadedAction::Info(
        42,
        Err("offline".into()),
    )));
    assert!(!app.state.store.refreshing(app.state.screen));
    assert!(matches!(app.state.store.cache.prs, LoadState::Loaded(_)));
}

#[test]
fn read_only_capabilities_block_shortcuts_commands_and_optional_loads() {
    use crate::domain::{capabilities::Capabilities, pr::MergeStrategy, review::ReviewVerdict};
    let mut app = app();
    app.state.store.capabilities = Capabilities::default();
    let data = app.state.store.cache.details.get_mut(&42).unwrap();
    data.builds = LoadState::NotRequested;
    data.mergeability = LoadState::NotRequested;
    detail(&mut app, DetailTab::Overview);
    for ch in ['c', 'r', 'a', 'v', 'm', 'x', 'e', 'd', 'R', '5'] {
        press(&mut app, KeyCode::Char(ch));
    }
    assert!(app.state.ui.detail.editor.draft.is_none());
    assert!(app.state.ui.detail.review_picker.is_none());
    assert!(app.state.ui.detail.merge_picker.is_none());
    assert!(app.state.ui.detail.confirm.is_none());
    assert_eq!(
        app.state.screen,
        Screen::Detail {
            pr_id: 42,
            tab: DetailTab::Overview
        }
    );
    let data = &app.state.store.cache.details[&42];
    assert!(matches!(data.builds, LoadState::NotRequested));
    assert!(matches!(data.mergeability, LoadState::NotRequested));
    detail(&mut app, DetailTab::Commits);
    app.apply(Action::Detail(DetailAction::NextTab));
    assert_eq!(
        app.state.screen,
        Screen::Detail {
            pr_id: 42,
            tab: DetailTab::Description
        }
    );
    app.apply(Action::Detail(DetailAction::PrevTab));
    assert_eq!(
        app.state.screen,
        Screen::Detail {
            pr_id: 42,
            tab: DetailTab::Commits
        }
    );
    for command in [
        Command::StartReview,
        Command::SubmitReview {
            verdict: ReviewVerdict::Comment,
            body: "summary".into(),
        },
        Command::SubmitComment {
            target: CommentTarget::Pr,
            text: "comment".into(),
        },
        Command::Merge(MergeStrategy::Merge),
        Command::Decline,
        Command::DeleteComment {
            id: 1,
            review: false,
        },
        Command::ResolveThread {
            node_id: Some("thread".into()),
            comment_id: Some(1),
            resolved: true,
        },
    ] {
        app.state.store.errors.clear();
        app.execute(42, command);
        assert!(app.state.store.errors[&42].contains("not supported"));
        assert!(app.state.store.operations.is_empty());
        assert!(app.state.store.reviews.is_empty());
    }
}

#[test]
fn failed_refresh_marks_cached_data_until_that_resource_recovers() {
    let mut app = app();
    detail(&mut app, DetailTab::Diff);
    app.apply(Action::Loaded(LoadedAction::Diff(
        42,
        Err("offline".into()),
    )));
    assert!(app.state.store.refresh_failed(app.state.screen));
    assert!(matches!(
        app.state.store.cache.details[&42].diff,
        LoadState::Loaded(_)
    ));
    assert!(!app.state.store.refresh_failed(Screen::Detail {
        pr_id: 43,
        tab: DetailTab::Diff
    }));
    app.apply(Action::Loaded(LoadedAction::Builds(42, Ok(vec![]))));
    assert!(app.state.store.refresh_failed(app.state.screen));
    let diff = match &app.state.store.cache.details[&42].diff {
        LoadState::Loaded(diff) => diff.clone(),
        other => panic!("the diff is not loaded: {other:?}"),
    };
    app.apply(Action::Loaded(LoadedAction::Diff(42, Ok(diff))));
    assert!(!app.state.store.refresh_failed(app.state.screen));
}

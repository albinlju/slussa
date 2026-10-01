//! Reads: refresh bookkeeping, failed reloads and what capabilities switch off.

use super::support::*;

#[test]
fn refresh_failure_preserves_visible_data() {
    let mut app = app();
    app.apply_result(TaskResult::Read(Read::Prs {
        group: crate::domain::pr::PrGroup::Open,
        after: None,
        result: Err(failed("offline")),
    }));
    assert!(matches!(&app.state.store.cache.prs, LoadState::Loaded(prs) if prs.len() == 1));
}

#[tokio::test(flavor = "current_thread")]
async fn refresh_tracks_each_resource_and_refetches_after_mutation() {
    use crate::app::store::{FetchKey, Operation};
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    app.apply(Action::Effect(Effect::Refresh));
    app.apply(Action::Effect(Effect::Refresh));
    assert_eq!(app.state.store.fetches.len(), 4);
    assert!(app.state.store.refreshing(app.state.screen));
    app.apply_result(TaskResult::Read(Read::Prs {
        group: crate::domain::pr::PrGroup::Open,
        after: None,
        result: Err(failed("offline")),
    }));
    assert!(app.state.store.refreshing(app.state.screen));
    app.state
        .store
        .operations
        .insert(PrId(42), Operation::Moderation);
    finish_write(&mut app, PrId(42), Ok(()));
    assert!(
        app.state
            .store
            .reload_after_fetch
            .contains(&FetchKey::Activity(PrId(42)))
    );
    app.apply_result(TaskResult::Read(Read::Activity(
        PrId(42),
        Err(failed("old response")),
    )));
    assert!(
        app.state
            .store
            .fetches
            .contains(&FetchKey::Activity(PrId(42)))
    );
    assert!(
        !app.state
            .store
            .reload_after_fetch
            .contains(&FetchKey::Activity(PrId(42)))
    );
    app.apply_result(TaskResult::Read(Read::Activity(
        PrId(42),
        Err(failed("new response")),
    )));
    assert!(
        !app.state
            .store
            .fetches
            .contains(&FetchKey::Activity(PrId(42)))
    );
    app.apply_result(TaskResult::Read(Read::Mergeability(
        PrId(42),
        Err(failed("old response")),
    )));
    app.apply_result(TaskResult::Read(Read::Mergeability(
        PrId(42),
        Err(failed("new response")),
    )));
    app.apply_result(TaskResult::Read(Read::Prs {
        group: crate::domain::pr::PrGroup::Open,
        after: None,
        result: Err(failed("offline")),
    }));
    app.apply_result(TaskResult::Read(Read::Info(
        PrId(42),
        Err(failed("offline")),
    )));
    assert!(!app.state.store.refreshing(app.state.screen));
    assert!(matches!(app.state.store.cache.prs, LoadState::Loaded(_)));
}

#[test]
fn read_only_capabilities_block_shortcuts_commands_and_optional_loads() {
    use crate::domain::{capabilities::Capabilities, pr::MergeStrategy, review::ReviewVerdict};
    let mut app = app();
    app.state.store.capabilities = Capabilities::default();
    let data = app.state.store.cache.details.get_mut(&PrId(42)).unwrap();
    data.builds = LoadState::NotRequested;
    data.mergeability = LoadState::NotRequested;
    detail(&mut app, DetailTab::Overview);
    for ch in ['c', 'r', 'a', 'v', 'm', 'x', 'e', 'd', 'R', '5'] {
        press(&mut app, KeyCode::Char(ch));
    }
    assert!(!app.state.ui.detail.editor.has_draft());
    assert!(app.state.ui.detail.review_picker().is_none());
    assert!(app.state.ui.detail.merge_picker().is_none());
    assert!(app.state.ui.detail.confirm().is_none());
    assert_eq!(
        app.state.screen,
        Screen::Detail {
            pr_id: PrId(42),
            tab: DetailTab::Overview
        }
    );
    let data = &app.state.store.cache.details[&PrId(42)];
    assert!(matches!(data.builds, LoadState::NotRequested));
    assert!(matches!(data.mergeability, LoadState::NotRequested));
    detail(&mut app, DetailTab::Commits);
    app.apply(Action::Detail(DetailAction::Nav(NavAction::NextTab)));
    assert_eq!(
        app.state.screen,
        Screen::Detail {
            pr_id: PrId(42),
            tab: DetailTab::Description
        }
    );
    app.apply(Action::Detail(DetailAction::Nav(NavAction::PrevTab)));
    assert_eq!(
        app.state.screen,
        Screen::Detail {
            pr_id: PrId(42),
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
        Command::DeleteComment(CommentKey {
            id: 1,
            kind: CommentKind::Conversation,
        }),
        Command::ResolveThread {
            thread: ThreadHandle::NodeId("thread".into()),
            resolved: true,
        },
    ] {
        app.state.store.errors.clear();
        app.execute(PrId(42), command);
        assert!(app.state.store.errors[&PrId(42)].contains("not supported"));
        assert!(app.state.store.operations.is_empty());
        assert!(app.state.store.reviews.is_empty());
    }
}

#[test]
fn failed_refresh_marks_cached_data_until_that_resource_recovers() {
    let mut app = app();
    detail(&mut app, DetailTab::Diff);
    app.apply_result(TaskResult::Read(Read::Diff(
        PrId(42),
        Err(failed("offline")),
    )));
    assert!(app.state.store.refresh_failed(app.state.screen));
    assert!(matches!(
        app.state.store.cache.details[&PrId(42)].diff,
        LoadState::Loaded(_)
    ));
    assert!(!app.state.store.refresh_failed(Screen::Detail {
        pr_id: PrId(43),
        tab: DetailTab::Diff
    }));
    app.apply_result(TaskResult::Read(Read::Builds(PrId(42), Ok(vec![]))));
    assert!(app.state.store.refresh_failed(app.state.screen));
    let diff = match &app.state.store.cache.details[&PrId(42)].diff {
        LoadState::Loaded(diff) => diff.clone(),
        other => panic!("the diff is not loaded: {other:?}"),
    };
    app.apply_result(TaskResult::Read(Read::Diff(PrId(42), Ok(diff))));
    assert!(!app.state.store.refresh_failed(app.state.screen));
}

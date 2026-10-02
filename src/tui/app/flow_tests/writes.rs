//! Comments and reopening: sent once, reported on their PR, then refetched.

use super::support::*;

fn submit_pr_comment(app: &mut App, pr_id: PrId, text: &str) {
    app.apply(Action::Effect(Effect::Command {
        pr_id,
        command: Command::SubmitComment {
            target: CommentTarget::Pr,
            text: text.into(),
        },
    }));
}

#[tokio::test]
async fn a_comment_is_sent_once_and_followed_by_a_refetch() {
    let gh = FakeGh::new()
        .on("issues/1/comments", "{}")
        .on(OPEN_QUERY, &one_pr_page())
        .on("graphql", &any_connection())
        .install();
    let mut app = app();

    submit_pr_comment(&mut app, PrId(1), "hello");
    assert!(
        app.state.store.operations.contains_key(&PrId(1)),
        "the write is pending until the provider answers"
    );
    settle(&mut app).await;

    let calls = gh.calls();
    assert_eq!(
        calls[0],
        "api --method POST repos/{owner}/{repo}/issues/1/comments -f body=hello"
    );
    assert_eq!(
        calls
            .iter()
            .filter(|c| c.contains("issues/1/comments"))
            .count(),
        1,
        "{calls:?}"
    );
    for (what, needle) in [
        ("the activity", "comments(first: 100"),
        ("the PR list", "states: OPEN"),
        ("mergeability", "mergeable"),
    ] {
        assert!(
            calls.iter().any(|c| c.contains(needle)),
            "{what} is refetched after the write: {calls:?}"
        );
    }
    assert!(app.state.store.errors.is_empty());
    assert!(app.state.store.uncertain_submissions.is_empty());
    assert!(app.state.store.operations.is_empty());
}

#[tokio::test]
async fn a_failed_comment_is_reported_on_its_pr_and_not_followed_by_a_refetch() {
    let gh = FakeGh::new()
        .fail(
            "issues/1/comments",
            1,
            "gh: HTTP 500: Internal Server Error",
        )
        .install();
    let mut app = app();

    submit_pr_comment(&mut app, PrId(1), "hello");
    settle(&mut app).await;

    assert_eq!(gh.calls().len(), 1, "no refetch after a failed write");
    assert!(app.state.store.errors.contains_key(&PrId(1)));
    assert!(
        app.state.store.uncertain_submissions.contains(&PrId(1)),
        "an ambiguous failure is remembered so the user checks before retrying"
    );
    assert!(app.state.store.operations.is_empty());
}

#[tokio::test]
async fn a_second_write_to_the_same_pr_is_ignored_while_one_is_pending() {
    let gh = FakeGh::new()
        .on("issues/1/comments", "{}")
        .on(OPEN_QUERY, &one_pr_page())
        .on("graphql", &any_connection())
        .install();
    let mut app = app();

    submit_pr_comment(&mut app, PrId(1), "first");
    submit_pr_comment(&mut app, PrId(1), "second");
    settle(&mut app).await;

    let posts: Vec<_> = gh
        .calls()
        .into_iter()
        .filter(|c| c.contains("issues/1/comments"))
        .collect();
    assert_eq!(posts.len(), 1, "{posts:?}");
    assert!(posts[0].ends_with("body=first"));
}

fn reopen(app: &mut App, pr_id: PrId) {
    app.apply(Action::Effect(Effect::Command {
        pr_id,
        command: Command::Reopen,
    }));
}

#[tokio::test]
async fn reopening_is_sent_once_reported_and_followed_by_a_refetch() {
    let gh = FakeGh::new()
        .on("pulls/1 -f state=open", "{}")
        .on(OPEN_QUERY, &one_pr_page())
        .on("graphql", &any_connection())
        .install();
    let mut app = app();

    reopen(&mut app, PrId(1));
    assert!(
        app.state.store.operations.contains_key(&PrId(1)),
        "pending until the provider answers"
    );
    settle(&mut app).await;

    let calls = gh.calls();
    assert_eq!(
        calls[0],
        "api --method PATCH repos/{owner}/{repo}/pulls/1 -f state=open"
    );
    assert_eq!(calls.iter().filter(|c| c.contains("state=open")).count(), 1);
    assert!(
        calls.iter().any(|c| c.contains(OPEN_QUERY)),
        "the list is refetched so the PR shows as open: {calls:?}"
    );
    assert_eq!(
        app.state.store.notice.as_ref().map(|n| n.message.as_str()),
        Some("PR #1 · reopened")
    );
    assert!(app.state.store.errors.is_empty());
    assert!(app.state.store.operations.is_empty());
}

#[tokio::test]
async fn a_refused_reopen_is_reported_on_its_pr_without_a_refetch() {
    let gh = FakeGh::new()
        .fail(
            "pulls/1 -f state=open",
            1,
            "gh: Validation Failed: the head branch was deleted (HTTP 422)",
        )
        .install();
    let mut app = app();

    reopen(&mut app, PrId(1));
    settle(&mut app).await;

    assert_eq!(gh.calls().len(), 1, "no refetch after a failed write");
    assert!(
        app.state.store.errors[&PrId(1)].contains("head branch was deleted"),
        "{:?}",
        app.state.store.errors
    );
    assert!(app.state.store.operations.is_empty());
}

/// A line of a diff whose revision was not read when the comment was made.
fn anchor_without_a_revision() -> crate::tui::app::reviews::CommentAnchor {
    crate::tui::app::reviews::CommentAnchor {
        revision: None,
        path: "src/lib.rs".into(),
        line: 3,
        removed: false,
    }
}

#[tokio::test]
async fn a_line_comment_on_an_unknown_revision_is_refused_before_anything_is_sent() {
    let gh = FakeGh::new().on("api", "{}").install();
    let mut app = app();

    app.apply(Action::Effect(Effect::Command {
        pr_id: PrId(1),
        command: Command::SubmitComment {
            target: CommentTarget::Line(anchor_without_a_revision()),
            text: "note".into(),
        },
    }));
    settle(&mut app).await;

    assert!(gh.calls().is_empty(), "{:?}", gh.calls());
    let error = &app.state.store.errors[&PrId(1)];
    assert!(
        error.contains("Reload the diff before commenting"),
        "{error}"
    );
    assert!(
        app.state.store.uncertain_submissions.is_empty(),
        "nothing left the machine, so there is nothing to check before retrying"
    );
}

#[tokio::test]
async fn a_review_with_a_comment_on_an_unknown_revision_is_refused_whole() {
    use crate::tui::app::reviews::{PendingComment, PendingReview};
    let gh = FakeGh::new().on("api", "{}").install();
    let mut app = app();
    app.state.store.reviews.insert(
        PrId(1),
        PendingReview {
            submitted_summary: None,
            comments: vec![PendingComment {
                anchor: anchor_without_a_revision(),
                text: "queued".into(),
            }],
        },
    );

    app.apply(Action::Effect(Effect::Command {
        pr_id: PrId(1),
        command: Command::SubmitReview {
            verdict: crate::domain::review::ReviewVerdict::Comment,
            body: "summary".into(),
        },
    }));
    settle(&mut app).await;

    assert!(gh.calls().is_empty(), "{:?}", gh.calls());
    let error = &app.state.store.errors[&PrId(1)];
    assert!(error.contains("recreate comments"), "{error}");
    assert!(app.state.store.uncertain_submissions.is_empty());
    assert_eq!(
        app.state.store.reviews[&PrId(1)].comments.len(),
        1,
        "the queued comment is kept"
    );
}

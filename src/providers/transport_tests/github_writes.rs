//! GitHub writes through a fake `gh`: merge, close, reopen, comment, review.

use super::support::*;

#[test]
fn github_merge_sends_the_chosen_strategy() {
    let installed = FakeGh::new().on("api", "{}").install();
    Provider::GitHub
        .merge(PrId(7), MergeStrategy::Squash)
        .unwrap();

    assert_eq!(
        installed.calls(),
        vec!["api --method PUT repos/{owner}/{repo}/pulls/7/merge -f merge_method=squash"]
    );
}

#[test]
fn github_decline_closes_the_pull_request() {
    let installed = FakeGh::new().on("api", "{}").install();
    Provider::GitHub.decline(PrId(7)).unwrap();

    assert_eq!(
        installed.calls(),
        vec!["api --method PATCH repos/{owner}/{repo}/pulls/7 -f state=closed"]
    );
}

#[test]
fn github_pr_comment_passes_the_body_as_a_literal_argument() {
    let installed = FakeGh::new().on("api", "{}").install();
    let body = "thanks $(whoami) `id` \"quoted\" & more";
    Provider::GitHub.post_pr_comment(PrId(7), body).unwrap();

    assert_eq!(
        installed.calls(),
        vec![format!(
            "api --method POST repos/{{owner}}/{{repo}}/issues/7/comments -f body={body}"
        )]
    );
}

#[test]
fn github_batched_review_is_one_call_with_the_comments_on_stdin() {
    let installed = FakeGh::new().on("api", "{}").install();
    let comments = [
        review_comment("abc", 3, false),
        review_comment("abc", 9, true),
    ];
    Provider::GitHub
        .submit_full_review(
            PrId(7),
            ReviewVerdict::RequestChanges,
            "please fix",
            "me",
            &comments,
        )
        .unwrap();

    assert_eq!(
        installed.calls(),
        vec!["api --method POST repos/{owner}/{repo}/pulls/7/reviews --input -"]
    );
    let sent: Value = serde_json::from_str(&installed.stdin()).unwrap();
    assert_eq!(
        sent,
        json!({
            "commit_id": "abc",
            "event": "REQUEST_CHANGES",
            "body": "please fix",
            "comments": [
                {"path": "src/lib.rs", "line": 3, "side": "RIGHT", "body": "note on line 3"},
                {"path": "src/lib.rs", "line": 9, "side": "LEFT", "body": "note on line 9"}
            ]
        })
    );
}

#[test]
fn github_batched_review_refuses_unsafe_batches_without_calling_gh() {
    let installed = FakeGh::new().on("api", "{}").install();
    let mixed = [
        review_comment("abc", 1, false),
        review_comment("def", 2, false),
    ];
    let result =
        Provider::GitHub.submit_full_review(PrId(7), ReviewVerdict::Comment, "", "me", &mixed);
    assert!(
        matches!(
            result,
            Err(ReviewError::Failed(FetchError::InvalidInput(_)))
        ),
        "{result:?}"
    );
    assert!(installed.calls().is_empty(), "{:?}", installed.calls());
}

#[test]
fn github_reopen_sets_the_state_back_to_open() {
    let installed = FakeGh::new().on("api", "{}").install();
    Provider::GitHub.reopen(PrId(7)).unwrap();

    assert_eq!(
        installed.calls(),
        vec!["api --method PATCH repos/{owner}/{repo}/pulls/7 -f state=open"]
    );
}

#[test]
fn github_refusing_a_reopen_reaches_the_user_in_githubs_words() {
    let _installed = FakeGh::new()
        .fail(
            "api",
            1,
            "gh: Validation Failed: the head branch was deleted (HTTP 422)",
        )
        .install();
    let error = Provider::GitHub.reopen(PrId(7)).unwrap_err();

    assert!(matches!(error, FetchError::GhFailed { .. }), "{error:?}");
    assert!(
        error.user_message().contains("head branch was deleted"),
        "{}",
        error.user_message()
    );
}

#[test]
fn github_edits_and_deletes_a_comment_where_its_kind_lives() {
    use crate::domain::comment::{CommentId, CommentKey, CommentKind};
    let installed = FakeGh::new().on("api", "{}").install();
    let review = CommentKey {
        id: CommentId(11),
        kind: CommentKind::Review,
    };
    let conversation = CommentKey {
        id: CommentId(12),
        kind: CommentKind::Conversation,
    };
    Provider::GitHub
        .edit_comment(PrId(7), review, "new")
        .unwrap();
    Provider::GitHub
        .edit_comment(PrId(7), conversation, "new")
        .unwrap();
    Provider::GitHub.delete_comment(PrId(7), review).unwrap();
    Provider::GitHub
        .delete_comment(PrId(7), conversation)
        .unwrap();

    assert_eq!(
        installed.calls(),
        vec![
            "api --method PATCH repos/{owner}/{repo}/pulls/comments/11 -f body=new",
            "api --method PATCH repos/{owner}/{repo}/issues/comments/12 -f body=new",
            "api --method DELETE repos/{owner}/{repo}/pulls/comments/11",
            "api --method DELETE repos/{owner}/{repo}/issues/comments/12",
        ]
    );
}

#[test]
fn github_resolves_a_thread_by_its_node_id_and_refuses_another_providers_handle() {
    use crate::domain::comment::{CommentId, ThreadHandle};
    let installed = FakeGh::new().on("api", "{}").install();
    Provider::GitHub
        .set_thread_resolved(PrId(7), &ThreadHandle::NodeId("PRRT_1".into()), true)
        .unwrap();
    let calls = installed.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(calls[0].contains("resolveReviewThread"), "{calls:?}");
    assert!(calls[0].contains("PRRT_1"), "{calls:?}");

    let refused = Provider::GitHub.set_thread_resolved(
        PrId(7),
        &ThreadHandle::RootComment(CommentId(3)),
        true,
    );
    assert!(matches!(refused, Err(FetchError::InvalidInput(_))));
    assert_eq!(installed.calls().len(), 1, "nothing was sent for it");
}

#[test]
fn github_auto_merge_asks_gh_for_the_chosen_strategy_and_off_disables_it() {
    let installed = FakeGh::new().on("pr", "").install();
    Provider::GitHub
        .set_auto_merge(PrId(7), Some(MergeStrategy::Squash))
        .unwrap();
    Provider::GitHub.set_auto_merge(PrId(7), None).unwrap();

    assert_eq!(
        installed.calls(),
        vec!["pr merge 7 --auto --squash", "pr merge 7 --disable-auto"]
    );
}

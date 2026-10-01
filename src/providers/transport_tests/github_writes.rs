//! GitHub writes through a fake `gh`: merge, close, reopen, comment, review.

use super::support::*;

#[test]
fn github_merge_sends_the_chosen_strategy() {
    let installed = FakeGh::new().on("api", "{}").install();
    Provider::GitHub.merge(7, MergeStrategy::Squash).unwrap();

    assert_eq!(
        installed.calls(),
        vec!["api --method PUT repos/{owner}/{repo}/pulls/7/merge -f merge_method=squash"]
    );
}

#[test]
fn github_decline_closes_the_pull_request() {
    let installed = FakeGh::new().on("api", "{}").install();
    Provider::GitHub.decline(7).unwrap();

    assert_eq!(
        installed.calls(),
        vec!["api --method PATCH repos/{owner}/{repo}/pulls/7 -f state=closed"]
    );
}

#[test]
fn github_pr_comment_passes_the_body_as_a_literal_argument() {
    let installed = FakeGh::new().on("api", "{}").install();
    let body = "thanks $(whoami) `id` \"quoted\" & more";
    Provider::GitHub.post_pr_comment(7, body).unwrap();

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
        review_comment(Some("abc"), 3, false),
        review_comment(Some("abc"), 9, true),
    ];
    Provider::GitHub
        .submit_full_review(
            7,
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
        review_comment(Some("abc"), 1, false),
        review_comment(Some("def"), 2, false),
    ];
    let unknown = [review_comment(None, 1, false)];
    for comments in [&mixed[..], &unknown[..]] {
        let result =
            Provider::GitHub.submit_full_review(7, ReviewVerdict::Comment, "", "me", comments);
        assert!(
            matches!(result, Err(FetchError::InvalidInput(_))),
            "{result:?}"
        );
    }
    assert!(installed.calls().is_empty(), "{:?}", installed.calls());
}

#[test]
fn github_reopen_sets_the_state_back_to_open() {
    let installed = FakeGh::new().on("api", "{}").install();
    Provider::GitHub.reopen(7).unwrap();

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
    let error = Provider::GitHub.reopen(7).unwrap_err();

    assert!(matches!(error, FetchError::GhFailed { .. }), "{error:?}");
    assert!(
        error.user_message().contains("head branch was deleted"),
        "{}",
        error.user_message()
    );
}

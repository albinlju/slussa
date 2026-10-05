//! Bitbucket Data Center reviews and merges through a loopback HTTP server:
//! what is sent, in what order, and what is refused when the branch has moved.

use super::support::*;

#[test]
fn bitbucket_review_posts_comments_then_summary_then_the_verdict() {
    let comments_target = format!("{PR_9}/comments");
    let server = MockHttp::start(vec![
        pr_9_now(HEAD),
        Route::post(&comments_target, 201, "{}"),
        Route::put(&format!("{PR_9}/participants/me"), 200, "{}"),
    ]);
    let comments = [
        review_comment("abc", 3, false),
        review_comment("abc", 9, true),
    ];
    bitbucket(&server)
        .submit_full_review(
            PrId(9),
            ReviewVerdict::Approve,
            "ship it",
            "me",
            &comments,
            &read_head(),
        )
        .unwrap();

    let requests = server.requests();
    let order: Vec<_> = requests
        .iter()
        .map(|r| format!("{} {}", r.method, r.target.rsplit('/').next().unwrap()))
        .collect();
    assert_eq!(
        order,
        [
            "GET 9",
            "POST comments",
            "POST comments",
            "POST comments",
            "PUT me"
        ]
    );
    let first: Value = serde_json::from_str(&requests[1].body).unwrap();
    assert_eq!(first["anchor"]["lineType"], "ADDED");
    assert_eq!(first["anchor"]["toHash"], "abc");
    let second: Value = serde_json::from_str(&requests[2].body).unwrap();
    assert_eq!(second["anchor"]["lineType"], "REMOVED");
    let summary: Value = serde_json::from_str(&requests[3].body).unwrap();
    assert_eq!(summary, json!({"text": "ship it"}));
    let verdict: Value = serde_json::from_str(&requests[4].body).unwrap();
    assert_eq!(
        verdict,
        json!({"status": "APPROVED", "lastReviewedCommit": HEAD}),
        "the approval says which commit it is of"
    );
}

#[test]
fn bitbucket_review_reports_how_many_comments_landed_before_a_failure() {
    let target = format!("{PR_9}/comments");
    let server = MockHttp::start(vec![
        pr_9_now(HEAD),
        Route::post(&target, 201, "{}").times(1),
        Route::post(
            &target,
            500,
            &json!({"errors": [{"message": "boom"}]}).to_string(),
        ),
    ]);
    let comments = [
        review_comment("abc", 3, false),
        review_comment("abc", 4, false),
        review_comment("abc", 5, false),
    ];
    let error = bitbucket(&server)
        .submit_full_review(
            PrId(9),
            ReviewVerdict::Approve,
            "ship it",
            "me",
            &comments,
            &read_head(),
        )
        .unwrap_err();

    match error {
        ReviewError::Partial {
            posted_comments,
            summary_posted,
            source,
        } => {
            assert_eq!(posted_comments, 1);
            assert!(!summary_posted);
            assert!(matches!(source, FetchError::HttpFailed { status: 500, .. }));
        }
        other @ ReviewError::Failed(_) => panic!("expected a partial review, got {other:?}"),
    }
    let requests = server.requests();
    assert_eq!(
        requests.len(),
        3,
        "the read, then stops at the first failure"
    );
    assert!(
        requests.iter().all(|r| r.method != "PUT"),
        "no verdict is sent"
    );
}

#[test]
fn bitbucket_verdict_alone_that_is_refused_is_a_plain_failure() {
    let refusal = json!({"errors": [{"message": "You cannot approve your own pull request"}]});
    let server = MockHttp::start(vec![
        pr_9_now(HEAD),
        Route::post(&format!("{PR_9}/comments"), 201, "{}"),
        Route::put(
            &format!("{PR_9}/participants/me"),
            409,
            &refusal.to_string(),
        ),
    ]);

    // No comments and no summary: the verdict is the only request.
    let error = bitbucket(&server)
        .submit_full_review(PrId(9), ReviewVerdict::Approve, "", "me", &[], &read_head())
        .unwrap_err();
    match &error {
        ReviewError::Failed(source) => assert!(
            !source.may_have_reached_server(),
            "a stated refusal changed nothing: {source:?}"
        ),
        ReviewError::Partial { .. } => panic!("nothing was sent in part: {error:?}"),
    }
    assert_eq!(server.requests().len(), 2, "the read and the verdict");

    // With a summary, the summary arrived and the verdict did not.
    let error = bitbucket(&server)
        .submit_full_review(
            PrId(9),
            ReviewVerdict::Approve,
            "ship it",
            "me",
            &[],
            &read_head(),
        )
        .unwrap_err();
    assert!(
        matches!(
            error,
            ReviewError::Partial {
                posted_comments: 0,
                summary_posted: true,
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn bitbucket_refuses_to_approve_a_branch_that_moved_before_posting_anything() {
    let server = MockHttp::start(vec![
        pr_9_now("def456"),
        Route::post(&format!("{PR_9}/comments"), 201, "{}"),
        Route::put(&format!("{PR_9}/participants/me"), 200, "{}"),
    ]);
    let comments = [review_comment("abc123", 3, false)];
    let error = bitbucket(&server)
        .submit_full_review(
            PrId(9),
            ReviewVerdict::Approve,
            "ship it",
            "me",
            &comments,
            &read_head(),
        )
        .unwrap_err();

    assert!(
        matches!(error, ReviewError::Failed(FetchError::Stale(_))),
        "{error:?}"
    );
    assert_eq!(
        server.requests().len(),
        1,
        "only the read: not a comment, not the approval"
    );
}

#[test]
fn bitbucket_refuses_to_merge_a_branch_that_moved_since_it_was_read() {
    let server = MockHttp::start(vec![
        pr_9_now("def456"),
        Route::post(&format!("{PR_9}/merge?version=3"), 200, "{}"),
    ]);
    let error = bitbucket(&server)
        .merge(PrId(9), MergeStrategy::Merge, None, &read_head())
        .unwrap_err();

    assert!(
        matches!(error, MergeError::Failed(FetchError::Stale(_))),
        "{error:?}"
    );
    assert_eq!(server.requests().len(), 1, "nothing was merged");
}

#[test]
fn bitbucket_refuses_a_request_for_changes_on_a_branch_that_moved_too() {
    let server = MockHttp::start(vec![
        pr_9_now("def456"),
        Route::put(&format!("{PR_9}/participants/me"), 200, "{}"),
    ]);
    let error = bitbucket(&server)
        .submit_full_review(
            PrId(9),
            ReviewVerdict::RequestChanges,
            "",
            "me",
            &[],
            &read_head(),
        )
        .unwrap_err();
    assert!(
        matches!(error, ReviewError::Failed(FetchError::Stale(_))),
        "{error:?}"
    );
    assert_eq!(server.requests().len(), 1, "only the read");
}

#[test]
fn bitbucket_lets_an_approval_be_withdrawn_after_a_push() {
    let server = MockHttp::start(vec![Route::put(
        &format!("{PR_9}/participants/me"),
        200,
        "{}",
    )]);
    bitbucket(&server)
        .submit_full_review(
            PrId(9),
            ReviewVerdict::Unapprove,
            "",
            "me",
            &[],
            &head_of("def456"),
        )
        .unwrap();
    let requests = server.requests();
    assert_eq!(
        requests.len(),
        1,
        "no read: the safe direction is not held back"
    );
    let sent: Value = serde_json::from_str(&requests[0].body).unwrap();
    assert_eq!(sent, json!({"status": "UNAPPROVED"}));
}

#[test]
fn bitbucket_a_user_name_is_kept_to_its_place_in_the_path() {
    let server = MockHttp::start(vec![Route::put(
        &format!("{PR_9}/participants/anna%2Fb%40corp"),
        200,
        "{}",
    )]);
    bitbucket(&server)
        .submit_full_review(
            PrId(9),
            ReviewVerdict::Unapprove,
            "",
            "anna/b@corp",
            &[],
            &read_head(),
        )
        .unwrap();
    assert_eq!(server.requests().len(), 1);
}

//! GitHub writes through a fake `gh`: merge, close, reopen, comment, review.

use super::support::*;
use crate::domain::pr::{AutoMerge, DeletableBranch, PullRequest, SourceRepo};

#[test]
fn github_merge_sends_the_chosen_strategy() {
    let installed = FakeGh::new().on("api", "{}").install();
    Provider::github_for_test()
        .merge(PrId(7), MergeStrategy::Squash, None, &read_head())
        .unwrap();

    assert_eq!(
        installed.calls(),
        vec![
            "api --method PUT repos/{owner}/{repo}/pulls/7/merge -f merge_method=squash -f sha=abc123"
        ]
    );
}

fn branch(name: &str) -> DeletableBranch {
    let pr = PullRequest {
        source_branch: name.into(),
        source_repo: SourceRepo::Same,
        target_branch: "main".into(),
        ..PullRequest::for_test(7, chrono::Utc::now())
    };
    DeletableBranch::of(&pr).expect("a branch of this repository")
}

#[test]
fn github_merge_then_deletes_the_branch_with_its_name_escaped() {
    let installed = FakeGh::new().on("api", "{}").install();
    Provider::github_for_test()
        .merge(
            PrId(7),
            MergeStrategy::Squash,
            Some(&branch("feature/fix #1?")),
            &read_head(),
        )
        .unwrap();

    assert_eq!(
        installed.calls(),
        vec![
            "api --method PUT repos/{owner}/{repo}/pulls/7/merge -f merge_method=squash -f sha=abc123",
            "api --method DELETE repos/{owner}/{repo}/git/refs/heads/feature/fix%20%231%3F",
        ]
    );
}

#[test]
fn github_a_branch_that_cannot_be_deleted_leaves_the_merge_done() {
    let installed = FakeGh::new()
        .fail("git/refs", 1, "Resource not accessible by integration")
        .on("api", "{}")
        .install();
    let error = Provider::github_for_test()
        .merge(
            PrId(7),
            MergeStrategy::Merge,
            Some(&branch("feature")),
            &read_head(),
        )
        .unwrap_err();

    assert!(
        matches!(error, MergeError::BranchDeleteFailed(_)),
        "{error:?}"
    );
    assert_eq!(installed.calls().len(), 2, "the merge was sent first");
}

#[test]
fn github_a_branch_that_is_already_gone_is_deleted_as_far_as_the_merge_goes() {
    // The repository deletes head branches itself, and was first.
    let installed = FakeGh::new()
        .fail("git/refs", 1, "gh: Reference does not exist (HTTP 422)")
        .on("api", "{}")
        .install();
    Provider::github_for_test()
        .merge(
            PrId(7),
            MergeStrategy::Merge,
            Some(&branch("feature")),
            &read_head(),
        )
        .unwrap();
    assert_eq!(installed.calls().len(), 2);
}

#[test]
fn github_a_merge_that_fails_does_not_delete_the_branch() {
    let installed = FakeGh::new()
        .fail("pulls/7/merge", 1, "Not mergeable")
        .install();
    let error = Provider::github_for_test()
        .merge(
            PrId(7),
            MergeStrategy::Merge,
            Some(&branch("feature")),
            &read_head(),
        )
        .unwrap_err();

    assert!(matches!(error, MergeError::Failed(_)), "{error:?}");
    assert_eq!(installed.calls().len(), 1, "{:?}", installed.calls());
}

#[test]
fn bitbucket_does_not_delete_the_branch_with_the_merge() {
    let server = MockHttp::start(vec![]);
    let error = bitbucket(&server)
        .merge(
            PrId(7),
            MergeStrategy::Merge,
            Some(&branch("feature")),
            &read_head(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        MergeError::Failed(FetchError::Unsupported(_))
    ));
}

#[test]
fn github_asks_each_one_who_asked_for_changes_to_review_again_in_one_call() {
    use crate::domain::review::{Rerequest, Reviewer};
    let installed = FakeGh::new().on("api", "{}").install();
    let asked = |name: &str| Reviewer {
        author: crate::domain::user::User {
            username: name.into(),
        },
        state: ReviewerState::ChangesRequested,
    };
    let who = Rerequest::of(&[asked("alice"), asked("erin")]).expect("two to ask");
    Provider::github_for_test()
        .rerequest_review(PrId(7), &who)
        .unwrap();

    assert_eq!(
        installed.calls(),
        vec![
            "api --method POST repos/{owner}/{repo}/pulls/7/requested_reviewers -f reviewers[]=alice -f reviewers[]=erin"
        ]
    );
}

#[test]
fn github_decline_closes_the_pull_request() {
    let installed = FakeGh::new().on("api", "{}").install();
    Provider::github_for_test().decline(PrId(7)).unwrap();

    assert_eq!(
        installed.calls(),
        vec!["api --method PATCH repos/{owner}/{repo}/pulls/7 -f state=closed"]
    );
}

#[test]
fn github_pr_comment_passes_the_body_as_a_literal_argument() {
    let installed = FakeGh::new().on("api", "{}").install();
    let body = "thanks $(whoami) `id` \"quoted\" & more";
    Provider::github_for_test()
        .post_pr_comment(PrId(7), body)
        .unwrap();

    assert_eq!(
        installed.calls(),
        vec![format!(
            "api --method POST repos/{{owner}}/{{repo}}/issues/7/comments -f body={body}"
        )]
    );
}

#[test]
fn github_batched_review_is_one_call_with_the_comments_on_stdin() {
    let installed = FakeGh::new()
        .on("--jq .head.sha", "abc\n")
        .on("api", "{}")
        .install();
    let comments = [
        review_comment("abc", 3, false),
        review_comment("abc", 9, true),
    ];
    Provider::github_for_test()
        .submit_full_review(
            PrId(7),
            ReviewVerdict::RequestChanges,
            "please fix",
            "me",
            &comments,
            &head_of("abc"),
        )
        .unwrap();

    assert_eq!(
        installed.calls(),
        vec![
            "api repos/{owner}/{repo}/pulls/7 --jq .head.sha",
            "api --method POST repos/{owner}/{repo}/pulls/7/reviews --input -"
        ]
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
    let result = Provider::github_for_test().submit_full_review(
        PrId(7),
        ReviewVerdict::Comment,
        "",
        "me",
        &mixed,
        &head_of("abc"),
    );
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
    Provider::github_for_test().reopen(PrId(7)).unwrap();

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
    let error = Provider::github_for_test().reopen(PrId(7)).unwrap_err();

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
    Provider::github_for_test()
        .edit_comment(PrId(7), review, "new")
        .unwrap();
    Provider::github_for_test()
        .edit_comment(PrId(7), conversation, "new")
        .unwrap();
    Provider::github_for_test()
        .delete_comment(PrId(7), review)
        .unwrap();
    Provider::github_for_test()
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
    Provider::github_for_test()
        .set_thread_resolved(PrId(7), &ThreadHandle::NodeId("PRRT_1".into()), true)
        .unwrap();
    let calls = installed.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(calls[0].contains("resolveReviewThread"), "{calls:?}");
    assert!(calls[0].contains("PRRT_1"), "{calls:?}");

    let refused = Provider::github_for_test().set_thread_resolved(
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
    Provider::github_for_test()
        .set_auto_merge(
            PrId(7),
            &AutoMerge::On {
                strategy: MergeStrategy::Squash,
                head: read_head(),
            },
        )
        .unwrap();
    Provider::github_for_test()
        .set_auto_merge(PrId(7), &AutoMerge::Off)
        .unwrap();

    assert_eq!(
        installed.calls(),
        vec![
            "pr merge 7 --auto --squash --match-head-commit abc123",
            "pr merge 7 --disable-auto"
        ]
    );
}

#[test]
fn github_runs_the_failed_jobs_of_each_failed_run_again_and_nothing_else() {
    let runs = json!([{"workflow_runs": [
        {"id": 11, "conclusion": "failure"},
        {"id": 12, "conclusion": "success"},
        {"id": 13, "conclusion": "timed_out"},
        {"id": 15, "conclusion": "cancelled"},
        {"id": 14, "conclusion": null}
    ]}])
    .to_string();
    let installed = FakeGh::new()
        .on("--jq .head.sha", "abc123\n")
        .on("actions/runs?head_sha=abc123", &runs)
        .on("rerun-failed-jobs", "{}")
        .install();
    Provider::github_for_test()
        .rerun_failed_builds(PrId(7))
        .unwrap();

    let calls = installed.calls();
    let reruns: Vec<_> = calls.iter().filter(|call| call.contains("rerun")).collect();
    assert_eq!(reruns.len(), 3, "{calls:?}");
    assert!(reruns[0].contains("runs/11/rerun-failed-jobs"), "{calls:?}");
    assert!(reruns[1].contains("runs/13/rerun-failed-jobs"), "{calls:?}");
    assert!(reruns[2].contains("runs/15/rerun-failed-jobs"), "{calls:?}");
}

#[test]
fn github_says_so_when_no_actions_run_failed() {
    let runs = json!([{"workflow_runs": [{"id": 12, "conclusion": "success"}]}]).to_string();
    let _gh = FakeGh::new()
        .on("--jq .head.sha", "abc123\n")
        .on("actions/runs?head_sha=abc123", &runs)
        .install();
    let error = Provider::github_for_test()
        .rerun_failed_builds(PrId(7))
        .unwrap_err();
    assert!(error.user_message().contains("No failed"), "{error:?}");
}

#[test]
fn github_keeps_going_when_one_run_cannot_be_started_and_says_how_many_did() {
    let runs = json!([{"workflow_runs": [
        {"id": 11, "conclusion": "failure"},
        {"id": 13, "conclusion": "failure"}
    ]}])
    .to_string();
    let installed = FakeGh::new()
        .on("--jq .head.sha", "abc123\n")
        .on("actions/runs?head_sha=abc123", &runs)
        .fail("runs/11/rerun-failed-jobs", 1, "run 11 is too old")
        .on("runs/13/rerun-failed-jobs", "{}")
        .install();
    let error = Provider::github_for_test()
        .rerun_failed_builds(PrId(7))
        .unwrap_err();

    assert!(
        error.user_message().contains("1 of 2"),
        "{}",
        error.user_message()
    );
    assert!(error.may_have_reached_server());
    let reruns = installed
        .calls()
        .iter()
        .filter(|call| call.contains("rerun-failed-jobs"))
        .count();
    assert_eq!(reruns, 2, "the second run is tried after the first failed");
}

#[test]
fn github_a_merge_of_a_branch_that_moved_says_the_pr_changed_and_deletes_nothing() {
    let installed = FakeGh::new()
        .fail(
            "pulls/7/merge",
            1,
            "gh: Head branch was modified. Review and try the merge again. (HTTP 409)",
        )
        .on("api", "{}")
        .install();
    let error = Provider::github_for_test()
        .merge(
            PrId(7),
            MergeStrategy::Merge,
            Some(&branch("feature")),
            &read_head(),
        )
        .unwrap_err();

    assert!(
        matches!(error, MergeError::Failed(FetchError::Stale(_))),
        "{error:?}"
    );
    assert_eq!(
        installed.calls().len(),
        1,
        "no delete after a refused merge"
    );
}

#[test]
fn github_a_verdict_alone_names_the_commit_it_is_of() {
    let installed = FakeGh::new()
        .on("--jq .head.sha", "abc123\n")
        .on("api", "{}")
        .install();
    Provider::github_for_test()
        .submit_full_review(PrId(7), ReviewVerdict::Approve, "", "me", &[], &read_head())
        .unwrap();

    assert_eq!(
        installed.calls(),
        vec![
            "api repos/{owner}/{repo}/pulls/7 --jq .head.sha",
            "api --method POST repos/{owner}/{repo}/pulls/7/reviews -f event=APPROVE -f commit_id=abc123"
        ]
    );
}

#[test]
fn github_refuses_a_verdict_on_a_branch_that_moved_and_sends_nothing() {
    let installed = FakeGh::new()
        .on("--jq .head.sha", "def456\n")
        .on("api", "{}")
        .install();
    let alone = Provider::github_for_test().submit_full_review(
        PrId(7),
        ReviewVerdict::Approve,
        "",
        "me",
        &[],
        &read_head(),
    );
    let batch = Provider::github_for_test().submit_full_review(
        PrId(7),
        ReviewVerdict::Approve,
        "",
        "me",
        &[review_comment("abc123", 3, false)],
        &read_head(),
    );

    for result in [alone, batch] {
        assert!(
            matches!(result, Err(ReviewError::Failed(FetchError::Stale(_)))),
            "{result:?}"
        );
    }
    assert!(
        installed
            .calls()
            .iter()
            .all(|call| call.contains("--jq .head.sha")),
        "only the head was read: {:?}",
        installed.calls()
    );
}

#[test]
fn every_call_acts_on_the_repository_the_provider_was_given() {
    use crate::providers::GhRepo;
    let installed = FakeGh::new().on("api", "{}").on("pr", "").install();
    let provider = Provider::GitHub(GhRepo::new("github.com", "upstream", "slussa").unwrap());
    // `gh api`, which fills `{owner}/{repo}`, and `gh pr <number>`, which finds
    // the repository itself: both follow what they are told.
    provider
        .merge(PrId(7), MergeStrategy::Merge, None, &read_head())
        .unwrap();
    provider.set_auto_merge(PrId(7), &AutoMerge::Off).unwrap();
    provider.decline(PrId(7)).unwrap();

    assert_eq!(installed.calls().len(), 3);
    assert_eq!(
        installed.repos(),
        ["github.com/upstream/slussa"; 3],
        "{:?}",
        installed.calls()
    );
}

#[test]
fn another_provider_acts_on_another_repository() {
    use crate::providers::GhRepo;
    let installed = FakeGh::new().on("api", "{}").install();
    for name in ["one", "two"] {
        Provider::GitHub(GhRepo::new("github.com", "me", name).unwrap())
            .decline(PrId(7))
            .unwrap();
    }
    assert_eq!(
        installed.repos(),
        ["github.com/me/one", "github.com/me/two"]
    );
}

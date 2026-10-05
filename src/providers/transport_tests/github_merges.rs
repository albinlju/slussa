//! GitHub merges through a fake `gh`: now, by itself when ready, and the branch
//! deleted after, and what is refused when the branch has moved.

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

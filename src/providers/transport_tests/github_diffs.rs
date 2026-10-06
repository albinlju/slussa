//! GitHub diffs through a fake `gh`: what changed between two commits of a PR.

use super::support::*;
use crate::domain::{commit::CommitOid, diff::DiffRange};

const DIFF: &str = "diff --git a/src/a.rs b/src/a.rs\n\
--- a/src/a.rs\n\
+++ b/src/a.rs\n\
@@ -1,2 +1,2 @@\n\
 keep\n\
-old\n\
+new\n";

/// GitHub's answer about the two commits: the one they share.
fn shared(sha: &str) -> String {
    format!(r#"{{"status":"ahead","merge_base_commit":{{"sha":"{sha}"}},"commits":[]}}"#)
}

fn range() -> DiffRange {
    DiffRange {
        base: CommitOid::parse("aaa111").unwrap(),
        head: CommitOid::parse("bbb222").unwrap(),
    }
}

#[test]
fn what_is_new_is_a_compare_of_the_two_commits_read_as_a_diff() {
    let installed = FakeGh::new()
        .on("page=2", &shared("aaa111"))
        .on("compare/aaa111...bbb222", DIFF)
        .install();
    let diff = Provider::github_for_test()
        .fetch_range_diff(&range())
        .unwrap();
    assert_eq!(diff.files.len(), 1);
    let revision = diff.revision.expect("a revision");
    assert_eq!(
        (revision.head.as_str(), revision.base.as_deref()),
        ("bbb222", Some("aaa111"))
    );
    assert!(revision.commit, "it is not the PR's own diff");
    let calls = installed.calls();
    assert!(
        calls
            .iter()
            .any(|call| call.contains("application/vnd.github.diff")),
        "{calls:?}"
    );
}

#[test]
fn the_diff_starts_at_the_commit_the_two_share_which_tells_a_rewritten_branch() {
    let installed = FakeGh::new()
        .on("page=2", &shared("ccc333"))
        .on("compare/aaa111...bbb222", DIFF)
        .install();
    let diff = Provider::github_for_test()
        .fetch_range_diff(&range())
        .unwrap();
    assert_eq!(
        range().moved(&diff),
        Some(crate::domain::diff::Moved::Rewritten {
            from: CommitOid::parse("ccc333").unwrap()
        })
    );
    // Asked for past the first page, which is the one that lists the files.
    let calls = installed.calls();
    assert!(
        calls
            .iter()
            .any(|call| call.contains("compare/aaa111...bbb222?per_page=1&page=2")),
        "{calls:?}"
    );
}

#[test]
fn an_answer_about_the_commits_that_cannot_be_read_is_an_error_and_no_diff() {
    let _installed = FakeGh::new()
        .on("page=2", "{}")
        .on("compare/aaa111...bbb222", DIFF)
        .install();
    let error = Provider::github_for_test()
        .fetch_range_diff(&range())
        .unwrap_err();
    assert!(matches!(error, FetchError::ParseFailed(_)), "{error:?}");
}

#[test]
fn a_commit_that_is_gone_says_the_branch_was_probably_force_pushed() {
    let _installed = FakeGh::new()
        .fail(
            "compare/aaa111...bbb222",
            1,
            "gh: HTTP 404: No common ancestor",
        )
        .install();
    let error = Provider::github_for_test()
        .fetch_range_diff(&range())
        .unwrap_err();
    assert!(matches!(error, FetchError::Stale(_)), "{error:?}");
    let told = error.user_message();
    assert!(told.contains("force-pushed"), "{told}");
    assert!(
        told.contains("No common ancestor"),
        "GitHub's own words are kept: {told}"
    );
    assert!(
        !told.contains("w:"),
        "a provider names no key of the surface: {told}"
    );
}

#[test]
fn a_failure_that_only_has_those_digits_in_it_is_not_taken_for_a_missing_commit() {
    // A request id or a commit id with 404 in it is no 404.
    let _installed = FakeGh::new()
        .fail(
            "compare/aaa111...bbb222",
            1,
            "gh: HTTP 403: rate limited (request 4b404a1, commit a404b)",
        )
        .install();
    let error = Provider::github_for_test()
        .fetch_range_diff(&range())
        .unwrap_err();
    assert!(matches!(error, FetchError::GhFailed { .. }), "{error:?}");
}

#[test]
fn any_other_failure_of_a_compare_is_told_as_it_is() {
    let _installed = FakeGh::new()
        .fail("compare/aaa111...bbb222", 1, "gh: HTTP 403: rate limited")
        .install();
    let result = Provider::github_for_test().fetch_range_diff(&range());
    assert!(
        matches!(result, Err(FetchError::GhFailed { .. })),
        "{result:?}"
    );
}

#[test]
fn bitbucket_cannot_compare_two_commits() {
    let server = MockHttp::start(vec![]);
    let result = bitbucket(&server).fetch_range_diff(&range());
    assert!(
        matches!(result, Err(FetchError::Unsupported(_))),
        "{result:?}"
    );
}

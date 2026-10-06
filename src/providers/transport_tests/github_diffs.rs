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

fn range() -> DiffRange {
    DiffRange {
        base: CommitOid::parse("aaa111").unwrap(),
        head: CommitOid::parse("bbb222").unwrap(),
    }
}

#[test]
fn what_is_new_is_a_compare_of_the_two_commits_read_as_a_diff() {
    let installed = FakeGh::new().on("compare/aaa111...bbb222", DIFF).install();
    let compared = Provider::github_for_test()
        .fetch_range_diff(&range(), None)
        .unwrap();
    assert!(
        compared.in_pr_before.is_none(),
        "not asked for without the commit the PR is against"
    );
    let diff = compared.diff;
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

fn target() -> CommitOid {
    CommitOid::parse("0ba5e0").unwrap()
}

#[test]
fn the_files_the_pr_touched_at_the_commit_that_was_read_are_listed_with_what_is_new() {
    // What the branch at aaa111 had changed since it left the target.
    let before = r#"{"files": [
        {"filename": "src/a.rs"},
        {"filename": "src/new_name.rs", "previous_filename": "src/old_name.rs"}
    ]}"#;
    let installed = FakeGh::new()
        .on("compare/0ba5e0...aaa111", before)
        .on("compare/aaa111...bbb222", DIFF)
        .install();
    let compared = Provider::github_for_test()
        .fetch_range_diff(&range(), Some(&target()))
        .unwrap();
    let mut before: Vec<String> = compared.in_pr_before.expect("listed").into_iter().collect();
    before.sort();
    assert_eq!(before, ["src/a.rs", "src/new_name.rs", "src/old_name.rs"]);
    assert_eq!(compared.diff.files.len(), 1);
    assert_eq!(installed.calls().len(), 2);
}

#[test]
fn as_many_files_as_github_lists_at_most_are_not_taken_for_all_of_them() {
    let files: Vec<String> = (0..300)
        .map(|n| format!(r#"{{"filename": "src/f{n}.rs"}}"#))
        .collect();
    let _installed = FakeGh::new()
        .on(
            "compare/0ba5e0...aaa111",
            &format!(r#"{{"files": [{}]}}"#, files.join(",")),
        )
        .on("compare/aaa111...bbb222", DIFF)
        .install();
    let compared = Provider::github_for_test()
        .fetch_range_diff(&range(), Some(&target()))
        .unwrap();
    assert!(
        compared.in_pr_before.is_none(),
        "there may be more, so nothing is left out on their word"
    );
}

#[test]
fn the_files_that_cannot_be_listed_fail_the_read_instead_of_leaving_some_out() {
    let _installed = FakeGh::new()
        .fail("compare/0ba5e0...aaa111", 1, "gh: HTTP 502: Bad Gateway")
        .on("compare/aaa111...bbb222", DIFF)
        .install();
    let result = Provider::github_for_test().fetch_range_diff(&range(), Some(&target()));
    assert!(
        matches!(result, Err(FetchError::GhFailed { .. })),
        "{result:?}"
    );
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
        .fetch_range_diff(&range(), None)
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
        .fetch_range_diff(&range(), None)
        .unwrap_err();
    assert!(matches!(error, FetchError::GhFailed { .. }), "{error:?}");
}

#[test]
fn any_other_failure_of_a_compare_is_told_as_it_is() {
    let _installed = FakeGh::new()
        .fail("compare/aaa111...bbb222", 1, "gh: HTTP 403: rate limited")
        .install();
    let result = Provider::github_for_test().fetch_range_diff(&range(), None);
    assert!(
        matches!(result, Err(FetchError::GhFailed { .. })),
        "{result:?}"
    );
}

#[test]
fn bitbucket_cannot_compare_two_commits() {
    let server = MockHttp::start(vec![]);
    let result = bitbucket(&server).fetch_range_diff(&range(), None);
    assert!(
        matches!(result, Err(FetchError::Unsupported(_))),
        "{result:?}"
    );
}

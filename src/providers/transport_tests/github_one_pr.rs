//! GitHub reads of one PR: its description, labels and the issues it closes,
//! and the PR itself by its number.

use super::support::*;

fn info_answer(labels: &[&str], more: bool) -> String {
    json!({"data": {"repository": {"pullRequest": {
        "id": "PR_1",
        "body": "Explains the change.",
        "labels": {
            "nodes": labels.iter().map(|name| json!({"name": name})).collect::<Vec<_>>(),
            "pageInfo": {"hasNextPage": more}
        }
    }}}})
    .to_string()
}

#[test]
fn github_info_reads_the_description_and_the_labels_of_one_pr() {
    let gh = FakeGh::new()
        .on("pullRequest(number: $pr)", &info_answer(&["bug"], false))
        .install();
    let info = Provider::GitHub.fetch_info(PrId(7)).unwrap();

    assert_eq!(info.description.as_deref(), Some("Explains the change."));
    assert_eq!(info.labels, vec!["bug"]);
    assert_eq!(gh.calls().len(), 1);
    assert!(gh.calls()[0].contains("pr=7"));
}

#[test]
fn github_info_reads_on_when_the_labels_are_truncated() {
    let node_page = json!({"data": {"item": {"connection": {
        "nodes": [{"name": "bug"}, {"name": "ux"}],
        "pageInfo": {"hasNextPage": false, "endCursor": null}
    }}}})
    .to_string();
    let gh = FakeGh::new()
        .on("node(id: \"PR_1\")", &node_page)
        .on("pullRequest(number: $pr)", &info_answer(&["bug"], true))
        .install();
    let info = Provider::GitHub.fetch_info(PrId(1)).unwrap();

    assert_eq!(info.labels, vec!["bug", "ux"]);
    assert_eq!(gh.calls().len(), 2);
}

#[test]
fn only_github_reads_the_description_and_labels_apart_from_the_list() {
    use crate::domain::capabilities::Feature;
    assert!(Provider::GitHub.capabilities().supports(Feature::PrInfo));
}

#[test]
fn github_reads_one_pr_by_its_number_as_the_list_holds_it() {
    let answer =
        json!({"data": {"repository": {"pullRequest": gh_pr(44, "2026-10-01T10:00:00Z")}}})
            .to_string();
    let gh = FakeGh::new().on("statusCheckRollup", &answer).install();
    let pr = Provider::GitHub.fetch_pr(PrId(44)).unwrap();

    assert_eq!((pr.id, pr.title.as_str()), (PrId(44), "PR number 44"));
    assert_eq!(gh.calls().len(), 1);
    assert!(gh.calls()[0].contains("pr=44"), "{:?}", gh.calls());
}

#[test]
fn github_reports_a_pr_that_does_not_exist_as_an_error() {
    let _gh = FakeGh::new()
        .fail(
            "statusCheckRollup",
            1,
            "GraphQL: Could not resolve to a PullRequest",
        )
        .install();
    assert!(Provider::GitHub.fetch_pr(PrId(9999)).is_err());
}

#[test]
fn github_info_reads_the_issues_the_pr_closes() {
    let answer = json!({"data": {"repository": {"pullRequest": {
        "id": "PR_1",
        "body": "Fixes it.",
        "labels": {"nodes": [], "pageInfo": {"hasNextPage": false}},
        "closingIssuesReferences": {"nodes": [
            {"number": 12, "title": "Crash on start", "url": "https://github.com/o/r/issues/12"},
            {"number": 31, "title": "Slow list", "url": "https://github.com/o/other/issues/31"}
        ], "pageInfo": {"hasNextPage": false}}
    }}}})
    .to_string();
    let _gh = FakeGh::new()
        .on("pullRequest(number: $pr)", &answer)
        .install();
    let info = Provider::GitHub.fetch_info(PrId(7)).unwrap();

    let issues: Vec<_> = info
        .issues
        .iter()
        .map(|issue| (issue.number, issue.title.as_str()))
        .collect();
    assert_eq!(issues, [(12, "Crash on start"), (31, "Slow list")]);
    assert_eq!(
        info.issues[1].url.as_deref(),
        Some("https://github.com/o/other/issues/31"),
        "an issue of another repository keeps its own address"
    );
}

#[test]
fn github_info_reads_on_when_the_closing_issues_are_truncated() {
    let first_page = json!({"data": {"repository": {"pullRequest": {
        "id": "PR_1",
        "body": "Fixes many.",
        "labels": {"nodes": [], "pageInfo": {"hasNextPage": false}},
        "closingIssuesReferences": {
            "nodes": [{"number": 1, "title": "One"}],
            "pageInfo": {"hasNextPage": true}
        }
    }}}})
    .to_string();
    let all = json!({"data": {"item": {"connection": {
        "nodes": [{"number": 1, "title": "One"}, {"number": 2, "title": "Two"}],
        "pageInfo": {"hasNextPage": false, "endCursor": null}
    }}}})
    .to_string();
    let gh = FakeGh::new()
        .on("node(id: \"PR_1\")", &all)
        .on("pullRequest(number: $pr)", &first_page)
        .install();
    let info = Provider::GitHub.fetch_info(PrId(1)).unwrap();

    let numbers: Vec<_> = info.issues.iter().map(|issue| issue.number).collect();
    assert_eq!(numbers, [1, 2]);
    assert_eq!(gh.calls().len(), 2);
}

#[test]
fn github_a_pr_answered_with_null_is_said_not_to_exist() {
    let answer = json!({"data": {"repository": {"pullRequest": null}}}).to_string();
    let _gh = FakeGh::new().on("statusCheckRollup", &answer).install();
    let error = Provider::GitHub.fetch_pr(PrId(9999)).unwrap_err();
    assert!(error.user_message().contains("no PR #9999"), "{error:?}");
}

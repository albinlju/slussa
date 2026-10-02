//! GitHub payloads as the tests feed them to the fake `gh`.

use serde_json::{Value, json};

pub fn gh_pr(number: u64, created: &str) -> Value {
    json!({
        "id": format!("PR_{number}"),
        "url": format!("https://github.com/o/r/pull/{number}"),
        "title": format!("PR number {number}"),
        "number": number,
        "author": {"login": "alice"},
        "state": "OPEN",
        "isDraft": false,
        "headRefName": "feature",
        "baseRefName": "main",
        "createdAt": created,
        "updatedAt": created,
        "additions": 3,
        "deletions": 1,
        "changedFiles": 2,
        "comments": {"totalCount": 4},
        "reviewThreads": {"totalCount": 2},
        "latestReviews": {
            "nodes": [{"state": "APPROVED", "author": {"login": "bob"}}],
            "pageInfo": {"hasNextPage": false}
        },
        "commits": {"nodes": [{"commit": {"statusCheckRollup": {"state": "SUCCESS"}}}]}
    })
}

/// A PR as `gh_pr` builds it, in the given GitHub state (`MERGED` or `CLOSED`).
pub fn gh_closed_pr(number: u64, created: &str, state: &str) -> Value {
    let mut pr = gh_pr(number, created);
    pr["state"] = json!(state);
    pr
}

/// One page of `repository { connection { nodes pageInfo } }`.
pub fn gh_list_page(nodes: &[Value], next: Option<&str>) -> String {
    json!({"data": {"repository": {"connection": {
        "nodes": nodes,
        "pageInfo": {"hasNextPage": next.is_some(), "endCursor": next}
    }}}})
    .to_string()
}

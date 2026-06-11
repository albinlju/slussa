use serde::Deserialize;

use crate::clients::error::FetchError;
use crate::clients::github::cli::run_gh_json;
use crate::clients::github::{GqlComment, map_gql_comment};
use crate::domain::comment::ReviewThread;

// GraphQL instead of REST `/pulls/{n}/comments`: threads arrive pre-grouped
// with `isResolved`, `diffSide` tells old-side anchors apart, and the
// reactions carry `viewerHasReacted`.
const QUERY: &str = "query($owner: String!, $name: String!, $pr: Int!) { \
  repository(owner: $owner, name: $name) { pullRequest(number: $pr) { \
    reviewThreads(first: 100) { nodes { \
      isResolved path line originalLine diffSide \
      comments(first: 100) { nodes { \
        body createdAt author { login } \
        reactionGroups { content viewerHasReacted users { totalCount } } \
      } } } } } } }";

#[derive(Debug, Deserialize)]
struct GqlResponse {
    data: GqlData,
}

#[derive(Debug, Deserialize)]
struct GqlData {
    repository: GqlRepository,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlRepository {
    pull_request: GqlPullRequest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlPullRequest {
    review_threads: GqlThreads,
}

#[derive(Debug, Deserialize)]
struct GqlThreads {
    nodes: Vec<GqlThread>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlThread {
    #[serde(default)]
    is_resolved: bool,
    #[serde(default)]
    path: String,
    /// Anchor line on the side given by `diff_side`; `null` for outdated
    /// threads, where `original_line` still holds the anchor.
    #[serde(default)]
    line: Option<usize>,
    #[serde(default)]
    original_line: Option<usize>,
    #[serde(default)]
    diff_side: String,
    comments: GqlComments,
}

#[derive(Debug, Deserialize)]
struct GqlComments {
    nodes: Vec<GqlComment>,
}

pub fn fetch_review_threads(pr_number: u64) -> Result<Vec<ReviewThread>, FetchError> {
    let resp: GqlResponse = run_gh_json(&[
        "api",
        "graphql",
        "-F",
        "owner={owner}",
        "-F",
        "name={repo}",
        "-F",
        &format!("pr={pr_number}"),
        "-f",
        &format!("query={QUERY}"),
    ])?;
    Ok(resp
        .data
        .repository
        .pull_request
        .review_threads
        .nodes
        .into_iter()
        .map(map_thread)
        .collect())
}

fn map_thread(t: GqlThread) -> ReviewThread {
    let anchor = t.line.or(t.original_line);
    let (line, old_line) = if t.diff_side == "LEFT" {
        (None, anchor)
    } else {
        (anchor, None)
    };
    ReviewThread {
        path: t.path,
        line,
        old_line,
        resolved: t.is_resolved,
        comments: t.comments.nodes.into_iter().map(map_gql_comment).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{ "data": { "repository": { "pullRequest": { "reviewThreads": {
        "nodes": [{
            "isResolved": true,
            "path": "src/x.rs",
            "line": null,
            "originalLine": 7,
            "diffSide": "LEFT",
            "comments": { "nodes": [{
                "body": "old line looks wrong",
                "createdAt": "2026-06-06T13:09:13Z",
                "author": { "login": "ana" },
                "reactionGroups": [
                    { "content": "THUMBS_UP", "viewerHasReacted": true, "users": { "totalCount": 4 } },
                    { "content": "EYES", "viewerHasReacted": false, "users": { "totalCount": 3 } },
                    { "content": "HEART", "viewerHasReacted": false, "users": { "totalCount": 0 } }
                ]
            }] }
        }]
    } } } } }"#;

    #[test]
    fn maps_threads_with_reactions_and_old_side_anchor() {
        let resp: GqlResponse = serde_json::from_str(SAMPLE).unwrap();
        let threads: Vec<ReviewThread> = resp
            .data
            .repository
            .pull_request
            .review_threads
            .nodes
            .into_iter()
            .map(map_thread)
            .collect();

        let t = &threads[0];
        assert!(t.resolved);
        // LEFT side + null line → anchored to the old file via originalLine.
        assert_eq!((t.line, t.old_line), (None, Some(7)));

        let reactions = &t.comments[0].reactions;
        // The zero-count HEART group is dropped.
        assert_eq!(reactions.len(), 2);
        assert_eq!(
            (reactions[0].emoji.as_str(), reactions[0].count, reactions[0].mine),
            ("👍", 4, true)
        );
        assert_eq!(
            (reactions[1].emoji.as_str(), reactions[1].count, reactions[1].mine),
            ("👀", 3, false)
        );
    }
}

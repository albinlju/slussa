use serde::Deserialize;

use crate::domain::comment::{CommentThread, ThreadAnchor};
use crate::providers::error::FetchError;
use crate::providers::github::{COMMENT_FIELDS, GqlComment, map_gql_comment};

#[cfg(test)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlPullRequest {
    review_threads: GqlThreads,
}

#[cfg(test)]
#[derive(Debug, Deserialize)]
struct GqlThreads {
    nodes: Vec<GqlThread>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlThread {
    #[serde(default)]
    id: String,
    #[serde(default)]
    is_resolved: bool,
    #[serde(default)]
    is_outdated: bool,
    #[serde(default)]
    path: String,
    #[serde(default)]
    line: Option<usize>,
    #[serde(default)]
    original_line: Option<usize>,
    #[serde(default)]
    diff_side: String,
    #[serde(default)]
    comments: GqlComments,
}

#[derive(Debug, Default, Deserialize)]
struct GqlComments {
    nodes: Vec<GqlComment>,
}

pub fn fetch_review_threads(pr_number: u64) -> Result<Vec<CommentThread>, FetchError> {
    let mut nodes: Vec<GqlThread> = super::pagination::pr_nodes(
        pr_number,
        "reviewThreads",
        "id isResolved isOutdated path line originalLine diffSide",
    )?;
    for thread in &mut nodes {
        thread.comments.nodes = super::pagination::node_nodes(
            &thread.id,
            "PullRequestReviewThread",
            "comments",
            &format!("{COMMENT_FIELDS} commit {{ oid }} originalCommit {{ oid }}"),
        )?;
    }
    Ok(nodes.into_iter().map(map_thread).collect())
}

fn map_thread(t: GqlThread) -> CommentThread {
    let pos = t.line.or(t.original_line);
    let (line, old_line) = if t.diff_side == "LEFT" {
        (None, pos)
    } else {
        (pos, None)
    };
    let reply_to = t.comments.nodes.first().and_then(|c| c.database_id);
    let revision = t
        .comments
        .nodes
        .first()
        .and_then(|c| {
            if t.is_outdated {
                c.original_commit.as_ref()
            } else {
                c.commit.as_ref()
            }
        })
        .map(|c| c.oid.clone());
    // GitHub review threads are always anchored to code.
    CommentThread {
        comments: t.comments.nodes.into_iter().map(map_gql_comment).collect(),
        reply_to,
        anchor: Some(ThreadAnchor {
            revision,
            path: t.path,
            line,
            old_line,
            resolved: t.is_resolved,
            node_id: (!t.id.is_empty()).then_some(t.id),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{ "reviewThreads": {
        "nodes": [{
            "isResolved": true,
            "path": "src/x.rs",
            "line": null,
            "originalLine": 7,
            "diffSide": "LEFT",
            "comments": { "nodes": [{
                "databaseId": 555,
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
    } }"#;

    #[test]
    fn outdated_thread_keeps_its_original_revision() {
        let mut value: serde_json::Value = serde_json::from_str(SAMPLE).unwrap();
        let thread = &mut value["reviewThreads"]["nodes"][0];
        thread["isOutdated"] = true.into();
        thread["comments"]["nodes"][0]["originalCommit"] = serde_json::json!({"oid": "old-sha"});
        thread["comments"]["nodes"][0]["commit"] = serde_json::json!({"oid": "new-sha"});
        let pr: GqlPullRequest = serde_json::from_value(value).unwrap();
        let thread = map_thread(pr.review_threads.nodes.into_iter().next().unwrap());
        let mut revision = crate::domain::diff::DiffRevision {
            head: "old-sha".into(),
            base: None,
            commit: true,
        };
        assert!(thread.matches_revision(Some(&revision)));
        revision.head = "new-sha".into();
        assert!(!thread.matches_revision(Some(&revision)));
    }

    #[test]
    fn maps_threads_with_reactions_and_old_side_anchor() {
        let pr: GqlPullRequest = serde_json::from_str(SAMPLE).unwrap();
        let threads: Vec<CommentThread> = pr
            .review_threads
            .nodes
            .into_iter()
            .map(map_thread)
            .collect();

        let t = &threads[0];
        let anchor = t.anchor.as_ref().unwrap();
        assert!(anchor.resolved);
        assert_eq!((anchor.line, anchor.old_line), (None, Some(7)));
        assert_eq!(t.reply_to, Some(555));

        let reactions = &t.comments[0].reactions;
        assert_eq!(reactions.len(), 2);
        assert_eq!(
            (
                reactions[0].emoji.as_str(),
                reactions[0].count,
                reactions[0].mine
            ),
            ("👍", 4, true)
        );
        assert_eq!(
            (
                reactions[1].emoji.as_str(),
                reactions[1].count,
                reactions[1].mine
            ),
            ("👀", 3, false)
        );
    }
}

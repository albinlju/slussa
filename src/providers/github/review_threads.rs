use serde::Deserialize;

use crate::domain::comment::{CommentThread, ThreadAnchor, ThreadHandle};
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
    #[serde(default, rename = "pageInfo")]
    page_info: Option<super::pagination::PageInfo>,
}

/// A review thread with its first comments; `<<fields>>` is what to read of
/// each comment.
const THREAD_FIELDS: &str = r"
    id isResolved isOutdated path line originalLine diffSide
    comments(first: 20) {
      nodes { <<fields>> }
      pageInfo { hasNextPage endCursor }
    }
";

pub fn fetch_review_threads(pr_number: u64) -> Result<Vec<CommentThread>, FetchError> {
    let fields = format!("{COMMENT_FIELDS} commit {{ oid }} originalCommit {{ oid }}");
    // Keep the nested page modest: a PR page can contain 100 review threads.
    let nodes: Vec<GqlThread> = super::pagination::pr_nodes(
        pr_number,
        "reviewThreads",
        &super::graphql::fill(THREAD_FIELDS, &[("fields", &fields)]),
    )?;
    complete_threads(nodes, |id, cursor| {
        super::pagination::node_nodes_after(
            id,
            "PullRequestReviewThread",
            "comments",
            &fields,
            Some(cursor),
        )
    })
}

fn complete_threads(
    mut nodes: Vec<GqlThread>,
    mut remaining: impl FnMut(&str, &str) -> Result<Vec<GqlComment>, FetchError>,
) -> Result<Vec<CommentThread>, FetchError> {
    for thread in &mut nodes {
        let info = thread
            .comments
            .page_info
            .as_ref()
            .ok_or_else(|| FetchError::ParseFailed("Missing thread comment pageInfo".into()))?;
        if info.has_next_page {
            let cursor = info
                .end_cursor
                .as_deref()
                .filter(|c| !c.is_empty())
                .ok_or_else(|| FetchError::ParseFailed("Missing thread comment cursor".into()))?;
            thread.comments.nodes.extend(remaining(&thread.id, cursor)?);
        }
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
            handle: (!t.id.is_empty()).then_some(ThreadHandle::NodeId(t.id)),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paged_thread(id: &str, next: bool) -> GqlThread {
        serde_json::from_value(serde_json::json!({
            "id": id, "path": "src/main.rs", "line": 42,
            "comments": {
                "nodes": [{"databaseId": 1, "body": "First", "createdAt": "2026-01-01T00:00:00Z", "author": {"login": "alice"}}],
                "pageInfo": {"hasNextPage": next, "endCursor": "page-one"}
            }
        })).unwrap()
    }

    #[test]
    fn embedded_comments_only_fetch_remaining_pages_when_needed() {
        let mut requests = Vec::new();
        let threads = complete_threads(
            vec![paged_thread("short", false), paged_thread("long", true)],
            |id, cursor| {
                requests.push((id.to_owned(), cursor.to_owned()));
                let mut extra = paged_thread("unused", false).comments.nodes;
                extra[0].database_id = Some(2);
                Ok(extra)
            },
        )
        .unwrap();
        assert_eq!(requests, vec![("long".into(), "page-one".into())]);
        assert_eq!(threads[0].comments.len(), 1);
        assert_eq!(
            threads[1].comments.iter().map(|c| c.id).collect::<Vec<_>>(),
            vec![Some(1), Some(2)]
        );
        assert_eq!(threads[1].reply_to, Some(1));
        // Resolved through the thread's own id, not a comment's.
        assert_eq!(
            threads[1].anchor.as_ref().unwrap().handle,
            Some(ThreadHandle::NodeId("long".into()))
        );
    }

    #[test]
    fn incomplete_comment_pages_fail_instead_of_dropping_replies() {
        assert!(
            complete_threads(vec![paged_thread("long", true)], |_, _| Err(
                FetchError::Timeout
            ))
            .is_err()
        );
        let mut missing = paged_thread("long", true);
        missing.comments.page_info.as_mut().unwrap().end_cursor = None;
        assert!(
            complete_threads(vec![missing], |_, _| panic!("missing cursor must fail")).is_err()
        );
        let mut missing = paged_thread("short", false);
        missing.comments.page_info = None;
        assert!(
            complete_threads(vec![missing], |_, _| panic!("missing page info must fail")).is_err()
        );
    }

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

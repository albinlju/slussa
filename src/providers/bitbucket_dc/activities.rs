use serde::Deserialize;

use super::{Config, ms_to_utc};
use crate::domain::activity::Activity;
use crate::domain::authorship::Authorship;
use crate::domain::event::{EventKind, PushedCommit, TimelineEvent};
use crate::domain::user::User;
use crate::domain::{
    comment::{Comment, CommentId, CommentThread, Reaction, ThreadAnchor, ThreadHandle},
    diff::LineRef,
    pr::PrId,
};
use crate::providers::bitbucket_dc::http::get_all;
use crate::providers::error::FetchError;

#[cfg(test)]
#[derive(Debug, Deserialize)]
struct Page {
    values: Vec<BbActivity>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BbActivity {
    action: String,
    #[serde(default)]
    created_date: i64,
    #[serde(default)]
    user: Option<BbUser>,
    #[serde(default)]
    comment_anchor: Option<Anchor>,
    #[serde(default)]
    comment: Option<BbComment>,
    #[serde(default)]
    added: Option<Rescope>,
}

#[derive(Debug, Deserialize)]
struct Rescope {
    #[serde(default)]
    commits: Vec<RescopeCommit>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RescopeCommit {
    #[serde(default)]
    display_id: String,
    #[serde(default)]
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Anchor {
    #[serde(default)]
    to_hash: Option<String>,
    #[serde(default)]
    path: String,
    #[serde(default)]
    line: usize,
    #[serde(default)]
    line_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BbComment {
    #[serde(default)]
    id: u64,
    author: BbUser,
    text: String,
    created_date: i64,
    /// Task state (`OPEN`/`RESOLVED`) — only meaningful for BLOCKER tasks.
    #[serde(default)]
    state: String,
    /// Whether the conversation was closed via the "Resolve" button. This, not
    /// `state`, is what a resolved review thread sets (and `state` stays `OPEN`).
    #[serde(default)]
    thread_resolved: bool,
    #[serde(default)]
    comments: Vec<Self>,
    #[serde(default)]
    properties: BbProperties,
}

#[derive(Debug, Default, Deserialize)]
struct BbProperties {
    #[serde(default)]
    reactions: Vec<BbReaction>,
}

#[derive(Debug, Deserialize)]
struct BbReaction {
    emoticon: BbEmoticon,
    #[serde(default)]
    users: Vec<serde::de::IgnoredAny>,
}

#[derive(Debug, Deserialize)]
struct BbEmoticon {
    #[serde(default)]
    url: String,
}

#[derive(Debug, Deserialize)]
struct BbUser {
    name: String,
}

pub fn fetch(config: &Config, pr_id: PrId) -> Result<Activity, FetchError> {
    let path = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/activities?limit=100",
        config.repo.project_key, config.repo.repo_slug
    );
    let values: Vec<BbActivity> = get_all(&config.repo.base_url, &path, &config.pat)?;
    Ok(project(values))
}

fn project(activities: Vec<BbActivity>) -> Activity {
    let mut bundle = Activity::default();
    for activity in activities {
        if activity.action == "COMMENTED" {
            match (activity.comment_anchor, activity.comment) {
                (Some(anchor), Some(root)) => bundle.threads.push(make_thread(anchor, &root)),
                // A general comment with replies is a conversation — keep the
                // whole thread, not just the root. A lone one stays a flat comment.
                (None, Some(root)) if root.comments.is_empty() => {
                    bundle.comments.push(map_comment(&root));
                }
                (None, Some(root)) => bundle.threads.push(make_general_thread(&root)),
                _ => {}
            }
            continue;
        }

        let kind = if activity.action == "RESCOPED" {
            let commits: Vec<PushedCommit> = activity
                .added
                .map(|a| a.commits)
                .unwrap_or_default()
                .into_iter()
                .map(map_pushed)
                .collect();
            if commits.is_empty() {
                continue;
            }
            EventKind::Pushed(commits)
        } else if let Some(kind) = event_kind(&activity.action) {
            kind
        } else {
            continue;
        };

        bundle.events.push(TimelineEvent {
            actor: activity.user.as_ref().map(map_user),
            kind,
            created: ms_to_utc(activity.created_date),
        });
    }
    bundle
}

fn event_kind(action: &str) -> Option<EventKind> {
    match action {
        "OPENED" => Some(EventKind::Opened),
        "APPROVED" => Some(EventKind::Approved),
        "REVIEWED" => Some(EventKind::ChangesRequested),
        "UNAPPROVED" => Some(EventKind::ReviewRemoved),
        "MERGED" => Some(EventKind::Merged),
        "DECLINED" => Some(EventKind::Declined),
        "REOPENED" => Some(EventKind::Reopened),
        _ => None,
    }
}

fn make_thread(anchor: Anchor, root: &BbComment) -> CommentThread {
    let line = if anchor.line_type.eq_ignore_ascii_case("REMOVED") {
        LineRef::Old(anchor.line)
    } else {
        LineRef::New(anchor.line)
    };
    let mut comments = Vec::new();
    collect_replies(root, &mut comments);
    let root_id = comment_id(root);
    CommentThread {
        comments,
        reply_to: root_id,
        anchor: Some(ThreadAnchor {
            revision: anchor.to_hash,
            path: anchor.path,
            line: Some(line),
            // Resolved by the "Resolve" button (threadResolved) or, for a task,
            // by closing the task (state == RESOLVED).
            resolved: root.thread_resolved || root.state.eq_ignore_ascii_case("RESOLVED"),
            handle: root_id.map(ThreadHandle::RootComment),
        }),
    }
}

/// A general (non-inline) comment that has replies — a path-less conversation
/// (no anchor) so the whole thread renders in Overview, not just the root.
fn make_general_thread(root: &BbComment) -> CommentThread {
    let mut comments = Vec::new();
    collect_replies(root, &mut comments);
    CommentThread {
        comments,
        reply_to: comment_id(root),
        anchor: None,
    }
}

fn collect_replies(c: &BbComment, out: &mut Vec<Comment>) {
    out.push(map_comment(c));
    for child in &c.comments {
        collect_replies(child, out);
    }
}

/// A comment's id; none where the server sent none (it then parses as 0).
fn comment_id(comment: &BbComment) -> Option<CommentId> {
    (comment.id != 0).then_some(CommentId(comment.id))
}

fn map_comment(c: &BbComment) -> Comment {
    let id = comment_id(c);
    Comment {
        id,
        author: map_user(&c.author),
        account: crate::domain::user::AccountKind::Person,
        authorship: Authorship::Human,
        content: c.text.clone(),
        created: ms_to_utc(c.created_date),
        reactions: map_reactions(&c.properties),
        reply_to: id,
    }
}

fn map_reactions(props: &BbProperties) -> Vec<Reaction> {
    props
        .reactions
        .iter()
        .filter_map(|r| {
            Some(Reaction {
                emoji: emoji_from_url(&r.emoticon.url)?,
                count: u32::try_from(r.users.len()).unwrap_or(u32::MAX),
                mine: false,
            })
        })
        .collect()
}

fn emoji_from_url(url: &str) -> Option<String> {
    let stem = url.rsplit('/').next()?.strip_suffix(".svg")?;
    let mut emoji = String::new();
    for part in stem.split('-') {
        emoji.push(char::from_u32(u32::from_str_radix(part, 16).ok()?)?);
    }
    (!emoji.is_empty()).then_some(emoji)
}

fn map_user(u: &BbUser) -> User {
    User {
        username: u.name.clone(),
    }
}

fn map_pushed(c: RescopeCommit) -> PushedCommit {
    PushedCommit {
        id: c.display_id,
        message: c.message.lines().next().unwrap_or_default().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{ "values": [
        { "action": "OPENED", "createdDate": 1000, "user": { "name": "ana" } },
        { "action": "COMMENTED", "createdDate": 2000,
          "comment": { "id": 1, "text": "general", "createdDate": 2000, "updatedDate": 2000,
                       "author": { "name": "bo" } } },
        { "action": "COMMENTED", "createdDate": 3000,
          "commentAnchor": { "path": "src/x.rs", "line": 42, "lineType": "ADDED" },
          "comment": { "id": 2, "text": "inline", "createdDate": 3000, "updatedDate": 3000,
                       "state": "OPEN", "author": { "name": "cy" },
                       "comments": [ { "id": 3, "text": "reply", "createdDate": 3100,
                                       "updatedDate": 3100, "author": { "name": "ana" } } ] } },
        { "action": "MERGED", "createdDate": 4000, "user": { "name": "ana" } },
        { "action": "RESCOPED", "createdDate": 4500, "user": { "name": "ana" },
          "added": { "commits": [
            { "displayId": "ab0d1e2", "message": "Merge 'develop' into feat\n\nbody" }
          ] } },
        { "action": "RESCOPED", "createdDate": 4600, "user": { "name": "ana" } }
    ]}"#;

    #[test]
    fn splits_feed_into_comments_events_and_threads() {
        let page: Page = serde_json::from_str(SAMPLE).unwrap();
        let bundle = project(page.values);

        assert_eq!(bundle.comments.len(), 1);
        assert_eq!(bundle.comments[0].content, "general");

        assert_eq!(bundle.threads.len(), 1);
        let anchor = bundle.threads[0].anchor.as_ref().unwrap();
        assert_eq!(anchor.path, "src/x.rs");
        assert_eq!(anchor.line, Some(LineRef::New(42)));
        assert_eq!(bundle.threads[0].comments.len(), 2);
        assert_eq!(bundle.threads[0].reply_to, Some(CommentId(2)));
        assert!(!anchor.resolved); // state OPEN, no threadResolved
        // Resolved through its root comment.
        assert_eq!(anchor.handle, Some(ThreadHandle::RootComment(CommentId(2))));

        assert_eq!(bundle.events.len(), 3);
        assert_eq!(bundle.events[0].kind, EventKind::Opened);
        assert_eq!(bundle.events[1].kind, EventKind::Merged);
        assert_eq!(
            bundle.events[2].kind,
            EventKind::Pushed(vec![PushedCommit {
                id: "ab0d1e2".to_string(),
                message: "Merge 'develop' into feat".to_string(),
            }])
        );
    }

    const WITH_REACTIONS: &str = r#"{ "values": [
        { "action": "COMMENTED", "createdDate": 5000,
          "comment": { "id": 9, "text": "hi", "createdDate": 5000, "updatedDate": 5000,
                       "author": { "name": "bo" },
                       "properties": { "repositoryId": 1, "reactions": [
                         { "emoticon": { "shortcut": "+1",
                             "url": "https://x/twemoji/12.1.2/1f44d.svg" },
                           "users": [ { "name": "bo" }, { "name": "cy" } ] },
                         { "emoticon": { "shortcut": "tada",
                             "url": "https://x/twemoji/12.1.2/1f389.svg" },
                           "users": [ { "name": "ana" } ] }
                       ] } } }
    ]}"#;

    #[test]
    fn reads_reactions_from_comment_properties() {
        let page: Page = serde_json::from_str(WITH_REACTIONS).unwrap();
        let bundle = project(page.values);
        let reactions = &bundle.comments[0].reactions;
        assert_eq!(reactions.len(), 2);
        assert_eq!(reactions[0].emoji, "👍");
        assert_eq!(reactions[0].count, 2);
        assert_eq!(reactions[1].emoji, "🎉");
        assert_eq!(reactions[1].count, 1);
        assert!(reactions.iter().all(|r| !r.mine));
    }

    #[test]
    fn thread_resolved_flag_marks_thread_resolved() {
        // Real shape: the "Resolve" button sets threadResolved=true while the
        // comment's state stays "OPEN" (resolvedDate can even be null).
        let json = r#"{ "values": [
            { "action": "COMMENTED", "createdDate": 1,
              "commentAnchor": { "path": "a.rs", "line": 5, "lineType": "ADDED" },
              "comment": { "id": 7, "text": "fix this", "createdDate": 1, "updatedDate": 1,
                           "state": "OPEN", "threadResolved": true,
                           "author": { "name": "bo" } } }
        ]}"#;
        let page: Page = serde_json::from_str(json).unwrap();
        let bundle = project(page.values);
        assert_eq!(bundle.threads.len(), 1);
        assert!(bundle.threads[0].resolved());
    }

    #[test]
    fn general_comment_with_replies_becomes_a_thread() {
        // Bitbucket nests replies under the root's `comments`, even for general
        // (non-anchored) comments — they must surface as a full thread.
        let json = r#"{ "values": [
            { "action": "COMMENTED", "createdDate": 1,
              "comment": { "id": 1, "text": "root", "createdDate": 1, "updatedDate": 1,
                           "author": { "name": "bo" },
                           "comments": [
                             { "id": 2, "text": "reply", "createdDate": 2, "updatedDate": 2,
                               "author": { "name": "cy" } } ] } },
            { "action": "COMMENTED", "createdDate": 3,
              "comment": { "id": 3, "text": "lone", "createdDate": 3, "updatedDate": 3,
                           "author": { "name": "bo" } } }
        ]}"#;
        let page: Page = serde_json::from_str(json).unwrap();
        let bundle = project(page.values);
        // Lone comment stays flat; the one with a reply becomes a path-less thread.
        assert_eq!(bundle.comments.len(), 1);
        assert_eq!(bundle.comments[0].content, "lone");
        assert_eq!(bundle.threads.len(), 1);
        assert!(bundle.threads[0].anchor.is_none());
        assert_eq!(bundle.threads[0].comments.len(), 2);
        assert_eq!(bundle.threads[0].comments[1].content, "reply");
    }

    #[test]
    fn decodes_multi_codepoint_and_rejects_junk() {
        assert_eq!(
            emoji_from_url("https://x/1f1f8-1f1ea.svg").as_deref(),
            Some("🇸🇪")
        );
        assert_eq!(emoji_from_url("https://x/not-an-emoji.png"), None);
        assert_eq!(emoji_from_url(""), None);
    }
}

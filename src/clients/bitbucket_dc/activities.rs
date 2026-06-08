//! Bitbucket DC exposes a PR's whole history through one `/activities` feed —
//! comments, approvals, merges, the lot. We fetch it once and split it into the
//! three views the UI wants: general comments, lifecycle events, and inline
//! review threads. (This is the activity-side mirror of [`super::structured`],
//! which does the same one-fetch-many-views trick for the diff.)
//!
//! Inline comments come straight from here rather than the structured diff,
//! since the diff window can omit comments whose line falls outside it; the
//! activities feed always lists every one.

use chrono::{DateTime, TimeZone, Utc};
use serde::Deserialize;

use super::Config;
use crate::clients::ActivityBundle;
use crate::clients::bitbucket_dc::http::get_json;
use crate::clients::error::FetchError;
use crate::domain::comment::{Comment, ReviewThread};
use crate::domain::event::{EventKind, TimelineEvent};
use crate::domain::user::User;

#[derive(Debug, Deserialize)]
struct Page {
    values: Vec<Activity>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Activity {
    action: String,
    #[serde(default)]
    created_date: i64,
    #[serde(default)]
    user: Option<BbUser>,
    #[serde(default)]
    comment_anchor: Option<Anchor>,
    #[serde(default)]
    comment: Option<BbComment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Anchor {
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
    id: u64,
    author: BbUser,
    text: String,
    created_date: i64,
    updated_date: i64,
    #[serde(default)]
    state: String,
    #[serde(default)]
    comments: Vec<BbComment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BbUser {
    name: String,
    #[serde(default)]
    display_name: Option<String>,
}

pub fn fetch(config: &Config, pr_id: u64) -> Result<ActivityBundle, FetchError> {
    let path = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/activities?limit=100",
        config.repo.project_key, config.repo.repo_slug
    );
    let page: Page = get_json(&config.repo.host, &path, &config.pat)?;

    let mut bundle = ActivityBundle::default();
    for activity in page.values {
        if activity.action == "COMMENTED" {
            match (activity.comment_anchor, activity.comment) {
                // Anchored to a diff line → inline review thread.
                (Some(anchor), Some(root)) => bundle.threads.push(make_thread(anchor, &root)),
                // No anchor → general discussion comment.
                (None, Some(root)) => bundle.comments.push(map_comment(&root)),
                _ => {}
            }
        } else if let Some(kind) = event_kind(&activity.action) {
            bundle.events.push(TimelineEvent {
                actor: activity.user.map(map_user),
                kind,
                created: ms_to_utc(activity.created_date),
            });
        }
    }
    Ok(bundle)
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

fn make_thread(anchor: Anchor, root: &BbComment) -> ReviewThread {
    // Removed lines anchor to the old-file line; added/context to the new.
    let (line, old_line) = if anchor.line_type.eq_ignore_ascii_case("REMOVED") {
        (None, Some(anchor.line))
    } else {
        (Some(anchor.line), None)
    };
    let mut comments = Vec::new();
    collect_replies(root, &mut comments);
    ReviewThread {
        path: anchor.path,
        line,
        old_line,
        diff_hunk: String::new(),
        resolved: root.state.eq_ignore_ascii_case("RESOLVED"),
        comments,
    }
}

fn collect_replies(c: &BbComment, out: &mut Vec<Comment>) {
    out.push(map_comment(c));
    for child in &c.comments {
        collect_replies(child, out);
    }
}

fn map_comment(c: &BbComment) -> Comment {
    Comment {
        id: c.id,
        author: map_user_ref(&c.author),
        content: c.text.clone(),
        created: ms_to_utc(c.created_date),
        updated: ms_to_utc(c.updated_date),
        replies: Vec::new(),
        resolved: c.state.eq_ignore_ascii_case("RESOLVED"),
    }
}

fn map_user(u: BbUser) -> User {
    User {
        id: u.name.clone(),
        username: u.name,
        display_name: u.display_name,
        avatar_url: None,
    }
}

fn map_user_ref(u: &BbUser) -> User {
    User {
        id: u.name.clone(),
        username: u.name.clone(),
        display_name: u.display_name.clone(),
        avatar_url: None,
    }
}

fn ms_to_utc(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).single().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::EventKind;

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
        { "action": "RESCOPED", "createdDate": 4500, "user": { "name": "ana" } }
    ]}"#;

    #[test]
    fn splits_feed_into_comments_events_and_threads() {
        let page: Page = serde_json::from_str(SAMPLE).unwrap();
        let mut bundle = ActivityBundle::default();
        for activity in page.values {
            if activity.action == "COMMENTED" {
                match (activity.comment_anchor, activity.comment) {
                    (Some(a), Some(r)) => bundle.threads.push(make_thread(a, &r)),
                    (None, Some(r)) => bundle.comments.push(map_comment(&r)),
                    _ => {}
                }
            } else if let Some(kind) = event_kind(&activity.action) {
                bundle.events.push(TimelineEvent {
                    actor: activity.user.map(map_user),
                    kind,
                    created: ms_to_utc(activity.created_date),
                });
            }
        }

        // One general comment, one inline thread (with a reply), two events
        // (OPENED + MERGED); RESCOPED is ignored.
        assert_eq!(bundle.comments.len(), 1);
        assert_eq!(bundle.comments[0].content, "general");

        assert_eq!(bundle.threads.len(), 1);
        assert_eq!(bundle.threads[0].path, "src/x.rs");
        assert_eq!(bundle.threads[0].line, Some(42));
        assert_eq!(bundle.threads[0].comments.len(), 2); // root + reply

        assert_eq!(bundle.events.len(), 2);
        assert_eq!(bundle.events[0].kind, EventKind::Opened);
        assert_eq!(bundle.events[1].kind, EventKind::Merged);
    }
}

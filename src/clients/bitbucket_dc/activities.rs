//! Bitbucket DC exposes a PR's whole history through one `/activities` feed;
//! it gets fetched once and split into general comments, lifecycle events,
//! and inline review threads. Inline comments come from here rather than the
//! structured diff because the diff window can omit comments whose line falls
//! outside it — the activities feed always lists every one.

use serde::Deserialize;

use super::{Config, ms_to_utc};
use crate::clients::ActivityBundle;
use crate::clients::bitbucket_dc::http::get_json;
use crate::clients::error::FetchError;
use crate::domain::comment::{Comment, ReviewThread};
use crate::domain::event::{EventKind, PushedCommit, TimelineEvent};
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
    /// Commits added by a RESCOPED (push) activity.
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
    path: String,
    #[serde(default)]
    line: usize,
    #[serde(default)]
    line_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BbComment {
    author: BbUser,
    text: String,
    created_date: i64,
    #[serde(default)]
    state: String,
    #[serde(default)]
    comments: Vec<BbComment>,
}

#[derive(Debug, Deserialize)]
struct BbUser {
    name: String,
}

pub fn fetch(config: &Config, pr_id: u64) -> Result<ActivityBundle, FetchError> {
    let path = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/activities?limit=100",
        config.repo.project_key, config.repo.repo_slug
    );
    let page: Page = get_json(&config.repo.host, &path, &config.pat)?;
    Ok(project(page.values))
}

fn project(activities: Vec<Activity>) -> ActivityBundle {
    let mut bundle = ActivityBundle::default();
    for activity in activities {
        if activity.action == "COMMENTED" {
            match (activity.comment_anchor, activity.comment) {
                // An anchor means the comment sits on a diff line.
                (Some(anchor), Some(root)) => bundle.threads.push(make_thread(anchor, &root)),
                (None, Some(root)) => bundle.comments.push(map_comment(&root)),
                _ => {}
            }
            continue;
        }

        // A push shows up as RESCOPED carrying the added commits.
        let kind = if activity.action == "RESCOPED" {
            let commits: Vec<PushedCommit> = activity
                .added
                .map(|a| a.commits)
                .unwrap_or_default()
                .into_iter()
                .map(map_pushed)
                .collect();
            // Skip force-pushes / re-targets that didn't add commits.
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
        author: map_user(&c.author),
        content: c.text.clone(),
        created: ms_to_utc(c.created_date),
    }
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

        // One general comment, one inline thread (with a reply).
        assert_eq!(bundle.comments.len(), 1);
        assert_eq!(bundle.comments[0].content, "general");

        assert_eq!(bundle.threads.len(), 1);
        assert_eq!(bundle.threads[0].path, "src/x.rs");
        assert_eq!(bundle.threads[0].line, Some(42));
        assert_eq!(bundle.threads[0].comments.len(), 2); // root + reply

        // OPENED + MERGED + the RESCOPED that added a commit. The second
        // RESCOPED added nothing, so it's dropped.
        assert_eq!(bundle.events.len(), 3);
        assert_eq!(bundle.events[0].kind, EventKind::Opened);
        assert_eq!(bundle.events[1].kind, EventKind::Merged);
        assert_eq!(
            bundle.events[2].kind,
            EventKind::Pushed(vec![PushedCommit {
                id: "ab0d1e2".to_string(),
                // Only the first message line is kept.
                message: "Merge 'develop' into feat".to_string(),
            }])
        );
    }
}

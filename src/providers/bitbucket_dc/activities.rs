use serde::Deserialize;

use super::{Config, ms_to_utc};
use crate::domain::activity::Activity;
use crate::domain::comment::{Comment, Reaction, ReviewThread};
use crate::domain::event::{EventKind, PushedCommit, TimelineEvent};
use crate::domain::user::User;
use crate::providers::bitbucket_dc::http::get_json;
use crate::providers::error::FetchError;

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

pub fn fetch(config: &Config, pr_id: u64) -> Result<Activity, FetchError> {
    let path = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/activities?limit=100",
        config.repo.project_key, config.repo.repo_slug
    );
    let page: Page = get_json(&config.repo.base_url, &path, &config.pat)?;
    Ok(project(page.values))
}

fn project(activities: Vec<BbActivity>) -> Activity {
    let mut bundle = Activity::default();
    for activity in activities {
        if activity.action == "COMMENTED" {
            match (activity.comment_anchor, activity.comment) {
                (Some(anchor), Some(root)) => bundle.threads.push(make_thread(anchor, &root)),
                (None, Some(root)) => bundle.comments.push(map_comment(&root)),
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

fn make_thread(anchor: Anchor, root: &BbComment) -> ReviewThread {
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
        reactions: map_reactions(&c.properties),
    }
}

fn map_reactions(props: &BbProperties) -> Vec<Reaction> {
    props
        .reactions
        .iter()
        .filter_map(|r| {
            Some(Reaction {
                emoji: emoji_from_url(&r.emoticon.url)?,
                count: r.users.len() as u32,
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

        assert_eq!(bundle.comments.len(), 1);
        assert_eq!(bundle.comments[0].content, "general");

        assert_eq!(bundle.threads.len(), 1);
        assert_eq!(bundle.threads[0].path, "src/x.rs");
        assert_eq!(bundle.threads[0].line, Some(42));
        assert_eq!(bundle.threads[0].comments.len(), 2);

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
    fn decodes_multi_codepoint_and_rejects_junk() {
        assert_eq!(
            emoji_from_url("https://x/1f1f8-1f1ea.svg").as_deref(),
            Some("🇸🇪")
        );
        assert_eq!(emoji_from_url("https://x/not-an-emoji.png"), None);
        assert_eq!(emoji_from_url(""), None);
    }
}

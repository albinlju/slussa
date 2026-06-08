//! Bitbucket Data Center's `/diff` endpoint returns a *structured* JSON diff
//! by default (not a raw unified-diff text). With `withComments=true` that
//! same payload also carries every inline review comment, attached to the
//! exact line it anchors to. We fetch it once and project it into both a
//! [`Diff`] (for the Diff tab) and a list of [`ReviewThread`]s (the comments
//! on the diff), so a single request feeds both fetchers.
//!
//! Shape (trimmed to what we read):
//! ```json
//! { "diffs": [ {
//!     "source":      { "toString": "old/path" },   // null for added files
//!     "destination": { "toString": "new/path" },   // null for deleted files
//!     "hunks": [ {
//!       "sourceLine": 10, "destinationLine": 10,
//!       "segments": [ {
//!         "type": "CONTEXT" | "ADDED" | "REMOVED",
//!         "lines": [ {
//!           "source": 10, "destination": 10, "line": "text",
//!           "comments": [ { comment } ]            // only with withComments
//!         } ] } ] } ] } ] }
//! ```

use chrono::{DateTime, TimeZone, Utc};
use serde::Deserialize;

use super::Config;
use crate::clients::bitbucket_dc::http::get_json;
use crate::clients::error::FetchError;
use crate::domain::{
    comment::{Comment, ReviewThread},
    diff::{Diff, DiffLine, FileDiff, Hunk},
    user::User,
};

#[derive(Debug, Deserialize)]
struct BbDiffResponse {
    #[serde(default)]
    diffs: Vec<BbFileDiff>,
}

#[derive(Debug, Deserialize)]
struct BbFileDiff {
    #[serde(default)]
    source: Option<BbFileRef>,
    #[serde(default)]
    destination: Option<BbFileRef>,
    #[serde(default)]
    hunks: Vec<BbHunk>,
}

#[derive(Debug, Deserialize)]
struct BbFileRef {
    #[serde(rename = "toString", default)]
    to_string: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BbHunk {
    #[serde(default)]
    source_line: usize,
    #[serde(default)]
    destination_line: usize,
    #[serde(default)]
    segments: Vec<BbSegment>,
}

#[derive(Debug, Deserialize)]
struct BbSegment {
    #[serde(rename = "type", default)]
    seg_type: String,
    #[serde(default)]
    lines: Vec<BbLine>,
}

#[derive(Debug, Deserialize)]
struct BbLine {
    #[serde(default)]
    source: usize,
    #[serde(default)]
    destination: usize,
    #[serde(default)]
    line: String,
    #[serde(default)]
    comments: Vec<BbComment>,
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

/// One fetch, two views: the parsed [`Diff`] and the inline [`ReviewThread`]s.
pub(super) struct StructuredDiff {
    pub diff: Diff,
    pub threads: Vec<ReviewThread>,
}

pub(super) fn fetch(config: &Config, pr_id: u64) -> Result<StructuredDiff, FetchError> {
    let path = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/diff?withComments=true",
        config.repo.project_key, config.repo.repo_slug
    );
    let response: BbDiffResponse = get_json(&config.repo.host, &path, &config.pat)?;
    Ok(project(response))
}

fn project(response: BbDiffResponse) -> StructuredDiff {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut threads: Vec<ReviewThread> = Vec::new();

    for file in response.diffs {
        // Prefer the destination path; fall back to source for deleted files.
        let path = file
            .destination
            .as_ref()
            .map(|r| r.to_string.clone())
            .filter(|p| !p.is_empty())
            .or_else(|| file.source.as_ref().map(|r| r.to_string.clone()))
            .unwrap_or_default();
        if path.is_empty() {
            continue;
        }

        let mut hunks: Vec<Hunk> = Vec::new();
        for hunk in file.hunks {
            let mut lines: Vec<DiffLine> = Vec::new();
            for segment in hunk.segments {
                let removed = segment.seg_type.eq_ignore_ascii_case("REMOVED");
                let added = segment.seg_type.eq_ignore_ascii_case("ADDED");
                for bl in segment.lines {
                    lines.push(if removed {
                        DiffLine::Removed(bl.line.clone())
                    } else if added {
                        DiffLine::Added(bl.line.clone())
                    } else {
                        DiffLine::Context(bl.line.clone())
                    });

                    if !bl.comments.is_empty() {
                        // Removed lines anchor to the old-file line number;
                        // added/context anchor to the new-file line.
                        let (line, old_line) = if removed {
                            (None, Some(bl.source))
                        } else {
                            (Some(bl.destination), None)
                        };
                        for root in &bl.comments {
                            threads.push(make_thread(&path, line, old_line, root));
                        }
                    }
                }
            }
            hunks.push(Hunk {
                old_start: hunk.source_line,
                new_start: hunk.destination_line,
                lines,
            });
        }

        files.push(FileDiff { path, hunks });
    }

    StructuredDiff {
        diff: Diff { files },
        threads,
    }
}

fn make_thread(
    path: &str,
    line: Option<usize>,
    old_line: Option<usize>,
    root: &BbComment,
) -> ReviewThread {
    let mut flat: Vec<Comment> = Vec::new();
    collect_replies(root, &mut flat);
    ReviewThread {
        path: path.to_string(),
        line,
        old_line,
        diff_hunk: String::new(),
        comments: flat,
        resolved: root.state.eq_ignore_ascii_case("RESOLVED"),
    }
}

/// Flatten a comment and its nested replies into a single chronological list
/// (root first, then replies in source order), matching how GitHub threads
/// are rendered.
fn collect_replies(c: &BbComment, out: &mut Vec<Comment>) {
    out.push(map_comment(c));
    for child in &c.comments {
        collect_replies(child, out);
    }
}

fn map_comment(c: &BbComment) -> Comment {
    Comment {
        id: c.id,
        author: User {
            id: c.author.name.clone(),
            username: c.author.name.clone(),
            display_name: c.author.display_name.clone(),
            avatar_url: None,
        },
        content: c.text.clone(),
        created: ms_to_utc(c.created_date),
        updated: ms_to_utc(c.updated_date),
        replies: Vec::new(),
        resolved: c.state.eq_ignore_ascii_case("RESOLVED"),
    }
}

fn ms_to_utc(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).single().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    // A trimmed but realistic `/diff?withComments=true` payload: one file with
    // a context line, a removed line carrying a comment, and an added line
    // carrying a comment thread (root + one reply).
    const SAMPLE: &str = r#"{
      "diffs": [{
        "source": { "toString": "src/app.rs" },
        "destination": { "toString": "src/app.rs" },
        "hunks": [{
          "sourceLine": 10,
          "destinationLine": 10,
          "segments": [
            { "type": "CONTEXT", "lines": [
              { "source": 10, "destination": 10, "line": "fn main() {" }
            ]},
            { "type": "REMOVED", "lines": [
              { "source": 11, "destination": 11, "line": "    old();",
                "comments": [{
                  "id": 1, "text": "drop this", "createdDate": 1000, "updatedDate": 1000,
                  "state": "OPEN", "author": { "name": "alice" }
                }]
              }
            ]},
            { "type": "ADDED", "lines": [
              { "source": 11, "destination": 11, "line": "    new();",
                "comments": [{
                  "id": 2, "text": "nice", "createdDate": 2000, "updatedDate": 2000,
                  "state": "RESOLVED", "author": { "name": "bob", "displayName": "Bob B" },
                  "comments": [{
                    "id": 3, "text": "thanks", "createdDate": 3000, "updatedDate": 3000,
                    "state": "RESOLVED", "author": { "name": "alice" }
                  }]
                }]
              }
            ]}
          ]
        }]
      }]
    }"#;

    fn parse_sample() -> StructuredDiff {
        project(serde_json::from_str(SAMPLE).expect("sample parses"))
    }

    #[test]
    fn maps_diff_lines_and_hunk_offsets() {
        let result = parse_sample();
        assert_eq!(result.diff.files.len(), 1);
        let file = &result.diff.files[0];
        assert_eq!(file.path, "src/app.rs");
        assert_eq!(file.hunks.len(), 1);
        let hunk = &file.hunks[0];
        assert_eq!((hunk.old_start, hunk.new_start), (10, 10));
        assert!(matches!(&hunk.lines[0], DiffLine::Context(s) if s == "fn main() {"));
        assert!(matches!(&hunk.lines[1], DiffLine::Removed(s) if s == "    old();"));
        assert!(matches!(&hunk.lines[2], DiffLine::Added(s) if s == "    new();"));
    }

    #[test]
    fn anchors_comments_to_correct_side() {
        let threads = parse_sample().threads;
        assert_eq!(threads.len(), 2);

        // Removed-line comment anchors on the old-file line, not the new side.
        let removed = threads.iter().find(|t| t.comments[0].id == 1).unwrap();
        assert_eq!(removed.old_line, Some(11));
        assert_eq!(removed.line, None);
        assert!(!removed.resolved);

        // Added-line thread anchors on the new-file line and flattens its reply.
        let added = threads.iter().find(|t| t.comments[0].id == 2).unwrap();
        assert_eq!(added.line, Some(11));
        assert_eq!(added.old_line, None);
        assert!(added.resolved);
        assert_eq!(added.comments.len(), 2);
        assert_eq!(added.comments[1].id, 3);
        assert_eq!(added.comments[0].author.display_name.as_deref(), Some("Bob B"));
    }
}

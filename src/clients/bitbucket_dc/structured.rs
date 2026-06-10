//! Bitbucket DC's `/diff` endpoint returns a *structured* JSON diff (not raw
//! unified-diff text), projected here into [`Diff`]. Inline comments come
//! from the activities feed instead (see [`super::activities`]).
//!
//! Shape (trimmed to what we read):
//! ```json
//! { "diffs": [ {
//!     "source":      { "toString": "old/path" },   // null for added files
//!     "destination": { "toString": "new/path" },   // null for deleted files
//!     "hunks": [ {
//!       "sourceLine": 10, "destinationLine": 10,
//!       "segments": [ { "type": "CONTEXT"|"ADDED"|"REMOVED",
//!         "lines": [ { "line": "text" } ] } ] } ] } ] }
//! ```

use serde::Deserialize;

use super::Config;
use crate::clients::bitbucket_dc::http::get_json;
use crate::clients::error::FetchError;
use crate::domain::diff::{Diff, DiffLine, FileDiff, Hunk};

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
    line: String,
}

pub(super) fn fetch(config: &Config, pr_id: u64) -> Result<Diff, FetchError> {
    let path = format!(
        "/rest/api/1.0/projects/{}/repos/{}/pull-requests/{pr_id}/diff",
        config.repo.project_key, config.repo.repo_slug
    );
    fetch_path(config, &path)
}

/// Same structured-diff endpoint scoped to a commit (diffs against the first
/// parent by default).
pub(super) fn fetch_commit(config: &Config, oid: &str) -> Result<Diff, FetchError> {
    let path = format!(
        "/rest/api/1.0/projects/{}/repos/{}/commits/{oid}/diff",
        config.repo.project_key, config.repo.repo_slug
    );
    fetch_path(config, &path)
}

fn fetch_path(config: &Config, path: &str) -> Result<Diff, FetchError> {
    let response: BbDiffResponse = get_json(&config.repo.host, path, &config.pat)?;
    Ok(project(response))
}

fn project(response: BbDiffResponse) -> Diff {
    let mut files: Vec<FileDiff> = Vec::new();

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
                        DiffLine::Removed(bl.line)
                    } else if added {
                        DiffLine::Added(bl.line)
                    } else {
                        DiffLine::Context(bl.line)
                    });
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

    Diff { files }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "diffs": [{
        "source": { "toString": "src/app.rs" },
        "destination": { "toString": "src/app.rs" },
        "hunks": [{
          "sourceLine": 10,
          "destinationLine": 10,
          "segments": [
            { "type": "CONTEXT", "lines": [ { "line": "fn main() {" } ] },
            { "type": "REMOVED", "lines": [ { "line": "    old();" } ] },
            { "type": "ADDED",   "lines": [ { "line": "    new();" } ] }
          ]
        }]
      }]
    }"#;

    #[test]
    fn maps_diff_lines_and_hunk_offsets() {
        let diff = project(serde_json::from_str(SAMPLE).expect("sample parses"));
        assert_eq!(diff.files.len(), 1);
        let file = &diff.files[0];
        assert_eq!(file.path, "src/app.rs");
        assert_eq!(file.hunks.len(), 1);
        let hunk = &file.hunks[0];
        assert_eq!((hunk.old_start, hunk.new_start), (10, 10));
        assert!(matches!(&hunk.lines[0], DiffLine::Context(s) if s == "fn main() {"));
        assert!(matches!(&hunk.lines[1], DiffLine::Removed(s) if s == "    old();"));
        assert!(matches!(&hunk.lines[2], DiffLine::Added(s) if s == "    new();"));
    }
}

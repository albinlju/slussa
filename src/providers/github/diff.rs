use std::process::Command;

use crate::domain::diff::{Diff, DiffLine, FileDiff, Hunk};

pub fn fetch_diff(pr_number: u64) -> Diff {
    let output = Command::new("gh")
        .args(["pr", "diff", &pr_number.to_string()])
        .output()
        .expect("gh not installed");

    let text = String::from_utf8_lossy(&output.stdout);
    parse_unified_diff(&text)
}

fn parse_unified_diff(text: &str) -> Diff {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut current_file: Option<FileDiff> = None;
    let mut current_hunk: Option<Hunk> = None;

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            if let Some(hunk) = current_hunk.take() {
                if let Some(f) = current_file.as_mut() {
                    f.hunks.push(hunk);
                }
            }
            if let Some(f) = current_file.take() {
                files.push(f);
            }
            let path = rest
                .split_whitespace()
                .next()
                .map(|s| s.strip_prefix("a/").unwrap_or(s).to_string())
                .unwrap_or_default();
            current_file = Some(FileDiff {
                path,
                hunks: Vec::new(),
            });
        } else if line.starts_with("@@") {
            if let Some(hunk) = current_hunk.take() {
                if let Some(f) = current_file.as_mut() {
                    f.hunks.push(hunk);
                }
            }
            let (old_start, new_start) = parse_hunk_header(line);
            current_hunk = Some(Hunk {
                old_start,
                new_start,
                lines: Vec::new(),
            });
        } else if let Some(hunk) = current_hunk.as_mut() {
            if line.starts_with("+++") || line.starts_with("---") {
                continue;
            }
            if let Some(rest) = line.strip_prefix('+') {
                hunk.lines.push(DiffLine::Added(rest.to_string()));
            } else if let Some(rest) = line.strip_prefix('-') {
                hunk.lines.push(DiffLine::Removed(rest.to_string()));
            } else if let Some(rest) = line.strip_prefix(' ') {
                hunk.lines.push(DiffLine::Context(rest.to_string()));
            }
        }
    }

    if let Some(hunk) = current_hunk.take() {
        if let Some(f) = current_file.as_mut() {
            f.hunks.push(hunk);
        }
    }
    if let Some(f) = current_file.take() {
        files.push(f);
    }

    Diff { files }
}

fn parse_hunk_header(line: &str) -> (usize, usize) {
    let mut old_start = 0;
    let mut new_start = 0;
    for part in line.split_whitespace() {
        if let Some(rest) = part.strip_prefix('-') {
            old_start = rest
                .split(',')
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
        } else if let Some(rest) = part.strip_prefix('+') {
            new_start = rest
                .split(',')
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
        }
    }
    (old_start, new_start)
}

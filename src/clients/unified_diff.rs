use crate::domain::diff::{Diff, DiffLine, FileDiff, Hunk};

pub fn parse(text: &str) -> Diff {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut current_file: Option<FileDiff> = None;
    let mut current_hunk: Option<Hunk> = None;

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            if let Some(hunk) = current_hunk.take()
                && let Some(f) = current_file.as_mut()
            {
                f.hunks.push(hunk);
            }
            if let Some(f) = current_file.take() {
                files.push(f);
            }
            let path = rest
                .split_whitespace()
                .next_back()
                .map(|s| s.strip_prefix("b/").unwrap_or(s).to_string())
                .unwrap_or_default();
            current_file = Some(FileDiff {
                path,
                hunks: Vec::new(),
            });
        } else if line.starts_with("@@") {
            if let Some(hunk) = current_hunk.take()
                && let Some(f) = current_file.as_mut()
            {
                f.hunks.push(hunk);
            }
            let (old_start, new_start) = parse_hunk_header(line);
            current_hunk = Some(Hunk {
                old_start,
                new_start,
                lines: Vec::new(),
            });
        } else if let Some(hunk) = current_hunk.as_mut() {
            if let Some(rest) = line.strip_prefix('+') {
                hunk.lines.push(DiffLine::Added(rest.to_string()));
            } else if let Some(rest) = line.strip_prefix('-') {
                hunk.lines.push(DiffLine::Removed(rest.to_string()));
            } else if let Some(rest) = line.strip_prefix(' ') {
                hunk.lines.push(DiffLine::Context(rest.to_string()));
            }
        }
    }

    if let Some(hunk) = current_hunk.take()
        && let Some(f) = current_file.as_mut()
    {
        f.hunks.push(hunk);
    }
    if let Some(f) = current_file.take() {
        files.push(f);
    }

    Diff { files }
}

fn parse_hunk_header(line: &str) -> (usize, usize) {
    let mut old_start = 0;
    let mut new_start = 0;
    for part in line.split_whitespace().skip(1) {
        if part == "@@" {
            break;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hunk_header_ignores_section_text() {
        assert_eq!(
            parse_hunk_header("@@ -10,7 +12,8 @@ fn half_page(viewport: u16) -> i16 {"),
            (10, 12)
        );
        assert_eq!(parse_hunk_header("@@ -1 +1,2 @@"), (1, 1));
    }

    #[test]
    fn keeps_diff_lines_starting_with_double_plus_or_minus() {
        let text =
            "diff --git a/x.c b/x.c\n--- a/x.c\n+++ b/x.c\n@@ -1,2 +1,2 @@\n int i = 0;\n---i;\n+++i;\n";
        let diff = parse(text);
        let lines = &diff.files[0].hunks[0].lines;
        assert!(matches!(&lines[0], DiffLine::Context(s) if s == "int i = 0;"));
        assert!(matches!(&lines[1], DiffLine::Removed(s) if s == "--i;"));
        assert!(matches!(&lines[2], DiffLine::Added(s) if s == "++i;"));
    }

    #[test]
    fn renamed_file_keeps_the_new_path() {
        let text = "diff --git a/old_name.rs b/new_name.rs\n\
                    similarity index 90%\n\
                    rename from old_name.rs\n\
                    rename to new_name.rs\n\
                    --- a/old_name.rs\n\
                    +++ b/new_name.rs\n\
                    @@ -1,1 +1,1 @@\n\
                    -x\n\
                    +y\n";
        let diff = parse(text);
        assert_eq!(diff.files[0].path, "new_name.rs");
    }

    #[test]
    fn splits_files_and_skips_headers() {
        let text = "diff --git a/a.rs b/a.rs\n\
                    index 123..456 100644\n\
                    --- a/a.rs\n\
                    +++ b/a.rs\n\
                    @@ -3,2 +3,2 @@\n\
                    -old();\n\
                    +new();\n\
                    diff --git a/b.rs b/b.rs\n\
                    --- a/b.rs\n\
                    +++ b/b.rs\n\
                    @@ -1,1 +1,1 @@\n\
                    -x\n\
                    +y\n";
        let diff = parse(text);
        assert_eq!(diff.files.len(), 2);
        assert_eq!(diff.files[0].path, "a.rs");
        assert_eq!(diff.files[0].hunks[0].old_start, 3);
        assert_eq!(diff.files[1].path, "b.rs");
        assert_eq!(diff.files[1].hunks[0].lines.len(), 2);
    }
}

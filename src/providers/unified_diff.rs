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
            current_file = Some(FileDiff {
                path: git_header_path(rest),
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
        } else if let Some(file) = current_file.as_mut()
            && let Some(path) = extended_header_path(line)
        {
            // Before the first hunk: these lines name the path exactly, which
            // the `diff --git` line cannot when a path has a space in it.
            file.path = path;
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

    Diff {
        files,
        revision: None,
    }
}

/// The new path as far as the `diff --git a/old b/new` line can tell. Unquoted
/// paths with spaces make that line ambiguous, so this is a first guess that
/// `extended_header_path` corrects; it stands for a file with no such lines
/// (a binary file, a mode change).
fn git_header_path(rest: &str) -> String {
    if rest.ends_with('"')
        && let Some(start) = rest.rfind(" \"")
    {
        return strip_side(&unquote(&rest[start + 1..]));
    }
    // Unchanged name: `a/<path> b/<path>`, which splits in the middle.
    if let Some(middle) = rest.len().checked_sub(1).map(|n| n / 2)
        && let (Some(old), Some(new)) = (rest.get(..middle), rest.get(middle + 1..))
        && rest.as_bytes().get(middle) == Some(&b' ')
        && let (Some(old), Some(new)) = (old.strip_prefix("a/"), new.strip_prefix("b/"))
        && old == new
    {
        return new.to_owned();
    }
    rest.rfind(" b/").map_or_else(
        || {
            rest.split_whitespace()
                .next_back()
                .map(strip_side)
                .unwrap_or_default()
        },
        |at| rest[at + 3..].to_owned(),
    )
}

/// The path named by a line between `diff --git` and the first hunk, if it
/// names one: `rename to`, `copy to`, or `---` / `+++` (the later wins, so a
/// deleted file keeps its old path and every other file gets its new one).
fn extended_header_path(line: &str) -> Option<String> {
    if let Some(to) = line
        .strip_prefix("rename to ")
        .or_else(|| line.strip_prefix("copy to "))
    {
        return Some(unquote(to));
    }
    let side = line
        .strip_prefix("+++ ")
        .or_else(|| line.strip_prefix("--- "))?;
    // git ends these lines with a tab when the path has a space in it.
    let side = side.strip_suffix('\t').unwrap_or(side);
    (side != "/dev/null").then(|| strip_side(&unquote(side)))
}

fn strip_side(path: &str) -> String {
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path)
        .to_owned()
}

/// Undo git's quoting of a path with special or non-ASCII characters:
/// `"b/\303\245.rs"` is `b/å.rs`. Anything not in quotes is returned as it is.
fn unquote(path: &str) -> String {
    let Some(inner) = path
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    else {
        return path.to_owned();
    };
    let mut bytes = Vec::with_capacity(inner.len());
    let mut rest = inner.bytes().peekable();
    while let Some(byte) = rest.next() {
        if byte != b'\\' {
            bytes.push(byte);
            continue;
        }
        match rest.next() {
            Some(digit @ b'0'..=b'7') => {
                let mut value = u32::from(digit - b'0');
                for _ in 0..2 {
                    match rest.peek() {
                        Some(next @ b'0'..=b'7') => {
                            value = value * 8 + u32::from(next - b'0');
                            rest.next();
                        }
                        _ => break,
                    }
                }
                bytes.push(u8::try_from(value).unwrap_or(b'?'));
            }
            Some(b'n') => bytes.push(b'\n'),
            Some(b't') => bytes.push(b'\t'),
            Some(b'r') => bytes.push(b'\r'),
            Some(other) => bytes.push(other),
            None => bytes.push(b'\\'),
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
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
        let text = "diff --git a/x.c b/x.c\n--- a/x.c\n+++ b/x.c\n@@ -1,2 +1,2 @@\n int i = 0;\n---i;\n+++i;\n";
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

    /// `git diff` output for a commit that touches paths with spaces, a tab
    /// and a non-ASCII letter, with a new, a deleted, a renamed and a binary
    /// file among them.
    const AWKWARD_PATHS: &str = concat!(
        "diff --git a/bin file.dat b/bin file.dat\n",
        "index bdc955b..8835708 100644\n",
        "Binary files a/bin file.dat and b/bin file.dat differ\n",
        "diff --git a/brand new.txt b/brand new.txt\n",
        "new file mode 100644\n",
        "index 0000000..3e75765\n",
        "--- /dev/null\n",
        "+++ b/brand new.txt\t\n",
        "@@ -0,0 +1 @@\n",
        "+new\n",
        "diff --git a/my file.rs b/my file.rs\n",
        "index 5626abf..f719efd 100644\n",
        "--- a/my file.rs\t\n",
        "+++ b/my file.rs\t\n",
        "@@ -1 +1 @@\n",
        "-one\n",
        "+two\n",
        "diff --git a/old gone.txt b/old gone.txt\n",
        "deleted file mode 100644\n",
        "index 286c5f5..0000000\n",
        "--- a/old gone.txt\t\n",
        "+++ /dev/null\n",
        "@@ -1 +0,0 @@\n",
        "-gone\n",
        "diff --git a/to rename.txt b/renamed b/new name.txt\n",
        "similarity index 100%\n",
        "rename from to rename.txt\n",
        "rename to renamed b/new name.txt\n",
        "diff --git \"a/tab\\tname.txt\" \"b/tab\\tname.txt\"\n",
        "index bca70f3..4286f42 100644\n",
        "--- \"a/tab\\tname.txt\"\n",
        "+++ \"b/tab\\tname.txt\"\n",
        "@@ -1 +1 @@\n",
        "-q\n",
        "+r\n",
        "diff --git \"a/\\303\\245.rs\" \"b/\\303\\245.rs\"\n",
        "index 587be6b..975fbec 100644\n",
        "--- \"a/\\303\\245.rs\"\n",
        "+++ \"b/\\303\\245.rs\"\n",
        "@@ -1 +1 @@\n",
        "-x\n",
        "+y\n",
    );

    #[test]
    fn paths_with_spaces_quotes_and_renames_are_read_whole() {
        let diff = parse(AWKWARD_PATHS);
        let paths: Vec<&str> = diff.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "bin file.dat",
                "brand new.txt",
                "my file.rs",
                "old gone.txt",
                "renamed b/new name.txt",
                "tab\tname.txt",
                "å.rs",
            ]
        );
        // The header lines are not taken for content, and content is kept.
        assert!(diff.files[0].hunks.is_empty());
        assert_eq!(diff.files[2].hunks[0].lines.len(), 2);
        assert!(matches!(&diff.files[3].hunks[0].lines[0], DiffLine::Removed(s) if s == "gone"));
    }

    #[test]
    fn a_header_alone_names_the_file_when_it_can() {
        assert_eq!(git_header_path("a/x.rs b/x.rs"), "x.rs");
        assert_eq!(git_header_path("a/my file.rs b/my file.rs"), "my file.rs");
        assert_eq!(git_header_path("a/old.rs b/new.rs"), "new.rs");
        assert_eq!(
            git_header_path(r#""a/\303\245.rs" "b/\303\245.rs""#),
            "å.rs"
        );
        assert_eq!(git_header_path(""), "");
        assert_eq!(unquote(r#""a\\b\"c""#), "a\\b\"c");
        assert_eq!(unquote("plain name"), "plain name");
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

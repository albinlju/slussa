//! Rules from AGENTS.md that a test can hold, so they do not rest on review.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// "Modules stay under about 500 lines" is the norm, and review holds it. This
/// is the stop behind it, set higher on purpose: a limit at the norm itself
/// would fail on a two-line change and reward trimming a file over splitting it.
const MAX_LINES: usize = 600;

fn rust_files(dir: &Path, found: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            rust_files(&path, found)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    Ok(())
}

#[test]
fn no_source_file_is_over_the_size_rule() -> io::Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for dir in ["src", "tests"] {
        rust_files(&root.join(dir), &mut files)?;
    }
    assert!(
        !files.is_empty(),
        "found no Rust files under {}",
        root.display()
    );

    let mut over = Vec::new();
    for file in &files {
        let lines = fs::read_to_string(file)?.lines().count();
        if lines > MAX_LINES {
            let name = file.strip_prefix(root).unwrap_or(file);
            over.push(format!("  {} ({lines} lines)", name.display()));
        }
    }
    assert!(
        over.is_empty(),
        "over {MAX_LINES} lines, well past the norm of about 500; split the file by \
         concern before adding to it (AGENTS.md, *Modules stay under about 500 lines*):\n{}",
        over.join("\n")
    );
    Ok(())
}

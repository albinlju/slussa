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

/// What each layer may import from the crate, by first path segment. The
/// layers hold this in non-test code today; this keeps it that way.
const IMPORTS: &[(&str, &[&str])] = &[
    ("domain", &["domain"]),
    ("providers", &["domain", "providers", "git_url"]),
    ("session", &["domain", "providers", "git_url", "session"]),
    ("tui", &["app", "domain", "tui"]),
];

/// The doubles are for tests, and tests reach into any layer for them.
const EVERYWHERE: &str = "test_support";

/// Test code is not held to the layers: a file or directory named for tests.
fn is_test_file(path: &Path) -> bool {
    let named_for_tests = |name: &str| {
        matches!(name, "tests" | "regression_tests" | "testdata")
            || name.ends_with("_tests")
            || name.ends_with("_tests.rs")
            || name == "tests.rs"
    };
    path.components()
        .any(|part| part.as_os_str().to_str().is_some_and(named_for_tests))
}

/// The first segment of every `crate::` path in the non-test part of a file,
/// grouped imports (`use crate::{app::x, tui::y}`) included. A test module is
/// last in a file, which clippy's `items_after_test_module` holds.
fn crate_owners(source: &str) -> Vec<String> {
    let code = source
        .split_once("#[cfg(test)]\nmod tests")
        .map_or(source, |(code, _)| code);
    // Only a whole-line comment is dropped: a `//` inside a string must not hide
    // the rest of its line. A trailing comment that names a path is reported,
    // and the report says where.
    let without_comments = code
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    let mut owners: Vec<String> = Vec::new();
    for after in without_comments.split("crate::").skip(1) {
        let Some(group) = after.strip_prefix('{') else {
            owners.push(after.chars().take_while(|c| is_ident(*c)).collect());
            continue;
        };
        // One owner per top-level branch of the group, however deep it nests.
        let (mut depth, mut reading) = (1_usize, true);
        let mut owner = String::new();
        for c in group.chars() {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                ',' if depth == 1 => reading = true,
                c if depth == 1 && reading && is_ident(c) => owner.push(c),
                _ if depth == 1 && !owner.is_empty() => reading = false,
                _ if depth == 1 && reading && !c.is_whitespace() => reading = false,
                _ => {}
            }
            if depth == 0 {
                break;
            }
            if !reading || c == ',' || c == '}' {
                owners.extend((!owner.is_empty()).then(|| std::mem::take(&mut owner)));
            }
        }
        owners.extend((!owner.is_empty()).then_some(owner));
    }
    owners.retain(|owner| !owner.is_empty());
    owners
}

#[test]
fn the_scan_reads_plain_and_grouped_paths() {
    let source = "use crate::domain::pr;\n\
        use crate::{\n    app::{a, b},\n    tui::c,\n    domain,\n};\n\
        use crate::{domain, git_url::parse};\n\
        // crate::providers in a comment\n\
        fn f() { crate::git_url::parse(); }\n\
        fn g() { format!(\"https://{}\", crate::app::v()); }\n\
        #[cfg(test)]\nmod tests { use crate::providers::X; }\n";
    assert_eq!(
        crate_owners(source),
        [
            "domain", "app", "tui", "domain", "domain", "git_url", "git_url", "app"
        ]
    );
}

#[test]
fn each_layer_imports_only_from_the_layers_it_may() -> io::Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut broken = Vec::new();
    let mut seen_in_tui = Vec::new();
    for (layer, allowed) in IMPORTS {
        let mut files = Vec::new();
        rust_files(&root.join(layer), &mut files)?;
        assert!(!files.is_empty(), "found no Rust files in src/{layer}");
        for file in files.iter().filter(|file| !is_test_file(file)) {
            for owner in crate_owners(&fs::read_to_string(file)?) {
                if *layer == "tui" {
                    seen_in_tui.push(owner.clone());
                }
                if owner != EVERYWHERE && !allowed.contains(&owner.as_str()) {
                    let name = file.strip_prefix(&root).unwrap_or(file);
                    broken.push(format!("  {} imports {owner}", name.display()));
                }
            }
        }
    }
    // The scan must find what is there, or it passes by reading nothing.
    for expected in ["app", "domain"] {
        assert!(
            seen_in_tui.iter().any(|owner| owner == expected),
            "the scan found no `crate::{expected}` in src/tui"
        );
    }
    assert!(
        broken.is_empty(),
        "a layer imports from one it may not (AGENTS.md, *Modules*; ARCHITECTURE.md, \
         *Import types from their owners*):\n{}",
        broken.join("\n")
    );
    Ok(())
}

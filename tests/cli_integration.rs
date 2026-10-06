//! Drives the built `slussa` binary as a subprocess. Every test runs in an
//! isolated temporary directory with its own HOME and config, outside any git
//! checkout, and none of them touch the network or start the TUI.

#![expect(
    clippy::expect_used,
    reason = "the sandbox helpers are test code outside a `#[test]` function"
)]

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

struct Output {
    code: i32,
    combined: String,
}

/// A unique directory under the system temp dir, removed on drop. It is the
/// git ceiling too, so a parent repository can never be discovered.
struct Sandbox {
    root: PathBuf,
    work: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "slussa-it-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let work = root.join("work");
        std::fs::create_dir_all(&work).expect("create sandbox");
        Self { root, work }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_slussa"));
        command
            .env("HOME", &self.root)
            .env("XDG_CONFIG_HOME", &self.root)
            .env("GIT_CEILING_DIRECTORIES", &self.root)
            .env_remove("SLUSSA_THEME")
            .stdin(Stdio::null());
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        let output = self.command().args(args).output().expect("run slussa");
        Output {
            code: output.status.code().unwrap_or(-1),
            combined: format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        }
    }

    fn run_in_work(&self, args: &[&str]) -> Output {
        let work = self.work.to_str().expect("utf-8 path");
        let mut full = vec!["-C", work];
        full.extend_from_slice(args);
        self.run(&full)
    }

    /// Run with `input` on standard input, as an agent that pipes a document.
    fn run_with_input(&self, args: &[&str], input: &str) -> Output {
        use std::io::Write;
        let mut child = self
            .command()
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("run slussa");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(input.as_bytes())
            .expect("write the document");
        let output = child.wait_with_output().expect("wait for slussa");
        Output {
            code: output.status.code().unwrap_or(-1),
            combined: format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        }
    }

    fn git(&self, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(&self.work)
            .env("HOME", &self.root)
            .env("GIT_CEILING_DIRECTORIES", &self.root)
            .status()
            .expect("run git");
        assert!(status.success(), "git {args:?}");
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(Path::new(&self.root));
    }
}

#[test]
fn help_lists_every_command() {
    let output = Sandbox::new().run(&["--help"]);
    assert_eq!(output.code, 0, "{}", output.combined);
    for expected in ["slussa", "-C <dir>", "auth login", "--version", "--help"] {
        assert!(
            output.combined.contains(expected),
            "missing {expected:?} in:\n{}",
            output.combined
        );
    }
}

#[test]
fn short_help_flag_matches_long() {
    let sandbox = Sandbox::new();
    assert_eq!(
        sandbox.run(&["-h"]).combined,
        sandbox.run(&["--help"]).combined
    );
}

#[test]
fn version_prints_the_cargo_version_under_both_spellings() {
    let sandbox = Sandbox::new();
    for flag in ["--version", "-V"] {
        let output = sandbox.run(&[flag]);
        assert_eq!(output.code, 0, "{flag}: {}", output.combined);
        assert_eq!(
            output.combined.trim(),
            format!("slussa {}", env!("CARGO_PKG_VERSION"))
        );
    }
}

#[test]
fn unknown_command_exits_with_usage_error() {
    let output = Sandbox::new().run(&["bogus"]);
    assert_eq!(output.code, 2);
    assert!(output.combined.contains("unknown command `bogus`"));
    assert!(output.combined.contains("slussa --help"));
}

#[test]
fn chdir_flag_requires_a_directory() {
    let output = Sandbox::new().run(&["-C"]);
    assert_eq!(output.code, 2);
    assert!(output.combined.contains("`-C` requires a directory"));
}

#[test]
fn chdir_flag_reports_a_missing_directory() {
    let sandbox = Sandbox::new();
    let missing = sandbox.root.join("does-not-exist");
    let output = sandbox.run(&["-C", missing.to_str().expect("utf-8 path")]);
    assert_eq!(output.code, 1);
    assert!(output.combined.contains("couldn't chdir"));
}

#[test]
fn auth_login_needs_a_repository_too() {
    let output = Sandbox::new().run_in_work(&["auth", "login"]);
    assert_eq!(output.code, 1);
    assert!(
        output
            .combined
            .contains("must be run inside a git repository")
    );
}

#[test]
fn auth_login_on_github_points_to_gh_instead_of_asking_for_a_token() {
    let sandbox = Sandbox::new();
    sandbox.git(&["init", "-q"]);
    sandbox.git(&["remote", "add", "origin", "git@github.com:owner/repo.git"]);
    let output = sandbox.run_in_work(&["auth", "login"]);
    assert_eq!(output.code, 1);
    assert!(
        output.combined.contains("gh auth login"),
        "{}",
        output.combined
    );
    assert!(
        !output.combined.contains("access token"),
        "no Bitbucket instructions are shown: {}",
        output.combined
    );
}

#[test]
fn auth_login_on_bitbucket_cloud_says_it_is_unsupported() {
    let sandbox = Sandbox::new();
    sandbox.git(&["init", "-q"]);
    sandbox.git(&["remote", "add", "origin", "git@bitbucket.org:team/repo.git"]);
    let output = sandbox.run_in_work(&["auth", "login"]);
    assert_eq!(output.code, 1);
    assert!(
        output.combined.contains("Bitbucket Cloud"),
        "{}",
        output.combined
    );
    assert!(
        !output.combined.contains("access token"),
        "no token instructions are shown: {}",
        output.combined
    );
}

#[test]
fn without_a_terminal_it_exits_2_before_any_preflight() {
    let sandbox = Sandbox::new();
    // A repository whose remote preflight would reject: the terminal check
    // must come first, so that message never appears.
    sandbox.git(&["init", "-q"]);
    sandbox.git(&["remote", "add", "origin", "not-a-url"]);
    let output = sandbox.run_in_work(&[]);
    assert_eq!(output.code, 2, "{}", output.combined);
    assert!(
        output.combined.contains("needs a terminal"),
        "{}",
        output.combined
    );
    assert!(output.combined.contains("slussa --help"));
    assert!(
        !output.combined.contains("not-a-url"),
        "{}",
        output.combined
    );
    assert!(!output.combined.contains('\u{1b}'), "no escape bytes");
}

#[test]
fn help_lists_the_command_that_opens_a_pr_by_its_number() {
    let output = Sandbox::new().run(&["--help"]);
    assert!(
        output.combined.contains("slussa <number>"),
        "{}",
        output.combined
    );
}

#[test]
fn a_pr_number_needs_a_terminal_like_the_list_does_and_is_taken_with_or_without_a_hash() {
    let sandbox = Sandbox::new();
    for argument in ["44", "#44"] {
        let output = sandbox.run_in_work(&[argument]);
        assert_eq!(output.code, 2, "{argument}: {}", output.combined);
        assert!(
            output.combined.contains("needs a terminal"),
            "{argument}: {}",
            output.combined
        );
        assert!(
            !output.combined.contains("unknown command"),
            "{argument}: {}",
            output.combined
        );
    }
}

#[test]
fn a_pr_number_with_something_after_it_is_refused() {
    let output = Sandbox::new().run_in_work(&["44", "extra"]);
    assert_eq!(output.code, 2, "{}", output.combined);
    assert!(
        output.combined.contains("unexpected argument `extra`"),
        "{}",
        output.combined
    );
}

#[test]
fn a_word_that_is_not_a_number_is_still_an_unknown_command() {
    let output = Sandbox::new().run(&["4x4"]);
    assert_eq!(output.code, 2);
    assert!(output.combined.contains("unknown command `4x4`"));
}

#[test]
fn help_lists_the_command_an_agent_proposes_with() {
    let output = Sandbox::new().run(&["--help"]);
    assert!(
        output.combined.contains("propose import <PR>"),
        "{}",
        output.combined
    );
}

#[test]
fn propose_without_a_pr_is_a_usage_error_told_as_json_on_stderr() {
    for args in [
        &["propose"][..],
        &["propose", "import"],
        &["propose", "import", "x"],
        &["propose", "list"],
    ] {
        let output = Sandbox::new().run(args);
        assert_eq!(output.code, 2, "{args:?}: {}", output.combined);
        let error: serde_json::Value = serde_json::from_str(output.combined.trim())
            .unwrap_or_else(|e| panic!("{args:?}: not JSON ({e}): {}", output.combined));
        assert_eq!(error["schema"], 1);
        assert_eq!(error["error"]["kind"], "usage");
    }
}

#[test]
fn a_document_that_is_not_one_is_refused_before_anything_is_asked_of_the_network() {
    let sandbox = Sandbox::new();
    for (input, wanted) in [
        ("not json", "cannot be read"),
        (r#"{"schema": 2, "head": "abc123"}"#, "reads 1"),
        (
            r#"{"schema": 1, "head": "abc123", "comments": [{"path": "a", "line": 0, "body": "x"}]}"#,
            "comments[0]",
        ),
    ] {
        let output = sandbox.run_with_input(&["propose", "import", "44"], input);
        assert_eq!(output.code, 2, "{input}: {}", output.combined);
        assert!(output.combined.contains(wanted), "{}", output.combined);
        assert!(
            output.combined.contains(r#""kind":"invalid""#),
            "{}",
            output.combined
        );
    }
}

#[test]
fn a_document_that_cannot_be_read_from_a_file_is_a_failure_not_a_usage_error() {
    let output = Sandbox::new().run(&[
        "propose",
        "import",
        "44",
        "--file",
        "/nonexistent/review.json",
    ]);
    assert_eq!(output.code, 1, "{}", output.combined);
    assert!(
        output.combined.contains(r#""kind":"failed""#),
        "{}",
        output.combined
    );
}

#[test]
fn help_lists_the_commands_an_agent_reads_a_pr_and_learns_of_slussa_with() {
    let output = Sandbox::new().run(&["--help"]);
    for wanted in ["slussa context <PR>", "slussa agent-instructions"] {
        assert!(output.combined.contains(wanted), "{}", output.combined);
    }
}

#[test]
fn context_without_one_pr_is_a_usage_error_told_as_json_on_stderr() {
    for args in [&["context"][..], &["context", "x"], &["context", "4", "5"]] {
        let output = Sandbox::new().run(args);
        assert_eq!(output.code, 2, "{args:?}: {}", output.combined);
        let error: serde_json::Value = serde_json::from_str(output.combined.trim())
            .unwrap_or_else(|e| panic!("{args:?}: not JSON ({e}): {}", output.combined));
        assert_eq!(error["error"]["kind"], "usage");
    }
}

#[test]
fn agent_instructions_name_the_two_commands_and_that_nothing_is_posted() {
    let output = Sandbox::new().run(&["agent-instructions"]);
    assert_eq!(output.code, 0, "{}", output.combined);
    for wanted in [
        "slussa context <PR>",
        "slussa propose import <PR>",
        "do not post comments",
        "lines_checked",
    ] {
        assert!(output.combined.contains(wanted), "missing {wanted:?}");
    }
    assert_eq!(Sandbox::new().run(&["agent-instructions", "x"]).code, 2);
}

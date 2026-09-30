//! Drives the built `tuipr` binary as a subprocess. Every test runs in an
//! isolated temporary directory with its own HOME and config, outside any git
//! checkout, and none of them touch the network or start the TUI.

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
            "tuipr-it-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let work = root.join("work");
        std::fs::create_dir_all(&work).expect("create sandbox");
        Self { root, work }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tuipr"));
        command
            .env("HOME", &self.root)
            .env("XDG_CONFIG_HOME", &self.root)
            .env("GIT_CEILING_DIRECTORIES", &self.root)
            .env_remove("TUIPR_THEME")
            .stdin(Stdio::null());
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        let output = self.command().args(args).output().expect("run tuipr");
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
    for expected in ["tuipr", "-C <dir>", "auth login", "--help"] {
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
fn unknown_command_exits_with_usage_error() {
    let output = Sandbox::new().run(&["bogus"]);
    assert_eq!(output.code, 2);
    assert!(output.combined.contains("unknown command `bogus`"));
    assert!(output.combined.contains("tuipr --help"));
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
fn outside_a_git_repository_is_explained() {
    let output = Sandbox::new().run_in_work(&[]);
    assert_eq!(output.code, 1);
    assert!(
        output
            .combined
            .contains("must be run inside a git repository")
    );
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
fn unparseable_remote_is_reported_with_the_remote() {
    let sandbox = Sandbox::new();
    sandbox.git(&["init", "-q"]);
    sandbox.git(&["remote", "add", "origin", "not-a-url"]);
    let output = sandbox.run_in_work(&[]);
    assert_eq!(output.code, 1);
    assert!(output.combined.contains("couldn't parse a host"));
    assert!(output.combined.contains("not-a-url"));
}

#[test]
fn bitbucket_cloud_is_reported_as_unsupported() {
    let sandbox = Sandbox::new();
    sandbox.git(&["init", "-q"]);
    sandbox.git(&["remote", "add", "origin", "git@bitbucket.org:team/repo.git"]);
    let output = sandbox.run_in_work(&[]);
    assert_eq!(output.code, 1);
    assert!(output.combined.contains("Bitbucket Cloud"));
    assert!(output.combined.contains("isn't supported yet"));
}

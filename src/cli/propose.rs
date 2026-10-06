//! `slussa propose import <PR>`: what an agent proposes on a PR, read from a
//! JSON document and kept for the reader, who sends, edits or discards each
//! proposal in the TUI. Nothing is posted. It prints and exits, and never asks
//! for input: it fails when the account is not logged in.
//!
//! A document of the PR's head is checked against the PR's diff before it is
//! kept: a comment on a line that is not in it could never be shown, so the
//! document is refused and the lines are named, for the agent to correct. A
//! `head` written short is written out when it is the PR's head.
//!
//! The document is the public contract, and `"schema": 1` is marked experimental
//! until it is used. Its types are separate from the domain's, so a change inside
//! does not change what an agent writes.

use std::{
    io::{IsTerminal, Read},
    path::{Path, PathBuf},
    process::ExitCode,
};

use serde::Serialize;

use super::{
    check,
    exit::{self, Failure, Kind},
};
use crate::{
    domain::{
        commit::CommitOid,
        pr::PrId,
        proposal_document::{self, Parsed, Source},
    },
    local::{
        proposals::{Batch, Imported, import},
        scope::scope,
    },
    session::remote,
};

/// The most a document may hold, so that an agent that loops cannot fill the
/// disk or the screen.
const MAX_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Serialize)]
struct Report {
    schema: u32,
    pr: u64,
    added: usize,
    duplicates: usize,
    head: String,
    /// The PR's head now, when the provider says.
    current_head: Option<String>,
    /// Whether the PR has moved since the agent read it: what it proposes is then
    /// shown as written against another commit.
    stale: bool,
    /// Whether the comments were checked to be on lines of the PR's diff. They
    /// are when `head` is the PR's head; an older commit's diff is not read.
    lines_checked: bool,
}

enum Command {
    Import { pr: PrId, file: Option<PathBuf> },
}

pub(super) fn run(args: &[String]) -> ExitCode {
    match run_import(args) {
        Ok(report) => {
            println!("{}", serde_json::to_string(&report).unwrap_or_default());
            ExitCode::SUCCESS
        }
        Err(failure) => exit::report("propose", &failure),
    }
}

fn parse_args(args: &[String]) -> Result<Command, Failure> {
    let usage = || {
        Failure::new(
            Kind::Usage,
            "usage: slussa propose import <PR> [--file <path>]  (the document is read from standard input without --file)",
        )
    };
    let mut args = args.iter();
    match args.next().map(String::as_str) {
        Some("import") => {}
        Some(other) => {
            return Err(Failure::new(
                Kind::Usage,
                format!("unknown propose subcommand `{other}`; try `slussa propose import`"),
            ));
        }
        None => return Err(usage()),
    }
    let pr = args
        .next()
        .and_then(|text| PrId::parse(text))
        .ok_or_else(usage)?;
    let mut file = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--file" => file = Some(PathBuf::from(args.next().ok_or_else(usage)?)),
            _ => return Err(usage()),
        }
    }
    Ok(Command::Import { pr, file })
}

fn read_document(file: Option<&Path>) -> Result<Vec<u8>, Failure> {
    let mut bytes = Vec::new();
    let read = if let Some(path) = file {
        std::fs::File::open(path)
            .and_then(|file| file.take(MAX_BYTES + 1).read_to_end(&mut bytes))
            .map_err(|e| {
                Failure::new(Kind::Failed, format!("cannot read {}: {e}", path.display()))
            })?
    } else {
        if std::io::stdin().is_terminal() {
            return Err(Failure::new(
                Kind::Usage,
                "pipe the document to standard input, or name it with --file",
            ));
        }
        std::io::stdin()
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| Failure::new(Kind::Failed, format!("cannot read standard input: {e}")))?
    };
    if read as u64 > MAX_BYTES {
        return Err(Failure::new(
            Kind::Invalid,
            format!("the document is larger than {MAX_BYTES} bytes"),
        ));
    }
    Ok(bytes)
}

/// What the document proposes, each part checked: the one that is wrong is
/// named, and nothing of a document with one is kept.
fn parse_document(bytes: &[u8]) -> Result<Parsed, Failure> {
    proposal_document::parse(bytes, Source::Caller)
        .map_err(|message| Failure::new(Kind::Invalid, message))
}

fn import_batch(
    root: &Path,
    scope_name: &str,
    pr: PrId,
    head: &CommitOid,
    batch: Batch,
    current_head: Option<&str>,
) -> Result<Report, Failure> {
    let Imported {
        added, duplicates, ..
    } = import(root, scope_name, pr, batch)
        .map_err(|e| Failure::new(Kind::Failed, e.to_string()))?;
    let current = current_head.and_then(CommitOid::parse);
    Ok(Report {
        schema: 1,
        pr: pr.0,
        added,
        duplicates,
        head: head.to_string(),
        stale: current.as_ref().is_some_and(|current| current != head),
        current_head: current.map(|current| current.to_string()),
        lines_checked: false,
    })
}

fn run_import(args: &[String]) -> Result<Report, Failure> {
    let Command::Import { pr, file } = parse_args(args)?;
    let parsed = parse_document(&read_document(file.as_deref())?)?;

    let session = exit::connect()?;
    let found = session
        .provider()
        .fetch_pr(pr)
        .map_err(|e| Failure::unread_pr(pr, &e))?;
    let (parsed, lines_checked) =
        check::checked(session.provider(), pr, parsed, found.head_oid.as_deref())?;
    let head = parsed.head.clone();
    let batch = Batch {
        comments: parsed.comments,
        summary: parsed.summary,
    };
    let origin = remote::origin_url().map_err(|e| Failure::new(Kind::Failed, e.to_string()))?;
    let scope_name = scope(session.provider(), &origin, session.user())
        .map_err(|e| Failure::new(Kind::Failed, e.to_string()))?;
    let root = dirs::data_local_dir()
        .ok_or_else(|| Failure::new(Kind::Failed, "cannot locate the local data directory"))?
        .join("slussa/proposals");
    let report = Report {
        lines_checked,
        ..import_batch(
            &root,
            &scope_name,
            pr,
            &head,
            batch,
            found.head_oid.as_deref(),
        )?
    };
    tracing::info!(
        "propose import: pr={pr} added={} duplicates={} stale={} lines_checked={}",
        report.added,
        report.duplicates,
        report.stale,
        report.lines_checked
    );
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|a| (*a).to_owned()).collect()
    }

    const DOCUMENT: &str = r#"{
        "schema": 1, "head": "abc123", "agent": "reviewer", "summary": "Two things to look at.",
        "comments": [
            {"path": "src/a.rs", "line": 12, "body": "This can panic.", "id": "F1"},
            {"path": "src/a.rs", "line": 3, "side": "old", "body": "Why was this removed?"}
        ]
    }"#;

    fn document() -> (CommitOid, Batch) {
        let parsed = parse_document(DOCUMENT.as_bytes())
            .ok()
            .expect("a document");
        (
            parsed.head,
            Batch {
                comments: parsed.comments,
                summary: parsed.summary,
            },
        )
    }

    fn kind_of(result: Result<impl Sized, Failure>) -> Kind {
        result.err().map(|failure| failure.kind).expect("a failure")
    }

    #[test]
    fn the_command_line_names_a_pr_and_perhaps_a_file() {
        let Ok(Command::Import { pr, file }) =
            parse_args(&args(&["import", "#44", "--file", "r.json"]))
        else {
            panic!("parsed");
        };
        assert_eq!((pr, file), (PrId(44), Some(PathBuf::from("r.json"))));
        for wrong in [
            &[][..],
            &["import"],
            &["import", "x"],
            &["import", "4", "--what"],
            &["list"],
        ] {
            assert_eq!(kind_of(parse_args(&args(wrong))), Kind::Usage, "{wrong:?}");
        }
        assert_eq!(
            kind_of(parse_args(&args(&["import", "4", "--file"]))),
            Kind::Usage
        );
    }

    #[test]
    fn a_document_becomes_proposals_bound_to_its_head() {
        let (head, batch) = document();
        assert_eq!(head.as_str(), "abc123");
        assert_eq!(batch.comments.len(), 2);
        assert!(batch.summary.is_some());
    }

    #[test]
    fn what_is_wrong_with_a_document_is_named_and_none_of_it_is_kept() {
        let wrong = |text: &str| {
            parse_document(text.as_bytes())
                .err()
                .map(|failure| (failure.kind, failure.message))
                .expect("a failure")
        };
        let (kind, message) = wrong("not json");
        assert_eq!(kind, Kind::Invalid);
        assert!(message.contains("cannot be read"), "{message}");
        assert!(
            wrong(r#"{"schema": 2, "head": "abc123"}"#)
                .1
                .contains("reads 1")
        );
        assert!(
            wrong(r#"{"schema": 1, "head": "main"}"#)
                .1
                .contains("`head`")
        );
        let (_, message) = wrong(
            r#"{"schema": 1, "head": "abc123", "comments": [
                {"path": "a", "line": 1, "body": "ok"},
                {"path": "a", "line": 0, "body": "no"}]}"#,
        );
        assert!(
            message.contains("comments[1]") && message.contains("`line`"),
            "{message}"
        );
        // A field nobody asked for is a mistake caught, not a field ignored.
        assert!(
            wrong(r#"{"schema": 1, "head": "abc123", "coments": []}"#)
                .1
                .contains("coments")
        );
    }

    #[test]
    fn a_report_says_whether_the_pr_has_moved_since_the_agent_read_it() {
        let dir = TempDir::new("propose");
        let (head, batch) = document();
        let report = import_batch(dir.path(), "s", PrId(44), &head, batch, Some("def456"))
            .ok()
            .expect("imported");
        assert_eq!((report.added, report.duplicates), (3, 0));
        assert!(report.stale);
        assert_eq!(report.current_head.as_deref(), Some("def456"));

        // The same document again adds nothing, and the PR has not moved.
        let (head, batch) = document();
        let report = import_batch(dir.path(), "s", PrId(44), &head, batch, Some("abc123"))
            .ok()
            .expect("imported");
        assert_eq!((report.added, report.duplicates), (0, 3));
        assert!(!report.stale);
    }
}

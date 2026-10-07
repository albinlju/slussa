//! The file an agent's proposals are kept in: one JSON file per scope, holding
//! what agents have proposed on each PR and nothing the reader has done with
//! it. An agent writes it with `slussa propose import`, which takes the lock
//! for the moment it needs; the TUI reads it without taking the lock, which is
//! safe because a write replaces the whole file at once.
//!
//! What the reader does with a proposal (sends, edits, discards it) is kept
//! elsewhere, by the TUI, so that two processes never write the same file for
//! long.
//!
//! The file does not grow for ever: a review of the PR's head replaces what was
//! proposed on its older commits, which can no longer be shown on their lines,
//! and a PR nothing has been handed in on for ninety days is forgotten.

use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

pub use super::file::Stamp;
use super::file::{ScopedFile, path_of};
use crate::domain::{
    commit::CommitOid,
    pr::PrId,
    proposal::{Proposal, Summary},
};

/// How long an import waits for another one to finish.
const WAIT: Duration = Duration::from_secs(5);

/// How many proposals a PR may hold, so that an agent that loops cannot fill the
/// disk.
pub const MAX_PER_PR: usize = 1_000;

/// How long a PR is kept after the last time something was handed in on it, so
/// that the file does not grow with every PR ever reviewed. The same as what
/// the reader did with its proposals is kept (`domain::seen`).
const KEEP: TimeDelta = TimeDelta::days(90);

/// Where the proposals are kept on this machine: the one place the import
/// writes and the TUI reads, so that neither can look somewhere else.
pub fn root() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join("slussa/proposals"))
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForPr {
    #[serde(default)]
    pub comments: Vec<Proposal>,
    #[serde(default)]
    pub summaries: Vec<Summary>,
    /// When something was last handed in on the PR. A file from before it was
    /// kept has none, and one with none is written without it, so the version 1
    /// text stays as it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<DateTime<Utc>>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Proposals(BTreeMap<PrId, ForPr>);

impl Proposals {
    /// What a test says is proposed on `pr`, without a file.
    #[cfg(test)]
    pub fn of(pr: PrId, held: ForPr) -> Self {
        Self(BTreeMap::from([(pr, held)]))
    }

    pub fn for_pr(&self, pr: PrId) -> Option<&ForPr> {
        self.0.get(&pr)
    }

    /// Forget the PRs nothing has been handed in on for the time kept, as of
    /// `now`. One with no date is from before dates were kept: it is given this
    /// one, and counted from here.
    fn forget_old(&mut self, now: DateTime<Utc>) {
        self.0
            .retain(|_, held| now - *held.at.get_or_insert(now) <= KEEP);
    }
}

#[derive(Serialize, Deserialize)]
struct Envelope {
    version: u32,
    scope: String,
    proposals: Proposals,
}

/// What one agent hands in about one PR.
#[derive(Debug, Default)]
pub struct Batch {
    pub comments: Vec<Proposal>,
    pub summary: Option<Summary>,
    /// The PR's head, when the batch is what an agent makes of that commit. It
    /// then replaces what was proposed on other commits, which no diff the
    /// reader has can show any more.
    pub current: Option<CommitOid>,
}

/// What became of the summary a batch held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummaryKept {
    /// The batch had none.
    None,
    Added,
    /// The same words on the same commit were there already.
    Duplicate,
}

/// What an import did. The comments are counted apart from the summary, so that
/// the one who handed in two comments is told of two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Imported {
    /// Comments added.
    pub comments: usize,
    /// Comments that were already there.
    pub duplicates: usize,
    pub summary: SummaryKept,
}

fn parse(bytes: &[u8], scope: &str, path: &Path) -> io::Result<Proposals> {
    let data: Envelope = serde_json::from_slice(bytes).map_err(|e| {
        io::Error::other(format!(
            "Cannot read {} (file preserved): {e}",
            path.display()
        ))
    })?;
    if data.version != 1 || data.scope != scope {
        return Err(io::Error::other(format!(
            "Unsupported file or repository mismatch at {}",
            path.display()
        )));
    }
    Ok(data.proposals)
}

/// What is proposed under `scope`, without taking the lock.
pub fn read(root: &Path, scope: &str) -> io::Result<Proposals> {
    let path = path_of(root, scope);
    match std::fs::read(&path) {
        Ok(bytes) => parse(&bytes, scope, &path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Proposals::default()),
        Err(error) => Err(error),
    }
}

/// As `read`, when the file is another than the one `stamp` is of, and `None`
/// when it is the same and nothing was read. The reader asks each time a PR is
/// opened or refreshed, and the file changes only when an agent hands
/// something in.
pub fn read_changed(root: &Path, scope: &str, stamp: &mut Stamp) -> io::Result<Option<Proposals>> {
    let found = Stamp::of(root, scope)?;
    if found == *stamp {
        return Ok(None);
    }
    let proposals = read(root, scope)?;
    *stamp = found;
    Ok(Some(proposals))
}

/// Add what an agent proposes on `pr` to the file for `scope`. What is there
/// already is kept, and a proposal that is there is not added again. An error
/// leaves the file as it was, including one that cannot be read: it is not
/// written over.
pub fn import(root: &Path, scope: &str, pr: PrId, batch: Batch) -> io::Result<Imported> {
    import_at(root, scope, pr, batch, Utc::now())
}

/// As `import`, at the time `now`.
fn import_at(
    root: &Path,
    scope: &str,
    pr: PrId,
    batch: Batch,
    now: DateTime<Utc>,
) -> io::Result<Imported> {
    let (mut file, bytes) = ScopedFile::open_waiting(root, scope, WAIT)?;
    let mut all = match &bytes {
        Some(bytes) => parse(bytes, scope, file.path())?,
        None => Proposals::default(),
    };
    all.forget_old(now);
    let held = all.0.entry(pr).or_default();
    held.at = Some(now);
    if let Some(current) = &batch.current {
        held.comments.retain(|known| known.head() == current);
        held.summaries.retain(|known| known.head() == current);
    }
    let mut imported = Imported {
        comments: 0,
        duplicates: 0,
        summary: SummaryKept::None,
    };
    for comment in batch.comments {
        if held.comments.iter().any(|known| known.same_as(&comment)) {
            imported.duplicates += 1;
        } else if held.comments.len() >= MAX_PER_PR {
            return Err(io::Error::other(format!(
                "PR #{pr} already holds {MAX_PER_PR} proposals, the most that are kept; a review \
                 of a newer commit replaces them"
            )));
        } else {
            held.comments.push(comment);
            imported.comments += 1;
        }
    }
    if let Some(summary) = batch.summary {
        if held.summaries.iter().any(|known| known.same_as(&summary)) {
            imported.summary = SummaryKept::Duplicate;
        } else {
            held.summaries.push(summary);
            imported.summary = SummaryKept::Added;
        }
    }
    let bytes = serde_json::to_vec(&Envelope {
        version: 1,
        scope: scope.to_owned(),
        proposals: all,
    })?;
    file.write(bytes)?;
    Ok(imported)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::proposal::{ProposalInput, Side},
        test_support::TempDir,
    };

    fn input(path: &str, line: usize, body: &str) -> ProposalInput {
        ProposalInput {
            head: "abc123".into(),
            path: path.into(),
            line,
            side: Side::New,
            body: body.into(),
            id: None,
            agent: Some("reviewer".into()),
        }
    }

    fn comment(path: &str, line: usize, body: &str) -> Proposal {
        Proposal::new(input(path, line, body)).unwrap()
    }

    fn batch(comments: Vec<Proposal>) -> Batch {
        Batch {
            comments,
            ..Batch::default()
        }
    }

    fn of(head: &str, line: usize) -> Proposal {
        Proposal::new(ProposalInput {
            head: head.into(),
            ..input("a.rs", line, "x")
        })
        .unwrap()
    }

    fn day(n: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(1_790_000_000, 0).unwrap() + TimeDelta::days(n)
    }

    #[test]
    fn what_an_agent_proposes_is_kept_for_the_pr_and_read_back_without_the_lock() {
        let dir = TempDir::new("proposals");
        let imported = import(
            dir.path(),
            "repo/a",
            PrId(7),
            batch(vec![comment("a.rs", 1, "one"), comment("a.rs", 2, "two")]),
        )
        .unwrap();
        assert_eq!(
            imported,
            Imported {
                comments: 2,
                duplicates: 0,
                summary: SummaryKept::None
            }
        );

        let read = read(dir.path(), "repo/a").unwrap();
        assert_eq!(read.for_pr(PrId(7)).map(|p| p.comments.len()), Some(2));
        assert_eq!(read.for_pr(PrId(8)), None);
        // Another repository has a file of its own.
        assert!(
            super::read(dir.path(), "repo/b")
                .unwrap()
                .for_pr(PrId(7))
                .is_none()
        );
    }

    #[test]
    fn the_same_proposal_sent_again_is_not_added_and_an_import_adds_to_what_is_there() {
        let dir = TempDir::new("proposals");
        import(
            dir.path(),
            "s",
            PrId(7),
            batch(vec![comment("a.rs", 1, "one")]),
        )
        .unwrap();
        let again = import(
            dir.path(),
            "s",
            PrId(7),
            batch(vec![comment("a.rs", 1, "one"), comment("b.rs", 4, "three")]),
        )
        .unwrap();
        assert_eq!(
            again,
            Imported {
                comments: 1,
                duplicates: 1,
                summary: SummaryKept::None
            }
        );
        let held = read(dir.path(), "s").unwrap();
        assert_eq!(held.for_pr(PrId(7)).unwrap().comments.len(), 2);
    }

    #[test]
    fn a_summary_is_kept_once_per_commit_and_its_words() {
        let dir = TempDir::new("proposals");
        let summary = || Summary::new("abc123", "Looks fine.".into(), None).unwrap();
        let with_summary = || Batch {
            summary: Some(summary()),
            ..Batch::default()
        };
        assert_eq!(
            import(dir.path(), "s", PrId(7), with_summary()).unwrap(),
            Imported {
                comments: 0,
                duplicates: 0,
                summary: SummaryKept::Added
            }
        );
        assert_eq!(
            import(dir.path(), "s", PrId(7), with_summary()).unwrap(),
            Imported {
                comments: 0,
                duplicates: 0,
                summary: SummaryKept::Duplicate
            }
        );
    }

    #[test]
    fn a_pr_takes_no_more_than_its_share_and_the_file_is_left_as_it_was() {
        let dir = TempDir::new("proposals");
        let many: Vec<Proposal> = (1..=MAX_PER_PR)
            .map(|line| comment("a.rs", line, "x"))
            .collect();
        import(dir.path(), "s", PrId(7), batch(many)).unwrap();
        let before = std::fs::read(path_of(dir.path(), "s")).unwrap();
        let error = import(
            dir.path(),
            "s",
            PrId(7),
            batch(vec![comment("b.rs", 1, "y")]),
        )
        .unwrap_err();
        assert!(error.to_string().contains("already holds"), "{error}");
        assert_eq!(std::fs::read(path_of(dir.path(), "s")).unwrap(), before);

        // The branch moves and the PR is reviewed again: there is room.
        let again = Batch {
            comments: vec![of("def456", 1)],
            current: CommitOid::parse("def456"),
            ..Batch::default()
        };
        assert_eq!(import(dir.path(), "s", PrId(7), again).unwrap().comments, 1);
    }

    #[test]
    fn a_review_of_the_prs_head_replaces_what_was_proposed_on_other_commits() {
        let dir = TempDir::new("proposals");
        let older = Batch {
            comments: vec![of("abc123", 1), of("abc123", 2)],
            summary: Some(Summary::new("abc123", "Then.".into(), None).unwrap()),
            current: CommitOid::parse("abc123"),
        };
        import(dir.path(), "s", PrId(7), older).unwrap();
        // One of a commit that is not the head is kept beside what is there.
        import(dir.path(), "s", PrId(7), batch(vec![of("0ddba11", 1)])).unwrap();
        let held = read(dir.path(), "s").unwrap();
        assert_eq!(held.for_pr(PrId(7)).unwrap().comments.len(), 3);

        let newer = Batch {
            comments: vec![of("def456", 5)],
            summary: Some(Summary::new("def456", "Now.".into(), None).unwrap()),
            current: CommitOid::parse("def456"),
        };
        import(dir.path(), "s", PrId(7), newer).unwrap();
        let held = read(dir.path(), "s").unwrap();
        let held = held.for_pr(PrId(7)).unwrap();
        assert_eq!(held.comments, vec![of("def456", 5)]);
        assert_eq!(held.summaries.len(), 1, "and its summary with them");
    }

    #[test]
    fn a_pr_nothing_was_handed_in_on_for_ninety_days_is_forgotten_at_the_next_import() {
        let dir = TempDir::new("proposals");
        let one = || batch(vec![comment("a.rs", 1, "one")]);
        import_at(dir.path(), "s", PrId(1), one(), day(0)).unwrap();
        import_at(dir.path(), "s", PrId(2), one(), day(60)).unwrap();
        import_at(dir.path(), "s", PrId(3), one(), day(100)).unwrap();
        let held = read(dir.path(), "s").unwrap();
        assert!(held.for_pr(PrId(1)).is_none(), "a hundred days ago");
        assert!(held.for_pr(PrId(2)).is_some() && held.for_pr(PrId(3)).is_some());
    }

    #[test]
    fn the_file_is_read_again_only_when_an_import_has_replaced_it() {
        let dir = TempDir::new("proposals");
        let mut stamp = Stamp::default();
        assert!(
            read_changed(dir.path(), "s", &mut stamp).unwrap().is_none(),
            "no file is nothing proposed, as at the start"
        );
        import(dir.path(), "s", PrId(7), batch(vec![of("abc123", 1)])).unwrap();
        let read = read_changed(dir.path(), "s", &mut stamp).unwrap();
        assert!(read.is_some_and(|read| read.for_pr(PrId(7)).is_some()));
        assert!(read_changed(dir.path(), "s", &mut stamp).unwrap().is_none());
        import(dir.path(), "s", PrId(7), batch(vec![of("abc123", 2)])).unwrap();
        assert!(read_changed(dir.path(), "s", &mut stamp).unwrap().is_some());
    }

    #[test]
    fn a_file_that_cannot_be_read_is_an_error_and_is_not_written_over() {
        let dir = TempDir::new("proposals");
        import(
            dir.path(),
            "s",
            PrId(7),
            batch(vec![comment("a.rs", 1, "one")]),
        )
        .unwrap();
        let path = path_of(dir.path(), "s");
        std::fs::write(&path, b"{not json").unwrap();
        let error = import(
            dir.path(),
            "s",
            PrId(7),
            batch(vec![comment("a.rs", 2, "two")]),
        )
        .unwrap_err();
        assert!(error.to_string().contains("file preserved"), "{error}");
        assert_eq!(std::fs::read(&path).unwrap(), b"{not json");
        assert!(read(dir.path(), "s").is_err());
    }

    #[test]
    fn two_imports_at_once_both_get_in() {
        let dir = TempDir::new("proposals");
        let root = dir.path().to_path_buf();
        let workers: Vec<_> = (1..=4)
            .map(|n| {
                let root = root.clone();
                std::thread::spawn(move || {
                    import(&root, "s", PrId(7), batch(vec![comment("a.rs", n, "x")])).unwrap();
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        assert_eq!(
            read(&root, "s")
                .unwrap()
                .for_pr(PrId(7))
                .unwrap()
                .comments
                .len(),
            4
        );
    }

    /// The file as version 1 writes it. A change to the types must keep this
    /// text readable and write it back unchanged.
    const VERSION_1: &str = concat!(
        r#"{"version":1,"scope":"repo/a","proposals":{"7":{"comments":[{"head":"abc123","#,
        r#""path":"a.rs","line":3,"side":"old","body":"words","id":"F1","agent":"reviewer"}],"#,
        r#""summaries":[{"head":"abc123","text":"Fine."}]}}}"#
    );

    #[test]
    fn version_1_is_read_and_written_back_unchanged() {
        let envelope: Envelope = serde_json::from_str(VERSION_1).unwrap();
        assert_eq!(
            envelope.proposals.for_pr(PrId(7)).unwrap().comments.len(),
            1
        );
        assert_eq!(serde_json::to_string(&envelope).unwrap(), VERSION_1);
    }
}

//! The file an agent's proposals are kept in: one JSON file per scope, holding
//! what agents have proposed on each PR and nothing the reader has done with
//! it. An agent writes it with `slussa propose import`, which takes the lock
//! for the moment it needs; the TUI reads it without taking the lock, which is
//! safe because a write replaces the whole file at once.
//!
//! What the reader does with a proposal (sends, edits, discards it) is kept
//! elsewhere, by the TUI, so that two processes never write the same file for
//! long.

use std::{collections::BTreeMap, io, path::Path, time::Duration};

use serde::{Deserialize, Serialize};

use super::file::{ScopedFile, path_of};
use crate::domain::{
    pr::PrId,
    proposal::{Proposal, Summary},
};

/// How long an import waits for another one to finish.
const WAIT: Duration = Duration::from_secs(5);

/// How many proposals a PR may hold, so that an agent that loops cannot fill the
/// disk.
pub const MAX_PER_PR: usize = 1_000;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForPr {
    #[serde(default)]
    pub comments: Vec<Proposal>,
    #[serde(default)]
    pub summaries: Vec<Summary>,
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
}

/// What an import did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Imported {
    /// What was added: comments and a summary.
    pub added: usize,
    /// Whether a summary was among them.
    pub summary_added: bool,
    /// Proposals that were already there.
    pub duplicates: usize,
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

/// Add what an agent proposes on `pr` to the file for `scope`. What is there
/// already is kept, and a proposal that is there is not added again. An error
/// leaves the file as it was, including one that cannot be read: it is not
/// written over.
pub fn import(root: &Path, scope: &str, pr: PrId, batch: Batch) -> io::Result<Imported> {
    let (mut file, bytes) = ScopedFile::open_waiting(root, scope, WAIT)?;
    let mut all = match &bytes {
        Some(bytes) => parse(bytes, scope, file.path())?,
        None => Proposals::default(),
    };
    let held = all.0.entry(pr).or_default();
    let mut imported = Imported {
        added: 0,
        summary_added: false,
        duplicates: 0,
    };
    for comment in batch.comments {
        if held.comments.iter().any(|known| known.same_as(&comment)) {
            imported.duplicates += 1;
        } else if held.comments.len() >= MAX_PER_PR {
            return Err(io::Error::other(format!(
                "PR #{pr} already holds {MAX_PER_PR} proposals; the reader has to clear some first"
            )));
        } else {
            held.comments.push(comment);
            imported.added += 1;
        }
    }
    if let Some(summary) = batch.summary {
        if held.summaries.iter().any(|known| known.same_as(&summary)) {
            imported.duplicates += 1;
        } else {
            held.summaries.push(summary);
            imported.added += 1;
            imported.summary_added = true;
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

    fn comment(path: &str, line: usize, body: &str) -> Proposal {
        Proposal::new(ProposalInput {
            head: "abc123".into(),
            path: path.into(),
            line,
            side: Side::New,
            body: body.into(),
            id: None,
            agent: Some("reviewer".into()),
        })
        .unwrap()
    }

    fn batch(comments: Vec<Proposal>) -> Batch {
        Batch {
            comments,
            summary: None,
        }
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
                added: 2,
                summary_added: false,
                duplicates: 0
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
                added: 1,
                summary_added: false,
                duplicates: 1
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
            comments: vec![],
            summary: Some(summary()),
        };
        assert_eq!(
            import(dir.path(), "s", PrId(7), with_summary()).unwrap(),
            Imported {
                added: 1,
                summary_added: true,
                duplicates: 0
            }
        );
        assert_eq!(
            import(dir.path(), "s", PrId(7), with_summary()).unwrap(),
            Imported {
                added: 0,
                summary_added: false,
                duplicates: 1
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

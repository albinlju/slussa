//! The drafts file: editor drafts and queued review comments, one JSON file per
//! scope. A file that cannot be parsed, or that belongs to another scope or
//! version, is left untouched and reported rather than overwritten.

use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::Path,
};

use serde::{Deserialize, Serialize};

use super::file::ScopedFile;
use crate::domain::{
    pr::PrId,
    review::{CommentDraft, PendingReview},
};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub editors: BTreeMap<PrId, CommentDraft>,
    pub reviews: BTreeMap<PrId, PendingReview>,
    pub interrupted: BTreeSet<PrId>,
}
#[derive(Serialize, Deserialize)]
struct Envelope {
    version: u32,
    scope: String,
    snapshot: Snapshot,
}

pub struct DraftStorage {
    file: ScopedFile,
    scope: String,
}

impl DraftStorage {
    pub fn open(root: &Path, scope: String) -> io::Result<(Self, Snapshot)> {
        let (file, bytes) = ScopedFile::open(root, &scope)?;
        let snapshot = match bytes {
            None => Snapshot::default(),
            Some(bytes) => {
                let data: Envelope = serde_json::from_slice(&bytes).map_err(|e| {
                    io::Error::other(format!(
                        "Cannot read drafts at {} (file preserved): {e}",
                        file.path().display()
                    ))
                })?;
                if data.version != 1 || data.scope != scope {
                    return Err(io::Error::other(format!(
                        "Unsupported draft file or repository mismatch at {}",
                        file.path().display()
                    )));
                }
                data.snapshot
            }
        };
        Ok((Self { file, scope }, snapshot))
    }

    pub fn save(&mut self, snapshot: Snapshot) -> io::Result<()> {
        let bytes = serde_json::to_vec(&Envelope {
            version: 1,
            scope: self.scope.clone(),
            snapshot,
        })?;
        self.file.write(bytes)
    }
}

#[cfg(test)]
/// Reopen a scope after dropping its storage.
pub fn reopen(root: &Path, scope: &str) -> io::Result<(DraftStorage, Snapshot)> {
    super::file::reopen_when_released(|| DraftStorage::open(root, scope.into()))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::*;
    use crate::domain::{
        comment::{CommentId, CommentKey, CommentKind},
        diff::DiffRevision,
        review::{CommentAnchor, CommentTarget, PendingComment},
    };
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    fn directory() -> PathBuf {
        std::env::temp_dir().join(format!(
            "slussa-drafts-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ))
    }
    #[test]
    fn restart_retains_targets_revisions_receipts_and_account_isolation() {
        let root = directory();
        let (mut storage, _) = DraftStorage::open(&root, "repo/account-a".into()).unwrap();
        let anchor = CommentAnchor {
            revision: Some(DiffRevision {
                head: "head".into(),
                base: Some("base".into()),
                commit: true,
            }),
            path: "f.rs".into(),
            line: 42,
            removed: true,
        };
        let snapshot = Snapshot {
            editors: [
                (
                    PrId(1),
                    CommentDraft {
                        target: CommentTarget::Reply(CommentId(789)),
                        text: "å\n🦀".into(),
                    },
                ),
                (
                    PrId(2),
                    CommentDraft {
                        target: CommentTarget::Line(anchor.clone()),
                        text: "line".into(),
                    },
                ),
            ]
            .into(),
            reviews: [(
                PrId(2),
                PendingReview {
                    submitted_summary: Some("already sent".into()),
                    comments: vec![PendingComment {
                        anchor,
                        text: "remaining".into(),
                    }],
                },
            )]
            .into(),
            interrupted: [PrId(2)].into(),
        };
        storage.save(snapshot).unwrap();
        assert!(DraftStorage::open(&root, "repo/account-a".into()).is_err());
        let (_, other) = DraftStorage::open(&root, "repo/account-b".into()).unwrap();
        assert!(other.editors.is_empty());
        drop(storage);
        let (mut storage, restored) = reopen(&root, "repo/account-a").unwrap();
        assert_eq!(restored.editors[&PrId(1)].text, "å\n🦀");
        assert!(matches!(
            restored.editors[&PrId(1)].target,
            CommentTarget::Reply(CommentId(789))
        ));
        assert_eq!(
            restored.reviews[&PrId(2)].comments[0]
                .anchor
                .revision
                .as_ref()
                .unwrap()
                .head,
            "head"
        );
        assert_eq!(
            restored.reviews[&PrId(2)].submitted_summary.as_deref(),
            Some("already sent")
        );
        assert!(restored.interrupted.contains(&PrId(2)));
        storage.save(Snapshot::default()).unwrap();
        drop(storage);
        let (storage, cleared) = reopen(&root, "repo/account-a").unwrap();
        assert!(cleared.editors.is_empty());
        drop(storage);
        fs::remove_dir_all(root).unwrap();
    }
    /// The file as version 1 writes it, with every kind of draft target. A
    /// file that no longer parses stops slussa from starting, so a change to
    /// these types must keep this text readable and write it back unchanged.
    const VERSION_1: &str = concat!(
        r#"{"version":1,"scope":"repo/account","snapshot":{"editors":{"#,
        r#""1":{"target":{"Reply":789},"text":"reply"},"#,
        r#""2":{"target":{"Line":{"revision":{"head":"h","base":"b","commit":true},"#,
        r#""path":"f.rs","line":42,"removed":true}},"text":"line"},"#,
        r#""3":{"target":"Pr","text":"pr"},"#,
        r#""4":{"target":{"Edit":{"id":5,"review":true}},"text":"edit"},"#,
        r#""5":{"target":{"Review":{"verdict":"RequestChanges"}},"text":"summary"}},"#,
        r#""reviews":{"2":{"submitted_summary":"sent","comments":[{"anchor":{"revision":null,"#,
        r#""path":"g.rs","line":7,"removed":false},"text":"queued"}]}},"#,
        r#""interrupted":[2]}}"#,
    );

    #[test]
    fn version_1_draft_file_is_read_and_written_back_unchanged() {
        let envelope: Envelope = serde_json::from_str(VERSION_1).unwrap();
        let editors = &envelope.snapshot.editors;
        assert!(matches!(
            editors[&PrId(1)].target,
            CommentTarget::Reply(CommentId(789))
        ));
        assert!(matches!(&editors[&PrId(2)].target, CommentTarget::Line(a) if a.line == 42));
        assert!(matches!(editors[&PrId(3)].target, CommentTarget::Pr));
        assert!(matches!(
            editors[&PrId(4)].target,
            CommentTarget::Edit(CommentKey {
                id: CommentId(5),
                kind: CommentKind::Review
            })
        ));
        assert!(matches!(
            editors[&PrId(5)].target,
            CommentTarget::Review { .. }
        ));
        assert!(
            envelope.snapshot.reviews[&PrId(2)].comments[0]
                .anchor
                .revision
                .is_none()
        );
        assert_eq!(serde_json::to_string(&envelope).unwrap(), VERSION_1);
    }

    #[test]
    fn failed_save_and_corrupt_input_preserve_existing_data() {
        let root = directory();
        let (mut storage, _) = DraftStorage::open(&root, "scope".into()).unwrap();
        storage.save(Snapshot::default()).unwrap();
        let old = fs::read(storage.file.path()).unwrap();
        fs::create_dir(storage.file.path().with_extension("tmp")).unwrap();
        let snapshot = Snapshot {
            interrupted: [PrId(42)].into(),
            ..Snapshot::default()
        };
        assert!(storage.save(snapshot).is_err());
        assert_eq!(fs::read(storage.file.path()).unwrap(), old);
        fs::write(storage.file.path(), b"broken").unwrap();
        let path = storage.file.path().to_path_buf();
        drop(storage);
        assert!(reopen(&root, "scope").is_err());
        assert_eq!(fs::read(path).unwrap(), b"broken");
        fs::remove_dir_all(root).unwrap();
    }
}

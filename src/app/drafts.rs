//! Local recovery data. Never restores executable commands or authentication.
//!
//! Editor drafts and queued review comments are saved to one JSON file per
//! scope (repository, provider and account). Saves are atomic, and a lock lets
//! only one slussa instance write a scope. A file that cannot be parsed, or that
//! belongs to another scope or version, is left untouched and reported rather
//! than overwritten.
use super::{
    App,
    reviews::{CommentDraft, PendingReview},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub editors: BTreeMap<u64, CommentDraft>,
    pub reviews: BTreeMap<u64, PendingReview>,
    pub interrupted: BTreeSet<u64>,
}
#[derive(Serialize, Deserialize)]
struct Envelope {
    version: u32,
    scope: String,
    snapshot: Snapshot,
}
pub struct DraftStorage {
    path: PathBuf,
    scope: String,
    _lock: File,
    previous: Vec<u8>,
}
impl DraftStorage {
    pub fn open(root: &Path, scope: String) -> io::Result<(Self, Snapshot)> {
        let mut directory = fs::DirBuilder::new();
        directory.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            directory.mode(0o700);
        }
        directory.create(root)?;
        // Deterministic filename; the full scope is checked when reading, too.
        let hash = scope.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
        });
        let path = root.join(format!("{hash:016x}.json"));
        let lock = private_options()
            .create(true)
            .truncate(false)
            .open(path.with_extension("lock"))?;
        lock.try_lock().map_err(|e| {
            io::Error::other(format!(
                "Draft storage is already in use or cannot be locked: {e}"
            ))
        })?;
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e),
        };
        let snapshot = if bytes.is_empty() && !path.exists() {
            Snapshot::default()
        } else {
            let data: Envelope = serde_json::from_slice(&bytes).map_err(|e| {
                io::Error::other(format!(
                    "Cannot read drafts at {} (file preserved): {e}",
                    path.display()
                ))
            })?;
            if data.version != 1 || data.scope != scope {
                return Err(io::Error::other(format!(
                    "Unsupported draft file or repository mismatch at {}",
                    path.display()
                )));
            }
            data.snapshot
        };
        Ok((
            Self {
                path,
                scope,
                _lock: lock,
                previous: bytes,
            },
            snapshot,
        ))
    }
    pub fn save(&mut self, snapshot: Snapshot) -> io::Result<()> {
        let bytes = serde_json::to_vec(&Envelope {
            version: 1,
            scope: self.scope.clone(),
            snapshot,
        })?;
        if bytes == self.previous {
            return Ok(());
        }
        let temporary = self.path.with_extension("tmp");
        let mut file = private_options()
            .create(true)
            .truncate(true)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, &self.path)?;
        #[cfg(unix)]
        File::open(
            self.path
                .parent()
                .ok_or_else(|| io::Error::other("Missing draft directory"))?,
        )?
        .sync_all()?;
        self.previous = bytes;
        Ok(())
    }
}
fn private_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}
impl App {
    pub fn enable_drafts(&mut self) -> io::Result<()> {
        if self.state.store.current_user.is_empty() {
            return Err(io::Error::other(
                "Cannot identify the account for draft storage. Check authentication and retry.",
            ));
        }
        let remote = super::remote::origin_url().map_err(io::Error::other)?;
        let (authority, repo) = crate::git_url::split(&remote)
            .ok_or_else(|| io::Error::other("Cannot identify repository for drafts"))?;
        let host = authority.rsplit('@').next().unwrap_or(authority);
        let provider = match self.provider {
            crate::providers::Provider::GitHub => "github",
            crate::providers::Provider::BitbucketDc(_) => "bitbucket-dc",
        };
        let scope = serde_json::to_string(&(
            provider,
            host,
            repo.trim_end_matches(".git"),
            &self.state.store.current_user,
        ))?;
        let root = dirs::data_local_dir()
            .ok_or_else(|| io::Error::other("Cannot locate local data directory"))?
            .join("slussa/drafts");
        let (storage, snapshot) = DraftStorage::open(&root, scope)?;
        self.restore_drafts(storage, snapshot);
        Ok(())
    }
    pub(super) fn restore_drafts(&mut self, storage: DraftStorage, snapshot: Snapshot) {
        self.state.ui.detail.restore_drafts(snapshot.editors);
        self.state.store.reviews = snapshot.reviews.into_iter().collect();
        for &id in &snapshot.interrupted {
            self.state.store.errors.insert(id, "A previous request was interrupted and may have reached the server. Check the PR before sending it again; nothing was resent automatically.".into());
        }
        self.state.store.uncertain_submissions = snapshot.interrupted;
        self.drafts = Some(storage);
    }
    pub(super) fn save_drafts(&mut self) -> bool {
        self.drafts_dirty = false;
        let Some(storage) = &mut self.drafts else {
            return true;
        };
        let snapshot = Snapshot {
            editors: self.state.ui.detail.draft_snapshot(),
            reviews: self
                .state
                .store
                .reviews
                .iter()
                .map(|(id, review)| (*id, review.clone()))
                .collect(),
            interrupted: self
                .state
                .store
                .operations
                .keys()
                .copied()
                .chain(self.state.store.uncertain_submissions.iter().copied())
                .collect(),
        };
        match storage.save(snapshot) {
            Ok(()) => {
                self.state.store.draft_error = None;
                true
            }
            Err(e) => {
                self.state.store.draft_error = Some(format!("Drafts not saved: {e}"));
                false
            }
        }
    }
    /// Journal the uncertain outcome before any remote write can start.
    pub(super) fn checkpoint_submission(&mut self, pr_id: u64) -> bool {
        if self.save_drafts() {
            return true;
        }
        self.state.store.operations.remove(&pr_id);
        self.state.store.errors.insert(
            pr_id,
            "Not sent: draft recovery data could not be saved. Check local storage and try again."
                .into(),
        );
        false
    }
}

#[cfg(test)]
/// Reopen a scope after dropping its storage. A child process started by a
/// concurrent test can briefly hold a copy of the lock file descriptor
/// between fork and exec, so a release is not always visible at once.
pub(super) fn reopen(root: &Path, scope: &str) -> io::Result<(DraftStorage, Snapshot)> {
    let mut attempt = 0;
    loop {
        match DraftStorage::open(root, scope.into()) {
            Err(error) if attempt < 100 && error.to_string().contains("already in use") => {
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            result => return result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::reviews::{CommentAnchor, CommentTarget, PendingComment};
    use crate::domain::diff::DiffRevision;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    pub(super) fn directory() -> PathBuf {
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
                    1,
                    CommentDraft {
                        target: CommentTarget::Reply(789),
                        text: "å\n🦀".into(),
                    },
                ),
                (
                    2,
                    CommentDraft {
                        target: CommentTarget::Line(anchor.clone()),
                        text: "line".into(),
                    },
                ),
            ]
            .into(),
            reviews: [(
                2,
                PendingReview {
                    submitted_summary: Some("already sent".into()),
                    comments: vec![PendingComment {
                        anchor,
                        text: "remaining".into(),
                    }],
                },
            )]
            .into(),
            interrupted: [2].into(),
        };
        storage.save(snapshot).unwrap();
        assert!(DraftStorage::open(&root, "repo/account-a".into()).is_err());
        let (_, other) = DraftStorage::open(&root, "repo/account-b".into()).unwrap();
        assert!(other.editors.is_empty());
        drop(storage);
        let (mut storage, restored) = reopen(&root, "repo/account-a").unwrap();
        assert_eq!(restored.editors[&1].text, "å\n🦀");
        assert!(matches!(
            restored.editors[&1].target,
            CommentTarget::Reply(789)
        ));
        assert_eq!(
            restored.reviews[&2].comments[0]
                .anchor
                .revision
                .as_ref()
                .unwrap()
                .head,
            "head"
        );
        assert_eq!(
            restored.reviews[&2].submitted_summary.as_deref(),
            Some("already sent")
        );
        assert!(restored.interrupted.contains(&2));
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
        assert!(matches!(editors[&1].target, CommentTarget::Reply(789)));
        assert!(matches!(&editors[&2].target, CommentTarget::Line(a) if a.line == 42));
        assert!(matches!(editors[&3].target, CommentTarget::Pr));
        assert!(matches!(
            editors[&4].target,
            CommentTarget::Edit { id: 5, .. }
        ));
        assert!(matches!(editors[&5].target, CommentTarget::Review { .. }));
        assert!(
            envelope.snapshot.reviews[&2].comments[0]
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
        let old = fs::read(&storage.path).unwrap();
        fs::create_dir(storage.path.with_extension("tmp")).unwrap();
        let snapshot = Snapshot {
            interrupted: [42].into(),
            ..Snapshot::default()
        };
        assert!(storage.save(snapshot).is_err());
        assert_eq!(fs::read(&storage.path).unwrap(), old);
        fs::write(&storage.path, b"broken").unwrap();
        let path = storage.path.clone();
        drop(storage);
        assert!(reopen(&root, "scope").is_err());
        assert_eq!(fs::read(path).unwrap(), b"broken");
        fs::remove_dir_all(root).unwrap();
    }
}

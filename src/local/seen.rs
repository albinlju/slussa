//! The file that remembers when each PR was last looked at: one JSON file per
//! scope, holding PR numbers and times and nothing of a PR's content. Like the
//! drafts file it is left untouched when it cannot be read.

use std::{io, path::Path};

use serde::{Deserialize, Serialize};

use super::file::ScopedFile;
use crate::domain::seen::Seen;

#[derive(Serialize, Deserialize)]
struct Envelope {
    version: u32,
    scope: String,
    seen: Seen,
}

pub struct SeenStorage {
    file: ScopedFile,
    scope: String,
}

impl SeenStorage {
    /// The file for `scope` under `root`, and what it holds. An error when it is
    /// in use by another slussa, cannot be read, or belongs to another scope or
    /// version; the caller can go on without remembering.
    pub fn open(root: &Path, scope: String) -> io::Result<(Self, Seen)> {
        let (file, bytes) = ScopedFile::open(root, &scope)?;
        let seen = match bytes {
            None => Seen::default(),
            Some(bytes) => {
                let data: Envelope = serde_json::from_slice(&bytes).map_err(|e| {
                    io::Error::other(format!(
                        "Cannot read {} (file preserved): {e}",
                        file.path().display()
                    ))
                })?;
                if data.version != 1 || data.scope != scope {
                    return Err(io::Error::other(format!(
                        "Unsupported file or repository mismatch at {}",
                        file.path().display()
                    )));
                }
                data.seen
            }
        };
        Ok((Self { file, scope }, seen))
    }

    pub fn save(&mut self, seen: &Seen) -> io::Result<()> {
        let bytes = serde_json::to_vec(&Envelope {
            version: 1,
            scope: self.scope.clone(),
            seen: seen.clone(),
        })?;
        self.file.write(bytes)
    }
}

#[cfg(test)]
/// Open a scope again after dropping its storage.
pub fn reopen(root: &Path, scope: &str) -> io::Result<(SeenStorage, Seen)> {
    super::file::reopen_when_released(|| SeenStorage::open(root, scope.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::pr::PrId;
    use crate::test_support::TempDir;
    use chrono::{DateTime, Utc};
    use std::fs;

    fn at(seconds: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(seconds, 0).unwrap()
    }

    #[test]
    fn what_was_looked_at_survives_a_restart_and_a_scope_of_its_own() {
        let dir = TempDir::new("seen");
        let root = dir.path();
        let (mut storage, none) = SeenStorage::open(root, "repo/a".into()).unwrap();
        assert!(none.is_empty());
        let mut seen = Seen::default();
        seen.mark(PrId(7), at(1_000), at(2_000));
        storage.save(&seen).unwrap();
        drop(storage);

        let (_storage, restored) = reopen(root, "repo/a").unwrap();
        assert_eq!(restored, seen);
        // Another account or repository has its own file.
        let (_other, theirs) = SeenStorage::open(root, "repo/b".into()).unwrap();
        assert!(theirs.is_empty());
    }

    /// The file as version 1 writes it. A change to the types must keep this text
    /// readable and write it back unchanged, or every reader's marks are lost.
    const VERSION_1: &str = concat!(
        r#"{"version":1,"scope":"repo/account","seen":{"#,
        r#""7":{"updated":"2026-10-04T08:00:00Z","at":"2026-10-04T09:00:00Z"}}}"#,
    );

    #[test]
    fn version_1_is_read_and_written_back_unchanged() {
        let envelope: Envelope = serde_json::from_str(VERSION_1).unwrap();
        assert_eq!(envelope.seen.len(), 1);
        assert_eq!(serde_json::to_string(&envelope).unwrap(), VERSION_1);
    }

    #[test]
    fn a_file_that_cannot_be_read_is_an_error_and_is_left_as_it_was() {
        let dir = TempDir::new("seen");
        let root = dir.path();
        let (storage, _) = SeenStorage::open(root, "scope".into()).unwrap();
        let path = storage.file.path().to_path_buf();
        drop(storage);
        fs::write(&path, b"broken").unwrap();
        assert!(reopen(root, "scope").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"broken");
    }

    #[test]
    fn a_second_reader_of_the_same_file_is_refused_not_let_to_overwrite_it() {
        let dir = TempDir::new("seen");
        let root = dir.path();
        let (_first, _) = SeenStorage::open(root, "scope".into()).unwrap();
        let second = SeenStorage::open(root, "scope".into());
        assert!(second.is_err_and(|e| e.to_string().contains("in use")));
    }
}

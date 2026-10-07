//! One file per scope: its name, a lock so that only one slussa instance writes
//! it, and an atomic write. What is in the file is for the caller to say.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

pub struct ScopedFile {
    path: PathBuf,
    _lock: File,
    previous: Vec<u8>,
}

impl ScopedFile {
    /// The file for `scope` under `root`, locked, and what it held: `None` when
    /// there is no file yet, and an existing empty file is `Some` of nothing.
    pub fn open(root: &Path, scope: &str) -> io::Result<(Self, Option<Vec<u8>>)> {
        let mut directory = fs::DirBuilder::new();
        directory.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            directory.mode(0o700);
        }
        directory.create(root)?;
        let path = path_of(root, scope);
        let lock = private_options()
            .create(true)
            .truncate(false)
            .open(path.with_extension("lock"))?;
        lock.try_lock().map_err(|e| match e {
            fs::TryLockError::WouldBlock => io::Error::new(
                io::ErrorKind::WouldBlock,
                "Draft storage is already in use or cannot be locked",
            ),
            fs::TryLockError::Error(e) => io::Error::other(format!(
                "Draft storage is already in use or cannot be locked: {e}"
            )),
        })?;
        let bytes = match fs::read(&path) {
            Ok(bytes) => Some(bytes),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(e),
        };
        let previous = bytes.clone().unwrap_or_default();
        Ok((
            Self {
                path,
                _lock: lock,
                previous,
            },
            bytes,
        ))
    }

    /// As `open`, for a file that is held only for a moment by a short job, such
    /// as an import: it waits up to `wait` for the one that has it.
    pub fn open_waiting(
        root: &Path,
        scope: &str,
        wait: std::time::Duration,
    ) -> io::Result<(Self, Option<Vec<u8>>)> {
        let started = std::time::Instant::now();
        loop {
            match Self::open(root, scope) {
                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock && started.elapsed() < wait =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                result => return result,
            }
        }
    }

    /// Replace the file's content, all or nothing. Nothing is written when it
    /// is what the file already holds.
    pub fn write(&mut self, bytes: Vec<u8>) -> io::Result<()> {
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

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Which file a reader without the lock read last, so that it reads again only
/// once a write has replaced it. At first it is of no file.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Stamp(Option<Mark>);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Mark {
    modified: std::time::SystemTime,
    len: u64,
    /// A write replaces the file, so the one after it is another file.
    #[cfg(unix)]
    inode: u64,
}

impl Stamp {
    /// The file for `scope` under `root` as it is now.
    pub fn of(root: &Path, scope: &str) -> io::Result<Self> {
        let file = match fs::metadata(path_of(root, scope)) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self(None)),
            Err(error) => return Err(error),
        };
        Ok(Self(Some(Mark {
            modified: file.modified()?,
            len: file.len(),
            #[cfg(unix)]
            inode: std::os::unix::fs::MetadataExt::ino(&file),
        })))
    }
}

/// Where the file for `scope` is: a name made from the scope, so the caller
/// checks the full scope when reading.
pub fn path_of(root: &Path, scope: &str) -> PathBuf {
    let hash = scope.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    });
    root.join(format!("{hash:016x}.json"))
}

/// Open again after the earlier handle was dropped. A child process started by
/// a concurrent test can briefly hold a copy of the lock file descriptor between
/// fork and exec, so a release is not always visible at once; this waits for it.
#[cfg(test)]
pub fn reopen_when_released<T>(open: impl Fn() -> io::Result<T>) -> io::Result<T> {
    let mut attempt = 0;
    loop {
        match open() {
            Err(error) if attempt < 100 && error.to_string().contains("already in use") => {
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            result => return result,
        }
    }
}

fn private_options() -> OpenOptions {
    let mut options = crate::private_file::options();
    options.read(true).write(true);
    options
}

//! The log file. It holds states, resource keys, counts and error text, the
//! server's included; never the content of a PR and never a token.
use std::fs::{File, create_dir_all};
use std::path::{Path, PathBuf};

use tracing_subscriber::{EnvFilter, fmt};

use crate::private_file;

pub fn init() -> std::io::Result<()> {
    let path = log_path();
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    let file = open(&path)?;

    let filter = EnvFilter::try_from_env("SLUSSA_LOG").unwrap_or_else(|_| EnvFilter::new("warn"));

    fmt()
        .with_env_filter(filter)
        .with_writer(file)
        .with_ansi(false)
        .with_target(true)
        .init();

    Ok(())
}

/// The log, emptied, for its owner only: it names hosts and repositories.
fn open(path: &Path) -> std::io::Result<File> {
    let file = private_file::options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    private_file::restrict(&file)?;
    Ok(file)
}

pub fn log_path() -> PathBuf {
    dirs::data_dir()
        .map_or_else(|| PathBuf::from("."), |d| d.join("slussa"))
        .join("slussa.log")
}

#[cfg(all(test, unix))]
mod tests {
    use super::open;
    use crate::test_support::TempDir;
    use std::{fs, os::unix::fs::PermissionsExt, path::Path};

    #[test]
    fn the_log_is_emptied_and_readable_by_its_owner_only() {
        let dir = TempDir::new("log");
        let path = dir.path().join("slussa.log");
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;

        drop(open(&path).unwrap());
        assert_eq!(mode(&path), 0o600, "a new log");

        // One left by an earlier version, readable by everyone and not empty.
        fs::write(&path, "an earlier run").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        drop(open(&path).unwrap());
        assert_eq!(mode(&path), 0o600, "an existing log");
        assert_eq!(fs::read_to_string(&path).unwrap(), "");
    }
}

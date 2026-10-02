//! Files only their owner may read: the drafts and the log. Both name hosts and
//! repositories, and the drafts hold unsent comments.
use std::{
    fs::{File, OpenOptions},
    io,
};

/// Options that create a file with mode 0600 on Unix. The caller says how it
/// is opened.
pub fn options() -> OpenOptions {
    let mut options = OpenOptions::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}

/// Takes the file away from group and others. `options` sets the mode only
/// when it creates the file, so one that an earlier version left readable
/// needs this.
pub fn restrict(file: &File) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = file;
    Ok(())
}

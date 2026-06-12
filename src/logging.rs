use std::fs::{File, create_dir_all};
use std::path::PathBuf;

use tracing_subscriber::{EnvFilter, fmt};

pub fn init() -> std::io::Result<()> {
    let path = log_path();
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    let file = File::create(&path)?;

    let filter = EnvFilter::try_from_env("TUIPR_LOG").unwrap_or_else(|_| EnvFilter::new("warn"));

    fmt()
        .with_env_filter(filter)
        .with_writer(file)
        .with_ansi(false)
        .with_target(true)
        .init();

    Ok(())
}

pub fn log_path() -> PathBuf {
    dirs::data_dir()
        .map(|d| d.join("tuipr"))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tuipr.log")
}

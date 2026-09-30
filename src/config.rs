use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    pub theme: Option<String>,
    /// `"attention"` (default) or `"recent"`: how the PR list is ordered.
    pub sort: Option<String>,
}

fn config_path() -> Option<PathBuf> {
    config_root().map(|d| d.join("tuipr").join("config.toml"))
}

fn config_root() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(xdg));
    }
    #[cfg(target_os = "macos")]
    let root = dirs::home_dir().map(|h| h.join(".config"));
    #[cfg(not(target_os = "macos"))]
    let root = dirs::config_dir();
    root
}

pub fn load() -> Config {
    let Some(path) = config_path() else {
        return Config::default();
    };
    let Ok(text) = fs::read_to_string(&path) else {
        return Config::default();
    };
    toml::from_str(&text).unwrap_or_else(|e| {
        tracing::warn!("ignoring invalid config {}: {e}", path.display());
        Config::default()
    })
}

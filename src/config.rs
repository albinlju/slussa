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
    config_root().map(|d| d.join("slussa").join("config.toml"))
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
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) => {
            // No file is the normal case; one that exists and cannot be read is not.
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!("ignoring unreadable config {}: {e}", path.display());
            }
            return Config::default();
        }
    };
    toml::from_str(&text).unwrap_or_else(|e| {
        tracing::warn!("ignoring invalid config {}: {e}", path.display());
        Config::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Config, toml::de::Error> {
        toml::from_str(text)
    }

    #[test]
    fn reads_theme_and_sort() {
        let config = parse("theme = \"slate\"\nsort = \"recent\"\n").unwrap();
        assert_eq!(config.theme.as_deref(), Some("slate"));
        assert_eq!(config.sort.as_deref(), Some("recent"));
    }

    #[test]
    fn accepts_unspaced_assignments_and_comments() {
        let config = parse("# my setup\ntheme=\"catppuccin\"  # mocha\n").unwrap();
        assert_eq!(config.theme.as_deref(), Some("catppuccin"));
        assert_eq!(config.sort, None);
    }

    #[test]
    fn an_empty_file_and_unknown_keys_are_fine() {
        assert_eq!(parse("").unwrap().theme, None);
        let config = parse("future_option = true\n[section]\nx = 1\ntheme = \"terminal\"\n");
        // `theme` after a table header belongs to the table, not the top level.
        assert_eq!(config.unwrap().theme, None);
        assert_eq!(
            parse("theme = \"terminal\"\nfuture_option = true\n")
                .unwrap()
                .theme
                .as_deref(),
            Some("terminal")
        );
    }

    #[test]
    fn invalid_toml_and_wrong_types_are_errors() {
        assert!(parse("theme = ").is_err());
        assert!(parse("theme = 3\n").is_err());
        assert!(parse("sort = [\"recent\"]\n").is_err());
    }

    #[test]
    fn platform_directories_are_absolute() {
        for dir in [dirs::home_dir(), dirs::config_dir()] {
            assert!(dir.is_some_and(|path| path.is_absolute()));
        }
    }
}

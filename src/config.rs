use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    pub theme: Option<String>,
    /// `"attention"` (default) or `"recent"`: how the PR list is ordered.
    pub sort: Option<String>,
    #[serde(default)]
    pub ai: AiConfig,
    /// The command that reviews a PR when asked to (`A`): a program and its
    /// arguments, given the PR's title, description and diff on standard input.
    /// An empty list turns the key off.
    pub agent_review: Option<Vec<String>>,
    /// A file with what the reader wants an asked-for review to be, instead of
    /// the built-in instructions. What the agent is given about the PR and the
    /// form of its answer stay slussa's.
    pub agent_review_instructions: Option<String>,
}

/// A path from the config, with a leading `~/` meaning the home directory.
pub fn expand_home(path: &str) -> PathBuf {
    match path
        .strip_prefix("~/")
        .and_then(|rest| dirs::home_dir().map(|home| home.join(rest)))
    {
        Some(expanded) => expanded,
        None => PathBuf::from(path),
    }
}

/// What reviews a PR when the config does not say.
pub fn default_agent_review() -> Vec<String> {
    vec!["claude".to_owned(), "-p".to_owned()]
}

/// The `[ai]` table: how comments by an AI agent are recognised.
#[derive(Debug, Default, Deserialize)]
pub struct AiConfig {
    /// First lines that agents start their comments with, such as
    /// `> **gator-agent**`.
    #[serde(default)]
    pub markers: Vec<String>,
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
    fn reads_the_ai_markers_and_defaults_to_none() {
        let config = parse("[ai]\nmarkers = [\"> **gator-agent**\", \"> **build**\"]\n").unwrap();
        assert_eq!(config.ai.markers, ["> **gator-agent**", "> **build**"]);
        assert!(parse("theme = \"slate\"\n").unwrap().ai.markers.is_empty());
        assert!(parse("[ai]\n").unwrap().ai.markers.is_empty());
        assert!(parse("[ai]\nmarkers = \"one\"\n").is_err());
    }

    #[test]
    fn reads_the_agent_review_command_and_it_is_a_list_not_a_line() {
        let config = parse("agent_review = [\"claude\", \"-p\", \"--model\", \"x\"]\n").unwrap();
        assert_eq!(
            config.agent_review.unwrap(),
            ["claude", "-p", "--model", "x"]
        );
        assert_eq!(parse("theme = \"slate\"\n").unwrap().agent_review, None);
        assert_eq!(
            parse("agent_review = []\n").unwrap().agent_review,
            Some(vec![])
        );
        // One string would need a shell to split; it is refused, not guessed at.
        assert!(parse("agent_review = \"claude -p\"\n").is_err());
        assert_eq!(default_agent_review(), ["claude", "-p"]);
    }

    #[test]
    fn reads_the_instructions_file_and_expands_the_home_directory() {
        let config = parse("agent_review_instructions = \"~/review.md\"\n").unwrap();
        assert_eq!(
            config.agent_review_instructions.as_deref(),
            Some("~/review.md")
        );
        assert!(parse("").unwrap().agent_review_instructions.is_none());
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_home("~/review.md"), home.join("review.md"));
        assert_eq!(
            expand_home("/etc/review.md"),
            PathBuf::from("/etc/review.md")
        );
        assert_eq!(expand_home("review.md"), PathBuf::from("review.md"));
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

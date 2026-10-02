//! Test-only: what the documents promise, for the tests that hold the code to
//! them. `docs/KEYS.md` and the README's config example are written by hand,
//! next to tables in the code that are written by hand too. A test beside each
//! table compares the two, so a key, a theme or a sort added on one side fails
//! until the other follows.
use std::collections::BTreeSet;

const KEYS_MD: &str = include_str!("../docs/KEYS.md");
const README: &str = include_str!("../README.md");

/// Fails unless a help table and `docs/KEYS.md` name the same keys for the view
/// under `## heading`, and says which side lacks which key.
pub fn assert_keys_match(table: &[(&str, &str)], heading: &str) {
    let help = table_keys(table);
    let page = documented_keys(heading);
    let only_in_help: Vec<_> = help.difference(&page).collect();
    let only_on_page: Vec<_> = page.difference(&help).collect();
    assert!(
        only_in_help.is_empty() && only_on_page.is_empty(),
        "the help and docs/KEYS.md (`## {heading}`) name different keys: \
         only in the help {only_in_help:?}, only on the page {only_on_page:?}"
    );
}

/// The keys `docs/KEYS.md` names for the view under `## heading`: every
/// backticked word in the rows of its table, and in the lines before the first
/// heading, which hold for every view (`?`).
fn documented_keys(heading: &str) -> BTreeSet<String> {
    let intro = KEYS_MD.split("\n## ").next().unwrap_or_default();
    let rows = section(KEYS_MD, heading)
        .lines()
        .filter(|line| line.starts_with('|'));
    let mut keys: BTreeSet<String> = intro
        .lines()
        .chain(rows.clone())
        .flat_map(backticked)
        .collect();
    // The one key the page names in words.
    if rows.clone().any(|row| row.contains("arrows")) {
        keys.insert(ARROWS.to_owned());
    }
    keys
}

/// The keys a help table names, one entry per key, spelled the way
/// `documented_keys` reads them.
fn table_keys(table: &[(&str, &str)]) -> BTreeSet<String> {
    table
        .iter()
        .flat_map(|(keys, _)| split_keys(keys))
        .collect()
}

/// The values the README's config example lists for `key`, in the order
/// written: `theme = "graphite"   # graphite (default), slate, ...`.
pub fn readme_values(key: &str) -> Vec<String> {
    readme_choices(key)
        .into_iter()
        .map(|choice| choice.trim_end_matches(DEFAULT_MARK).trim().to_owned())
        .collect()
}

/// The value the README marks `(default)` for `key`. Exactly one has to be:
/// two marks, or none, is a README that does not say what the default is.
pub fn readme_default(key: &str) -> String {
    let choices = readme_choices(key);
    match defaults(&choices).as_slice() {
        [only] => (*only).to_owned(),
        marked => panic!(
            "the README marks {} values `(default)` for `{key}`; it has to be one",
            marked.len()
        ),
    }
}

/// The choices that carry the `(default)` mark, without it.
fn defaults(choices: &[String]) -> Vec<&str> {
    choices
        .iter()
        .filter_map(|choice| choice.strip_suffix(DEFAULT_MARK).map(str::trim))
        .collect()
}

const ARROWS: &str = "arrows";
const DEFAULT_MARK: &str = "(default)";

/// The text under `## heading`, up to the next heading of that level.
fn section<'a>(page: &'a str, heading: &str) -> &'a str {
    let (_, body) = page
        .split_once(&format!("## {heading}\n"))
        .unwrap_or_else(|| panic!("docs/KEYS.md has no `## {heading}`"));
    body.split("\n## ").next().unwrap_or(body)
}

/// The backticked words in a line. A backticked phrase (`[ai] markers`) is not
/// a key and is left out.
fn backticked(line: &str) -> impl Iterator<Item = String> + '_ {
    line.split('`')
        .skip(1)
        .step_by(2)
        .filter(|word| !word.is_empty() && !word.contains(char::is_whitespace))
        .map(str::to_owned)
}

/// One help cell into its keys: `j/k / ↑↓`, `^d/^u`, `1-5`, `[ ]`, `? / esc`.
fn split_keys(cell: &str) -> Vec<String> {
    if cell == "/" {
        return vec![cell.to_owned()];
    }
    cell.split_whitespace()
        .filter(|word| *word != "/")
        .flat_map(|word| {
            if word == "↑↓" {
                vec![ARROWS]
            } else if let Some((from, to)) = digit_range(word) {
                vec![from, to]
            } else {
                word.split('/').collect()
            }
        })
        .map(str::to_owned)
        .collect()
}

/// `1-5` as its two ends, the way the page writes `1`-`5`.
fn digit_range(word: &str) -> Option<(&str, &str)> {
    let (from, to) = word.split_once('-')?;
    let digit = |end: &str| end.len() == 1 && end.chars().all(|c| c.is_ascii_digit());
    (digit(from) && digit(to)).then_some((from, to))
}

/// What follows `#` on the README line that sets `key`, split at commas and
/// at `or`, each choice still carrying its `(default)` mark.
fn readme_choices(key: &str) -> Vec<String> {
    let assignment = format!("{key} = ");
    let line = README
        .lines()
        .find(|line| line.starts_with(&assignment))
        .unwrap_or_else(|| panic!("the README's config example does not set `{key}`"));
    let (_, comment) = line
        .split_once('#')
        .unwrap_or_else(|| panic!("the README lists no values for `{key}`"));
    comment
        .replace(" or ", ",")
        .split(',')
        .map(|choice| choice.trim().to_owned())
        .filter(|choice| !choice.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(cells: &[&str]) -> Vec<String> {
        cells.iter().flat_map(|cell| split_keys(cell)).collect()
    }

    #[test]
    fn a_help_cell_is_split_into_single_keys() {
        assert_eq!(keys(&["j/k / ↑↓"]), ["j", "k", "arrows"]);
        assert_eq!(keys(&["^d/^u", "n/N"]), ["^d", "^u", "n", "N"]);
        assert_eq!(keys(&["1-5", "[ ]"]), ["1", "5", "[", "]"]);
        assert_eq!(keys(&["? / esc", "/", "enter"]), ["?", "esc", "/", "enter"]);
    }

    #[test]
    fn a_row_gives_its_backticked_keys_and_not_its_phrases() {
        let row = "| `f` | all, people's or an agent's; an `[ai] markers` line; `j`/`k` stop |";
        assert_eq!(backticked(row).collect::<Vec<_>>(), ["f", "j", "k"]);
    }

    #[test]
    fn both_views_are_on_the_page_and_every_view_has_the_help_key() {
        for heading in ["The list", "A PR"] {
            let keys = documented_keys(heading);
            assert!(keys.contains("?"), "{heading}: {keys:?}");
            assert!(keys.contains("q"), "{heading}: {keys:?}");
        }
        assert!(documented_keys("The list").contains(ARROWS));
        assert!(!documented_keys("The list").contains("R"), "a PR key");
    }

    #[test]
    fn the_readme_values_come_without_their_default_mark() {
        assert_eq!(readme_values("sort"), ["attention", "recent"]);
        assert_eq!(readme_default("sort"), "attention");
    }

    #[test]
    fn every_marked_default_is_found_so_two_or_none_can_be_refused() {
        let choices =
            |written: &[&str]| written.iter().map(|c| (*c).to_owned()).collect::<Vec<_>>();
        let one = choices(&["graphite (default)", "slate"]);
        assert_eq!(defaults(&one), ["graphite"]);
        let two = choices(&["graphite (default)", "slate (default)"]);
        assert_eq!(defaults(&two), ["graphite", "slate"]);
        assert!(defaults(&choices(&["graphite", "slate"])).is_empty());
    }
}

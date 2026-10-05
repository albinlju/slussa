//! Code as it can be read. A terminal draws nothing for a control character, a
//! zero-width one or a mark that reorders text, and ratatui drops them, so a
//! line can read one way and mean another, and a tab leaves no indentation.
//! Here a tab is spaces to the next tab stop, and what would be invisible is
//! drawn as a marker (`‹U+202E›`) in the warning colour.

use ratatui::{style::Style, text::Span};

/// Where a tab ends: every this many columns, counted from the start of the line.
const TAB_STOP: usize = 4;

/// A piece of a line, and whether it is a marker for what is not drawn.
#[derive(Debug, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub marker: bool,
}

/// Whether `c` is drawn as nothing, or as something other than what it does to
/// the text around it. `before` and `after` are its neighbours: a joiner has an
/// effect only between two characters of one kind that it joins, as in an emoji
/// sequence or a word of a script that uses it, and is invisible anywhere else,
/// as in an identifier or between two quotation marks.
fn is_invisible(c: char, before: Option<char>, after: Option<char>) -> bool {
    match c {
        '\t' => false,
        // C0, DEL and C1 controls, which a terminal would act on.
        _ if c.is_control() => true,
        // Marks and embeddings that reorder the text around them ("Trojan
        // Source"); zero width: space, word joiner and the invisible
        // operators, a byte order mark, a soft hyphen, line and paragraph
        // separators; tag characters and the supplementary variation
        // selectors, which have no glyph and are a way to hide text in text.
        '\u{061C}'
        | '\u{200E}'
        | '\u{200F}'
        | '\u{202A}'..='\u{202E}'
        | '\u{2066}'..='\u{2069}'
        | '\u{200B}'
        | '\u{2060}'..='\u{2064}'
        | '\u{FEFF}'
        | '\u{00AD}'
        | '\u{2028}'
        | '\u{2029}'
        | '\u{E0000}'..='\u{E007F}'
        | '\u{E0100}'..='\u{E01EF}' => true,
        '\u{200C}' | '\u{200D}' => !matches!(
            (before.and_then(joins), after.and_then(joins)),
            (Some(a), Some(b)) if a == b
        ),
        _ => false,
    }
}

/// What kind of text a joiner may join, if `c` is of one.
#[derive(PartialEq, Eq, Clone, Copy)]
enum Joins {
    /// Emoji and the symbols that go into emoji sequences, with their selector.
    Emoji,
    /// A script whose words are joined or kept apart by joiners.
    Script,
}

const fn joins(c: char) -> Option<Joins> {
    match c {
        '\u{1F000}'..='\u{1FAFF}'
        | '\u{2300}'..='\u{23FF}'
        | '\u{2600}'..='\u{27BF}'
        | '\u{2B00}'..='\u{2BFF}'
        | '\u{FE0F}' => Some(Joins::Emoji),
        // Arabic, Syriac, N'Ko, Indic scripts, Myanmar, Khmer, Mongolian, Adlam
        // and the Arabic presentation forms (not the byte order mark).
        '\u{0600}'..='\u{07FF}'
        | '\u{08A0}'..='\u{08FF}'
        | '\u{0900}'..='\u{0DFF}'
        | '\u{1000}'..='\u{109F}'
        | '\u{1780}'..='\u{17FF}'
        | '\u{1800}'..='\u{18AF}'
        | '\u{1E900}'..='\u{1E95F}'
        | '\u{FB50}'..='\u{FDFF}'
        | '\u{FE70}'..='\u{FEFE}' => Some(Joins::Script),
        _ => None,
    }
}

fn marker(c: char) -> String {
    format!("‹U+{:04X}›", u32::from(c))
}

fn width(text: &str) -> usize {
    Span::raw(text).width()
}

/// The runs of a line so far, and the column the next one starts at.
#[derive(Default)]
struct Drawn {
    runs: Vec<Run>,
    column: usize,
}

impl Drawn {
    fn push(&mut self, piece: String, marker: bool) {
        self.column += width(&piece);
        match self.runs.last_mut() {
            Some(last) if last.marker == marker => last.text.push_str(&piece),
            _ => self.runs.push(Run {
                text: piece,
                marker,
            }),
        }
    }
}

/// `text` as it is to be drawn: tabs spread to tab stops, and each character
/// that would be invisible replaced by a marker. A carriage return that ends the
/// line is the line ending of a file that has them, not content, and is left out.
pub fn reveal(text: &str) -> Vec<Run> {
    let text = text.strip_suffix('\r').unwrap_or(text);
    let chars: Vec<char> = text.chars().collect();
    let mut drawn = Drawn::default();
    for (i, &c) in chars.iter().enumerate() {
        let before = i.checked_sub(1).and_then(|j| chars.get(j)).copied();
        let after = chars.get(i + 1).copied();
        if c == '\t' {
            drawn.push(" ".repeat(TAB_STOP - drawn.column % TAB_STOP), false);
        } else if is_invisible(c, before, after) {
            drawn.push(marker(c), true);
        } else {
            drawn.push(c.to_string(), false);
        }
    }
    drawn.runs
}

/// How many columns the runs take.
pub fn runs_width(runs: &[Run]) -> usize {
    runs.iter().map(|run| width(&run.text)).sum()
}

/// The runs as spans: what is text in `text`, what is a marker in `marker`.
pub fn spans(runs: Vec<Run>, text: Style, marker: Style) -> Vec<Span<'static>> {
    runs.into_iter()
        .map(|run| Span::styled(run.text, if run.marker { marker } else { text }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drawn(text: &str) -> String {
        reveal(text).into_iter().map(|run| run.text).collect()
    }

    #[test]
    fn a_tab_is_spaces_to_the_next_tab_stop() {
        assert_eq!(drawn("\tx"), "    x");
        assert_eq!(drawn("a\tb"), "a   b");
        assert_eq!(drawn("abcd\te"), "abcd    e");
        assert_eq!(drawn("\t\tx"), "        x");
    }

    #[test]
    fn what_a_terminal_would_not_draw_is_a_marker() {
        for (c, name) in [
            ('\u{202E}', "‹U+202E›"),
            ('\u{2066}', "‹U+2066›"),
            ('\u{200B}', "‹U+200B›"),
            ('\u{FEFF}', "‹U+FEFF›"),
            ('\u{1B}', "‹U+001B›"),
            ('\u{85}', "‹U+0085›"),
            ('\u{E0041}', "‹U+E0041›"),
        ] {
            assert_eq!(drawn(&format!("a{c}b")), format!("a{name}b"), "{c:?}");
        }
    }

    #[test]
    fn a_marker_is_told_apart_from_the_text_around_it() {
        let runs = reveal("if x\u{202E} {");
        assert_eq!(
            runs,
            [
                Run {
                    text: "if x".into(),
                    marker: false
                },
                Run {
                    text: "‹U+202E›".into(),
                    marker: true
                },
                Run {
                    text: " {".into(),
                    marker: false
                },
            ]
        );
    }

    #[test]
    fn a_joiner_is_text_between_letters_of_a_script_and_a_marker_against_ascii() {
        // An emoji with a joiner, and a Persian word with a non-joiner.
        assert_eq!(drawn("👩\u{200D}💻"), "👩\u{200D}💻");
        assert_eq!(drawn("می\u{200C}خواهم"), "می\u{200C}خواهم");
        // An emoji with a selector before the joiner, and a Devanagari conjunct.
        assert_eq!(drawn("❤\u{FE0F}\u{200D}🔥"), "❤\u{FE0F}\u{200D}🔥");
        assert_eq!(drawn("क\u{200D}ष"), "क\u{200D}ष");
        // The same characters inside an identifier.
        assert_eq!(drawn("is\u{200D}Admin"), "is‹U+200D›Admin");
        assert_eq!(drawn("a\u{200C}b"), "a‹U+200C›b");
        // Where it joins nothing: between marks that are not of a joining kind,
        // between two kinds, or at the edge of the text.
        assert_eq!(drawn("«\u{200D}»"), "«‹U+200D›»");
        assert_eq!(drawn("é\u{200C}è"), "é‹U+200C›è");
        assert_eq!(drawn("👩\u{200D}ب"), "👩‹U+200D›ب");
        assert_eq!(drawn("\u{200D}"), "‹U+200D›");
        assert_eq!(drawn("🙂\u{200D}"), "🙂‹U+200D›");
    }

    #[test]
    fn a_line_ending_is_not_content_and_ordinary_text_is_untouched() {
        assert_eq!(drawn("let x = 1;\r"), "let x = 1;");
        assert_eq!(
            drawn("a\rb"),
            "a‹U+000D›b",
            "a carriage return inside a line is"
        );
        assert_eq!(
            drawn("fn main() { println!(\"åäö 🦀\") }"),
            "fn main() { println!(\"åäö 🦀\") }"
        );
        assert!(reveal("").is_empty());
    }

    #[test]
    fn the_width_of_the_runs_counts_the_markers_and_the_spread_tabs() {
        assert_eq!(runs_width(&reveal("\tab")), 6);
        assert_eq!(
            runs_width(&reveal("a\u{202E}")),
            1 + "‹U+202E›".chars().count()
        );
    }
}

//! The log of one build, read from the provider as text and kept as lines that
//! say what they are, so that the screen can find the errors without parsing
//! it again.

/// How many lines of a log are kept. A log is read whole and can be tens of
/// megabytes; the end is where a failure is, so it is the end that stays.
const MAX_LINES: usize = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogKind {
    Plain,
    /// The start of a step.
    Group,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    pub kind: LogKind,
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildLog {
    pub lines: Vec<LogLine>,
    /// How many lines came before the ones kept.
    pub omitted: usize,
    /// The positions in `lines` of the error lines, in order.
    pub errors: Vec<usize>,
}

impl BuildLog {
    /// Read the text of a GitHub Actions log: each line starts with a
    /// timestamp, and `##[group]`, `##[warning]` and `##[error]` mark steps and
    /// problems.
    pub fn parse(text: &str) -> Self {
        let all: Vec<&str> = text.lines().collect();
        let omitted = all.len().saturating_sub(MAX_LINES);
        let lines: Vec<LogLine> = all.iter().skip(omitted).filter_map(|l| line(l)).collect();
        let errors = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.kind == LogKind::Error)
            .map(|(at, _)| at)
            .collect();
        Self {
            lines,
            omitted,
            errors,
        }
    }

    /// The position of the first error at or after `from`, or when there is
    /// none, the first one: stepping past the last error comes round.
    pub fn next_error(&self, from: usize) -> Option<usize> {
        self.errors
            .iter()
            .copied()
            .find(|at| *at >= from)
            .or_else(|| self.errors.first().copied())
    }

    /// The position of the last error before `from`, or when there is none, the
    /// last one.
    pub fn previous_error(&self, from: usize) -> Option<usize> {
        self.errors
            .iter()
            .copied()
            .rev()
            .find(|at| *at < from)
            .or_else(|| self.errors.last().copied())
    }
}

fn line(raw: &str) -> Option<LogLine> {
    let text = printable(without_timestamp(raw.trim_start_matches('\u{feff}')));
    let (kind, text) = if let Some(rest) = text.strip_prefix("##[group]") {
        (LogKind::Group, rest)
    } else if let Some(rest) = text.strip_prefix("##[error]") {
        (LogKind::Error, rest)
    } else if let Some(rest) = text.strip_prefix("##[warning]") {
        (LogKind::Warning, rest)
    } else if text.starts_with("##[endgroup]") || text.starts_with("##[debug]") {
        return None;
    } else {
        (LogKind::Plain, text.as_str())
    };
    Some(LogLine {
        kind,
        text: text.to_owned(),
    })
}

/// `2024-05-01T10:00:00.1234567Z the line` is `the line`.
fn without_timestamp(line: &str) -> &str {
    let Some((stamp, rest)) = line.split_once(' ') else {
        return line;
    };
    let is_stamp = stamp.ends_with('Z')
        && stamp.len() >= 20
        && stamp
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '-' | ':' | '.' | 'T' | 'Z'));
    if is_stamp { rest } else { line }
}

/// The text without terminal escape sequences and control characters, which a
/// log carries from the tools that ran and which must never reach the terminal.
fn printable(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\u{1b}' => {
                if chars.next_if_eq(&'[').is_some() {
                    // A control sequence ends at its first letter.
                    for next in chars.by_ref() {
                        if next.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
            }
            '\t' => out.push_str("    "),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "\u{feff}2024-05-01T10:00:00.1234567Z ##[group]Run cargo test\n\
2024-05-01T10:00:01.0000000Z running 2 tests\n\
2024-05-01T10:00:02.0000000Z ##[warning]unused variable\n\
2024-05-01T10:00:03.0000000Z \u{1b}[31mFAILED\u{1b}[0m\ttests::it\n\
2024-05-01T10:00:04.0000000Z ##[error]Process completed with exit code 101.\n\
2024-05-01T10:00:05.0000000Z ##[endgroup]\n\
2024-05-01T10:00:06.0000000Z ##[debug]noise\n\
2024-05-01T10:00:07.0000000Z ##[error]second\n";

    fn kinds(log: &BuildLog) -> Vec<LogKind> {
        log.lines.iter().map(|line| line.kind).collect()
    }

    #[test]
    fn lines_lose_their_timestamp_and_marker_and_say_what_they_are() {
        let log = BuildLog::parse(LOG);
        assert_eq!(
            kinds(&log),
            [
                LogKind::Group,
                LogKind::Plain,
                LogKind::Warning,
                LogKind::Plain,
                LogKind::Error,
                LogKind::Error
            ]
        );
        assert_eq!(log.lines[0].text, "Run cargo test");
        assert_eq!(log.errors, [4, 5]);
        assert_eq!(log.omitted, 0);
    }

    #[test]
    fn escape_sequences_and_control_characters_do_not_reach_the_screen() {
        let log = BuildLog::parse(LOG);
        assert_eq!(log.lines[3].text, "FAILED    tests::it");
        assert_eq!(printable("a\u{7}b\u{1b}[2Jc\u{1b}"), "abc");
    }

    #[test]
    fn a_marker_behind_a_colour_code_is_still_a_marker() {
        let log = BuildLog::parse("\u{1b}[31;1m##[error]\u{1b}[0mred");
        assert_eq!(log.errors, [0]);
        assert_eq!(log.lines[0].text, "red");
    }

    #[test]
    fn a_line_that_only_looks_like_a_timestamp_is_kept_whole() {
        let log = BuildLog::parse("2024-05-01 not a stamp\nZ x");
        assert_eq!(log.lines[0].text, "2024-05-01 not a stamp");
        assert_eq!(log.lines[1].text, "Z x");
    }

    #[test]
    fn only_the_end_of_a_long_log_is_kept() {
        let numbers: Vec<String> = (0..MAX_LINES + 5).map(|n| n.to_string()).collect();
        let log = BuildLog::parse(&numbers.join("\n"));
        assert_eq!(log.lines.len(), MAX_LINES);
        assert_eq!(log.omitted, 5);
        assert_eq!(log.lines[0].text, "5");
    }

    #[test]
    fn stepping_between_errors_comes_round() {
        let log = BuildLog::parse(LOG);
        assert_eq!(log.next_error(0), Some(4));
        assert_eq!(log.next_error(5), Some(5));
        assert_eq!(log.next_error(6), Some(4));
        assert_eq!(log.previous_error(5), Some(4));
        assert_eq!(log.previous_error(4), Some(5));
        assert_eq!(BuildLog::parse("fine").next_error(0), None);
    }
}

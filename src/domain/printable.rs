//! Text that slussa did not write, as it may be printed to a terminal. A
//! terminal acts on a control character: an escape sequence in a server's
//! answer or in a remote's address could move the cursor, set the title or hide
//! what comes next. Each such character is replaced by one that is shown as what
//! it is; line breaks and tabs stay.

pub fn printable(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_control() && !matches!(c, '\n' | '\t') {
                char::REPLACEMENT_CHARACTER
            } else {
                c
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::printable;

    #[test]
    fn a_control_character_is_shown_as_a_replacement_and_nothing_else_changes() {
        assert_eq!(
            printable("token rejected\u{1b}]0;title\u{7}\u{1b}[2J\r\nsecond\tline"),
            "token rejected\u{FFFD}]0;title\u{FFFD}\u{FFFD}[2J\u{FFFD}\nsecond\tline"
        );
        assert_eq!(printable("åäö 🦀 ok"), "åäö 🦀 ok");
        assert_eq!(printable(""), "");
    }
}

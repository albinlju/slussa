//! Text as a part of a URL path: what is not plain is escaped, so that a name
//! with a `/`, a `?` or a `#` in it stays inside its place in the path.

use std::fmt::Write;

const fn plain(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

fn escape(text: &str, keep: Option<u8>) -> String {
    text.bytes().fold(String::new(), |mut out, byte| {
        if plain(byte) || Some(byte) == keep {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
        out
    })
}

/// One path segment: a slash is escaped too.
pub fn segment(text: &str) -> String {
    escape(text, None)
}

/// Several segments, as a branch name is (`feature/x`): slashes stay.
pub fn path(text: &str) -> String {
    escape(text, Some(b'/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_segment_keeps_what_is_plain_and_escapes_the_rest() {
        assert_eq!(segment("PLAT"), "PLAT");
        assert_eq!(segment("~jsmith"), "~jsmith", "a personal project's key");
        assert_eq!(segment("pay-ments_api.v2"), "pay-ments_api.v2");
        assert_eq!(segment("a/b"), "a%2Fb");
        assert_eq!(segment("a b?c#d%e"), "a%20b%3Fc%23d%25e");
        assert_eq!(segment("å"), "%C3%A5");
        assert_eq!(segment(""), "");
    }

    #[test]
    fn a_path_keeps_its_slashes() {
        assert_eq!(path("feature/fix #1?"), "feature/fix%20%231%3F");
    }
}

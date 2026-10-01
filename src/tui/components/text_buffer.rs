//! The text of a comment being written, and where the caret is in it.

use ratatui::text::{Line, Span};

/// Text with a caret. The caret is always on a character boundary inside the
/// text: only the methods here move it, so nothing that reads it has to check.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextBuffer {
    text: String,
    caret: usize,
}

impl TextBuffer {
    /// The caret starts after the last character.
    pub const fn new(text: String) -> Self {
        let caret = text.len();
        Self { text, caret }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The caret as a byte offset into the text.
    #[cfg(test)]
    pub const fn caret(&self) -> usize {
        self.caret
    }

    /// Insert at the caret. Line endings become `\n`, a tab becomes four
    /// spaces and other control characters are dropped, so pasted terminal
    /// output cannot put an escape sequence into a comment.
    pub fn insert(&mut self, text: &str) {
        let text: String = text
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .replace('\t', "    ")
            .chars()
            .filter(|c| !c.is_control() || *c == '\n')
            .collect();
        self.text.insert_str(self.caret, &text);
        self.caret += text.len();
    }

    /// One character left (negative) or right.
    pub fn move_horizontal(&mut self, delta: i16) {
        self.caret = if delta < 0 {
            self.previous_char()
        } else {
            let next = self.text[self.caret..].chars().next();
            self.caret + next.map_or(0, char::len_utf8)
        };
    }

    /// One line up (negative) or down, keeping the column where the line is
    /// long enough.
    pub fn move_vertical(&mut self, delta: i16) {
        let (text, pos) = (&self.text, self.caret);
        let start = self.line_start();
        let col = text[start..pos].chars().count();
        let target = if delta < 0 {
            if start == 0 {
                return;
            }
            let end = start - 1;
            (text[..end].rfind('\n').map_or(0, |i| i + 1), end)
        } else {
            let Some(end) = text[pos..].find('\n').map(|i| pos + i) else {
                return;
            };
            let next = end + 1;
            (
                next,
                text[next..].find('\n').map_or(text.len(), |i| next + i),
            )
        };
        self.caret = target.0
            + text[target.0..target.1]
                .char_indices()
                .nth(col)
                .map_or(target.1 - target.0, |(i, _)| i);
    }

    pub fn home(&mut self) {
        self.caret = self.line_start();
    }

    pub fn end(&mut self) {
        self.caret = self.text[self.caret..]
            .find('\n')
            .map_or(self.text.len(), |i| self.caret + i);
    }

    pub fn backspace(&mut self) {
        let previous = self.previous_char();
        self.text.drain(previous..self.caret);
        self.caret = previous;
    }

    pub fn delete(&mut self) {
        if self.caret < self.text.len() {
            self.text.remove(self.caret);
        }
    }

    /// The text wrapped to `width` columns, and the row and column the caret
    /// is drawn at.
    pub fn wrapped(&self, width: usize) -> (Vec<Line<'static>>, usize, usize) {
        visual_lines(&self.text, self.caret, width)
    }

    fn line_start(&self) -> usize {
        self.text[..self.caret].rfind('\n').map_or(0, |i| i + 1)
    }

    /// Where the character before the caret starts; the caret itself at the
    /// start of the text.
    fn previous_char(&self) -> usize {
        self.text[..self.caret]
            .char_indices()
            .next_back()
            .map_or(self.caret, |(i, _)| i)
    }
}

fn visual_lines(text: &str, cursor: usize, width: usize) -> (Vec<Line<'static>>, usize, usize) {
    let mut lines = vec![String::new()];
    let (mut row, mut col) = (0, 0);
    let mut caret = (0, 0);
    for (i, c) in text
        .char_indices()
        .chain(std::iter::once((text.len(), '\0')))
    {
        if c == '\n' {
            if i == cursor {
                caret = if col >= width {
                    (row + 1, 0)
                } else {
                    (row, col)
                };
            }
            lines.push(String::new());
            row += 1;
            col = 0;
            continue;
        }
        let size = Span::raw(c.to_string()).width();
        if col >= width || col + size > width {
            lines.push(String::new());
            row += 1;
            col = 0;
        }
        if i == cursor {
            caret = (row, col);
        }
        if i == text.len() {
            break;
        }
        // `row` is always the last line: it grows with every line pushed.
        if let Some(line) = lines.last_mut() {
            line.push(c);
        }
        col += size;
    }
    (lines.into_iter().map(Line::raw).collect(), caret.0, caret.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_caret_starts_at_the_end_and_moves_by_whole_characters() {
        let mut buffer = TextBuffer::new("å🦀".into());
        assert_eq!(buffer.caret(), "å🦀".len());
        buffer.move_horizontal(1);
        assert_eq!(buffer.caret(), "å🦀".len(), "the end is the end");
        buffer.move_horizontal(-1);
        assert_eq!(buffer.caret(), "å".len());
        buffer.move_horizontal(-1);
        buffer.move_horizontal(-1);
        assert_eq!(buffer.caret(), 0, "the start is the start");
        buffer.backspace();
        assert_eq!(buffer.as_str(), "å🦀");
        buffer.delete();
        assert_eq!(buffer.as_str(), "🦀");
        buffer.move_horizontal(1);
        buffer.backspace();
        buffer.delete();
        assert_eq!((buffer.as_str(), buffer.caret()), ("", 0));
    }

    #[test]
    fn inserted_text_is_cleaned_and_the_caret_follows_it() {
        let mut buffer = TextBuffer::new("å🦀".into());
        buffer.move_horizontal(-1);
        buffer.insert("x\r\ny\tq\u{1b}");
        assert_eq!(buffer.as_str(), "åx\ny    q🦀");
        assert_eq!(buffer.caret(), "åx\ny    q".len());
    }

    #[test]
    fn lines_are_walked_by_column_and_a_shorter_line_ends_the_walk() {
        let mut buffer = TextBuffer::new("långt\nab\ntredje".into());
        buffer.move_vertical(-1);
        assert_eq!(buffer.caret(), "långt\nab".len(), "clamped to the line");
        buffer.move_vertical(-1);
        assert_eq!(buffer.caret(), "lå".len());
        buffer.move_vertical(-1);
        assert_eq!(buffer.caret(), "lå".len(), "no line above");
        buffer.end();
        assert_eq!(buffer.caret(), "långt".len());
        buffer.move_vertical(1);
        buffer.move_vertical(1);
        assert_eq!(buffer.caret(), "långt\nab\ntr".len());
        buffer.move_vertical(1);
        assert_eq!(buffer.caret(), "långt\nab\ntr".len(), "no line below");
        buffer.home();
        assert_eq!(buffer.caret(), "långt\nab\n".len());
    }

    #[test]
    fn wrapped_unicode_cursor_remains_inside_viewport() {
        let (lines, row, col) = visual_lines("1234\nx", 6, 4);
        assert_eq!(lines.len(), 2);
        assert_eq!((row, col), (1, 1));
        for width in [2, 6, 30] {
            let text = "å🦀long text\nsecond line\n";
            for pos in text
                .char_indices()
                .map(|(i, _)| i)
                .chain(std::iter::once(text.len()))
            {
                let (lines, row, col) = visual_lines(text, pos, width);
                assert!(row < lines.len());
                assert!(col < width);
            }
        }
    }
}

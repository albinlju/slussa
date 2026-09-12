#[derive(Debug, Clone)]
pub struct Diff {
    pub revision: Option<DiffRevision>,
    pub files: Vec<FileDiff>,
}

#[derive(Debug, Clone)]
pub struct FileDiff {
    pub path: String,
    pub hunks: Vec<Hunk>,
}

#[derive(Debug, Clone)]
pub struct Hunk {
    pub old_start: usize,
    pub new_start: usize,
    pub lines: Vec<DiffLine>,
}

impl Hunk {
    pub fn numbered_lines(&self) -> NumberedLines<'_> {
        NumberedLines {
            lines: self.lines.iter(),
            new_no: self.new_start,
            old_no: self.old_start,
        }
    }
}

pub struct NumberedLines<'a> {
    lines: std::slice::Iter<'a, DiffLine>,
    new_no: usize,
    old_no: usize,
}

impl<'a> Iterator for NumberedLines<'a> {
    type Item = (&'a DiffLine, usize, usize);

    fn next(&mut self) -> Option<Self::Item> {
        let line = self.lines.next()?;
        let numbered = (line, self.new_no, self.old_no);
        match line {
            DiffLine::Added(_) => self.new_no += 1,
            DiffLine::Removed(_) => self.old_no += 1,
            DiffLine::Context(_) => {
                self.new_no += 1;
                self.old_no += 1;
            }
        }
        Some(numbered)
    }
}

#[derive(Debug, Clone)]
pub enum DiffLine {
    Added(String),
    Removed(String),
    Context(String),
}

impl DiffLine {
    pub fn content(&self) -> &str {
        match self {
            DiffLine::Added(c) | DiffLine::Removed(c) | DiffLine::Context(c) => c,
        }
    }
}

/// Revision of the diff actually displayed, retained with every comment draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffRevision {
    pub head: String,
    pub base: Option<String>,
    pub commit: bool,
}

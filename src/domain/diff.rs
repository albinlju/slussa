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

/// A line number on one side of a diff. A removed line has only an old
/// number, so which side it is on is part of the reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineRef {
    /// In the file as the change leaves it: an added or unchanged line.
    New(usize),
    /// In the file as it was: a removed line.
    Old(usize),
}

impl LineRef {
    pub const fn number(self) -> usize {
        match self {
            Self::New(number) | Self::Old(number) => number,
        }
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
            Self::Added(c) | Self::Removed(c) | Self::Context(c) => c,
        }
    }
}

/// What changed from one commit of a PR to another: what is new since the
/// reader looked.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiffRange {
    pub base: super::commit::CommitOid,
    pub head: super::commit::CommitOid,
}

/// What is new since a commit, as the provider compared the two, with what it
/// takes to keep it to the PR.
#[derive(Debug, Clone)]
pub struct Compared {
    pub diff: Diff,
    /// The files the PR touched at the commit that was read, when the provider
    /// could list them all. One of them that the PR no longer touches was put
    /// back as the target has it, which is new and must be seen. `None` when
    /// they are not known: nothing can then be left out of the diff.
    pub in_pr_before: Option<std::collections::HashSet<String>>,
}

/// Revision of the diff actually displayed, retained with every comment draft.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DiffRevision {
    pub head: String,
    pub base: Option<String>,
    pub commit: bool,
}

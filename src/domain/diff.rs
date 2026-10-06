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

/// How a branch got from the commit that was read to the one it is at, told by
/// where the compare of the two starts: the commit they share.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Moved {
    /// The commit that was read is under the one now, so the compare starts at
    /// it: what it shows is what is new, and all of it.
    Forward,
    /// The one now is under the commit that was read: the branch was reset to an
    /// older commit. Nothing is new, and what came after is gone from it.
    Back,
    /// Neither is under the other: the branch was rebased or force-pushed. The
    /// compare starts at `from`, the commit they share, so it holds again what
    /// the commit that was read already had, and nothing of what was dropped.
    Rewritten { from: super::commit::CommitOid },
}

impl DiffRange {
    /// How the branch moved over this range, as `compared` says: the diff of
    /// it, whose base is the commit the compare started from. `None` when the
    /// diff does not say where it starts.
    pub fn moved(&self, compared: &Diff) -> Option<Moved> {
        let from = compared.revision.as_ref()?.base.as_deref()?;
        let from = super::commit::CommitOid::parse(from)?;
        Some(if from == self.base {
            Moved::Forward
        } else if from == self.head {
            Moved::Back
        } else {
            Moved::Rewritten { from }
        })
    }
}

/// Revision of the diff actually displayed, retained with every comment draft.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DiffRevision {
    pub head: String,
    pub base: Option<String>,
    pub commit: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::commit::CommitOid;

    fn oid(text: &str) -> CommitOid {
        CommitOid::parse(text).expect("a commit id")
    }

    fn compared_from(from: Option<&str>) -> Diff {
        Diff {
            revision: Some(DiffRevision {
                head: "bbb222".into(),
                base: from.map(Into::into),
                commit: true,
            }),
            files: vec![],
        }
    }

    #[test]
    fn where_the_compare_starts_says_how_the_branch_moved() {
        let range = DiffRange {
            base: oid("aaa111"),
            head: oid("bbb222"),
        };
        let moved = |from| range.moved(&compared_from(from));
        assert_eq!(moved(Some("aaa111")), Some(Moved::Forward));
        assert_eq!(moved(Some("bbb222")), Some(Moved::Back));
        assert_eq!(
            moved(Some("ccc333")),
            Some(Moved::Rewritten {
                from: oid("ccc333")
            })
        );
        assert_eq!(
            moved(None),
            None,
            "a diff that does not say is not guessed at"
        );
    }
}

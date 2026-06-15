#[derive(Debug, Clone)]
pub struct Diff {
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

pub struct Diff {
    pub files: Vec<FileDiff>,
}

pub struct FileDiff {
    pub path: String,
    pub hunks: Vec<Hunk>,
}

pub struct Hunk {
    pub old_start: usize,
    pub new_start: usize,
    pub lines: Vec<DiffLine>,
}

pub enum DiffLine {
    Added(String),
    Removed(String),
    Context(String),
}

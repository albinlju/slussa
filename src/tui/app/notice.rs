//! The one-line message shown over the footer for a moment.

/// What a notice tells: that something went through, or that it did not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    Info,
    Error,
}

#[derive(Debug)]
pub struct Notice {
    pub message: String,
    pub kind: NoticeKind,
    created: std::time::Instant,
}
impl Notice {
    pub fn info(message: String) -> Self {
        Self::new(message, NoticeKind::Info)
    }
    pub fn error(message: String) -> Self {
        Self::new(message, NoticeKind::Error)
    }
    fn new(message: String, kind: NoticeKind) -> Self {
        Self {
            message,
            kind,
            created: std::time::Instant::now(),
        }
    }
    /// A failure stays long enough to be read.
    pub fn visible(&self) -> bool {
        let seconds = match self.kind {
            NoticeKind::Info => 2,
            NoticeKind::Error => 5,
        };
        self.created.elapsed() < std::time::Duration::from_secs(seconds)
    }
}

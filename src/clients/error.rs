use std::fmt;

#[derive(Debug)]
#[allow(dead_code)]
pub enum FetchError {
    /// `gh` binary couldn't be spawned at all.
    GhMissing,
    /// `gh` ran but returned a non-zero exit code. Stderr is captured so we
    /// can show the user what gh complained about.
    GhFailed { code: Option<i32>, stderr: String },
    /// HTTP request to a REST API (Bitbucket Data Center, Cloud) failed with
    /// a non-2xx status. Body is included for diagnosis.
    HttpFailed { status: u16, body: String },
    /// Network-level error before we even got an HTTP response — DNS,
    /// connect, TLS, etc.
    Network(String),
    /// Auth credentials are missing for the host — caller should send the
    /// user through `tuipr auth login`.
    NotAuthenticated { host: String },
    /// The response didn't match the schema we expected.
    ParseFailed(String),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GhMissing => write!(f, "gh CLI not available"),
            Self::GhFailed { code, stderr } => {
                let stderr = stderr.trim();
                match (code, stderr.is_empty()) {
                    (Some(c), false) => write!(f, "gh exited with code {c}: {stderr}"),
                    (Some(c), true) => write!(f, "gh exited with code {c}"),
                    (None, false) => write!(f, "gh failed: {stderr}"),
                    (None, true) => write!(f, "gh failed"),
                }
            }
            Self::HttpFailed { status, body } => {
                let body = body.trim();
                if body.is_empty() {
                    write!(f, "http {status}")
                } else {
                    write!(f, "http {status}: {body}")
                }
            }
            Self::Network(msg) => write!(f, "network error: {msg}"),
            Self::NotAuthenticated { host } => {
                write!(f, "not logged in to {host} — run `tuipr auth login`")
            }
            Self::ParseFailed(msg) => write!(f, "couldn't parse response: {msg}"),
        }
    }
}

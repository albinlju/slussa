use std::fmt;

#[derive(Debug)]
pub enum FetchError {
    /// `gh` binary couldn't be spawned at all.
    GhMissing,
    /// `gh` ran but exited non-zero.
    GhFailed { code: Option<i32>, stderr: String },
    /// Non-2xx from a REST API; body kept for diagnosis.
    HttpFailed { status: u16, body: String },
    /// Network-level error before any HTTP response — DNS, connect, TLS.
    Network(String),
    /// No credentials for the host — the user needs `tuipr auth login`.
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

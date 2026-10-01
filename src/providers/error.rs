use thiserror::Error;

/// The error a failure wraps, kept so it can be logged and inspected.
type Source = Box<dyn std::error::Error + Send + Sync>;

/// Why a provider call failed. `Display` is for the log; `user_message` is
/// what the UI shows.
#[derive(Debug, Error)]
pub enum FetchError {
    #[error("gh CLI not available")]
    GhMissing,
    /// The request cannot be made from what the user has in front of them.
    #[error("{0}")]
    InvalidInput(String),
    /// The provider has no such operation.
    #[error("{0}")]
    Unsupported(String),
    /// What was read no longer matches the server. Reading again fixes it.
    #[error("{0}")]
    Stale(String),
    /// The server cut its answer short.
    #[error("{0}")]
    Truncated(String),
    /// GitHub answered a GraphQL query with errors.
    #[error("graphql errors: {}", .0.join("; "))]
    GraphQl(Vec<String>),
    #[error("request timed out")]
    Timeout,
    #[error("{}", gh_failed(*code, stderr))]
    GhFailed { code: Option<i32>, stderr: String },
    #[error("{}", http_failed(*status, body))]
    HttpFailed { status: u16, body: String },
    #[error("network error: {0}")]
    Network(#[source] Source),
    #[error("not logged in to {host} — run `slussa auth login`")]
    NotAuthenticated { host: String },
    #[error("couldn't parse response: {0}")]
    ParseFailed(#[source] Source),
    #[error("worker thread panicked: {0}")]
    WorkerPanicked(String),
}

/// Why a review sent as a batch failed.
#[derive(Debug, Error)]
pub enum ReviewError {
    /// Nothing is known to have been posted.
    #[error(transparent)]
    Failed(#[from] FetchError),
    /// The provider sends a review as several requests, and one of them
    /// failed after others had arrived.
    #[error(
        "review partially sent ({posted_comments} comments, summary={summary_posted}): {source}"
    )]
    Partial {
        posted_comments: usize,
        summary_posted: bool,
        source: FetchError,
    },
}

fn gh_failed(code: Option<i32>, stderr: &str) -> String {
    let stderr = stderr.trim();
    match (code, stderr.is_empty()) {
        (Some(c), false) => format!("gh exited with code {c}: {stderr}"),
        (Some(c), true) => format!("gh exited with code {c}"),
        (None, false) => format!("gh failed: {stderr}"),
        (None, true) => "gh failed".to_owned(),
    }
}

fn http_failed(status: u16, body: &str) -> String {
    let body = body.trim();
    if body.is_empty() {
        format!("http {status}")
    } else {
        format!("http {status}: {body}")
    }
}

impl FetchError {
    /// A short, human-facing message for the UI (popup / failed-load state).
    /// The raw `Display` form is kept for logs.
    pub fn user_message(&self) -> String {
        match self {
            Self::InvalidInput(msg)
            | Self::Unsupported(msg)
            | Self::Stale(msg)
            | Self::Truncated(msg) => msg.clone(),
            Self::GraphQl(_) => "GitHub returned an incomplete GraphQL response.".to_owned(),
            Self::Timeout => "The request timed out. Check the PR before retrying: the server may have applied the change.".into(),
            Self::GhMissing => "GitHub CLI (gh) isn't installed or on your PATH.".to_owned(),
            Self::GhFailed { stderr, .. } => {
                api_message(stderr).unwrap_or_else(|| clean_gh(stderr))
            }
            Self::HttpFailed { status, body } => {
                api_message(body).unwrap_or_else(|| match status {
                    401 | 403 => "Not authorized — check your token's permissions.".to_owned(),
                    404 => "Not found.".to_owned(),
                    _ => format!("Request failed (HTTP {status})."),
                })
            }
            Self::Network(_) => "Couldn't reach the server.".to_owned(),
            Self::NotAuthenticated { host } => {
                format!("Not logged in to {host} — run `slussa auth login`.")
            }
            Self::ParseFailed(_) => "Couldn't read the server response.".to_owned(),
            Self::WorkerPanicked(reason) => format!("worker thread panicked: {reason}"),
        }
    }

    /// Whether a write that failed this way may still have reached the server.
    /// Only a failure before the request left, or a refusal the server stated,
    /// says no; everything else leaves the outcome open.
    pub const fn may_have_reached_server(&self) -> bool {
        match self {
            Self::GhMissing
            | Self::InvalidInput(_)
            | Self::Unsupported(_)
            | Self::Stale(_)
            | Self::Truncated(_)
            | Self::NotAuthenticated { .. } => false,
            // A 4xx is the server saying no, except 408: a request that timed
            // out on the way in may have been acted on. A 429 was turned away
            // before it was handled.
            Self::HttpFailed { status, .. } => *status >= 500 || *status == 408,
            Self::GraphQl(_)
            | Self::Timeout
            | Self::GhFailed { .. }
            | Self::Network(_)
            | Self::ParseFailed(_)
            | Self::WorkerPanicked(_) => true,
        }
    }
}

/// Pull the meaningful message out of a JSON error body (GitHub and Bitbucket
/// both use `errors[].message` and/or a top-level `message`).
fn api_message(raw: &str) -> Option<String> {
    let start = raw.find('{')?;
    let v: serde_json::Value = serde_json::from_str(raw[start..].trim()).ok()?;
    if let Some(errors) = v.get("errors").and_then(|e| e.as_array()) {
        let msgs: Vec<String> = errors
            .iter()
            .filter_map(|e| {
                let message = e
                    .as_str()
                    .or_else(|| e.get("message").and_then(serde_json::Value::as_str))?;
                Some(match veto_reasons(e) {
                    reasons if reasons.is_empty() => message.to_owned(),
                    reasons => format!("{message}: {}", reasons.join("; ")),
                })
            })
            .collect();
        if !msgs.is_empty() {
            return Some(msgs.join("; "));
        }
    }
    v.get("message")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

/// Bitbucket explains a refused merge in `vetoes`, each with a short summary,
/// beside a generic "Merging is vetoed" message.
fn veto_reasons(error: &serde_json::Value) -> Vec<String> {
    error
        .get("vetoes")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|veto| veto.get("summaryMessage")?.as_str())
        .filter(|summary| !summary.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Strip `gh:` noise and a trailing `(HTTP nnn)` when there's no JSON body.
fn clean_gh(stderr: &str) -> String {
    let s = stderr.trim().strip_prefix("gh:").unwrap_or(stderr).trim();
    match s.find("(HTTP") {
        Some(i) => s[..i].trim_end().to_owned(),
        None => s.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_veto_list_is_appended_to_the_generic_message() {
        let body = r#"{"errors":[{"message":"Merging is vetoed","vetoes":[{"summaryMessage":"Builds failing"},{"summaryMessage":""},{"detailedMessage":"no summary"}]}]}"#;
        assert_eq!(
            api_message(body).as_deref(),
            Some("Merging is vetoed: Builds failing")
        );
        let plain = r#"{"errors":[{"message":"Not found"}]}"#;
        assert_eq!(api_message(plain).as_deref(), Some("Not found"));
    }

    use super::*;

    #[test]
    fn user_message_extracts_github_validation_error() {
        let err = FetchError::GhFailed {
            code: Some(1),
            stderr: "gh: Validation Failed (HTTP 422) {\"message\":\"Validation Failed\",\
                \"errors\":[{\"resource\":\"PullRequestReview\",\"code\":\"custom\",\
                \"message\":\"Can not approve your own pull request\"}]}"
                .to_string(),
        };
        assert_eq!(err.user_message(), "Can not approve your own pull request");
    }

    #[test]
    fn user_message_extracts_bitbucket_error() {
        let err = FetchError::HttpFailed {
            status: 409,
            body: "{\"errors\":[{\"message\":\"You are already a reviewer.\"}]}".to_string(),
        };
        assert_eq!(err.user_message(), "You are already a reviewer.");
    }

    #[test]
    fn a_failure_before_sending_or_a_stated_refusal_cannot_have_arrived() {
        let http = |status| FetchError::HttpFailed {
            status,
            body: String::new(),
        };
        for unsent in [
            FetchError::GhMissing,
            FetchError::InvalidInput("no revision".into()),
            FetchError::Unsupported("no such strategy".into()),
            FetchError::NotAuthenticated { host: "h".into() },
            http(409),
            http(403),
            http(429),
        ] {
            assert!(!unsent.may_have_reached_server(), "{unsent:?}");
        }
        for open in [
            FetchError::Timeout,
            FetchError::Network("connection reset".into()),
            FetchError::ParseFailed("not json".into()),
            FetchError::WorkerPanicked("boom".into()),
            FetchError::GhFailed {
                code: Some(1),
                stderr: String::new(),
            },
            http(502),
            http(408),
        ] {
            assert!(open.may_have_reached_server(), "{open:?}");
        }
    }

    #[test]
    fn the_wrapped_error_is_kept_and_the_log_text_is_unchanged() {
        use std::error::Error;
        let wrapped = std::io::Error::other("connection reset");
        let error = FetchError::Network(wrapped.into());
        assert_eq!(error.to_string(), "network error: connection reset");
        assert_eq!(error.source().unwrap().to_string(), "connection reset");

        let failed = FetchError::GhFailed {
            code: Some(1),
            stderr: " boom \n".into(),
        };
        assert_eq!(failed.to_string(), "gh exited with code 1: boom");
        let http = FetchError::HttpFailed {
            status: 500,
            body: String::new(),
        };
        assert_eq!(http.to_string(), "http 500");
        let partial = ReviewError::Partial {
            posted_comments: 2,
            summary_posted: true,
            source: FetchError::Timeout,
        };
        assert_eq!(
            partial.to_string(),
            "review partially sent (2 comments, summary=true): request timed out"
        );
    }

    #[test]
    fn user_message_cleans_gh_noise_without_json() {
        let err = FetchError::GhFailed {
            code: Some(1),
            stderr: "gh: Could not resolve to a Repository (HTTP 404)".to_string(),
        };
        assert_eq!(err.user_message(), "Could not resolve to a Repository");
    }
}

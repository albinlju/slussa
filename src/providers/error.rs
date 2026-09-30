use std::fmt;

#[derive(Debug)]
pub enum FetchError {
    GhMissing,
    InvalidInput(String),
    Timeout,
    PartialReview {
        posted_comments: usize,
        summary_posted: bool,
        source: Box<Self>,
    },
    GhFailed {
        code: Option<i32>,
        stderr: String,
    },
    HttpFailed {
        status: u16,
        body: String,
    },
    Network(String),
    NotAuthenticated {
        host: String,
    },
    ParseFailed(String),
}

impl FetchError {
    /// A short, human-facing message for the UI (popup / failed-load state).
    /// The raw `Display` form is kept for logs.
    pub fn user_message(&self) -> String {
        match self {
            Self::PartialReview { source, .. } => source.user_message(),
            Self::InvalidInput(msg) => msg.clone(),
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
                format!("Not logged in to {host} — run `tuipr auth login`.")
            }
            Self::ParseFailed(_) => "Couldn't read the server response.".to_owned(),
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

/// Strip `gh:` noise and a trailing `(HTTP nnn)` when there's no JSON body.
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

fn clean_gh(stderr: &str) -> String {
    let s = stderr.trim().strip_prefix("gh:").unwrap_or(stderr).trim();
    match s.find("(HTTP") {
        Some(i) => s[..i].trim_end().to_owned(),
        None => s.to_owned(),
    }
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PartialReview {
                posted_comments,
                summary_posted,
                source,
            } => write!(
                f,
                "review partially sent ({posted_comments} comments, summary={summary_posted}): {source}"
            ),
            Self::InvalidInput(msg) => write!(f, "{msg}"),
            Self::Timeout => write!(f, "request timed out"),
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
    fn user_message_cleans_gh_noise_without_json() {
        let err = FetchError::GhFailed {
            code: Some(1),
            stderr: "gh: Could not resolve to a Repository (HTTP 404)".to_string(),
        };
        assert_eq!(err.user_message(), "Could not resolve to a Repository");
    }
}

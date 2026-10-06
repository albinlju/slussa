//! The document an agent hands in: the commit it read, an optional summary and
//! comments on lines. Read here, the same way whether the agent called
//! `slussa propose import` or slussa asked it for a review, and checked all at
//! once: a document with one thing wrong is refused whole, and the message
//! names the thing.
//!
//! The document is a public contract and `"schema": 1` is marked experimental
//! until it is used; its types are separate from the domain's, so a change
//! inside does not change what an agent writes.

use super::{
    commit::CommitOid,
    proposal::{Proposal, ProposalError, ProposalInput, Side, Summary},
};
use serde::Deserialize;

/// The most comments one document may hold, so that an agent that loops cannot
/// fill the disk or the screen.
pub const MAX_COMMENTS: usize = 500;

const DOCUMENT_FIELDS: &[&str] = &["schema", "head", "agent", "summary", "comments"];
const COMMENT_FIELDS: &[&str] = &["path", "line", "side", "body", "id"];

/// Who handed the document in, which decides how strictly it is read.
#[derive(Debug, Clone, Copy)]
pub enum Source<'a> {
    /// An agent that called `slussa propose import`: `schema` and `head` are
    /// required and a field that is not in the schema is refused, so that a
    /// misspelling is not ignored.
    Caller,
    /// What an agent answered when slussa asked it for a review. slussa gave it
    /// the diff of `head`, so `head` is slussa's and not the agent's: a
    /// `head` in the answer is ignored. A model adds fields and forgets
    /// `schema`; neither is an error here.
    Asked { head: &'a CommitOid, agent: &'a str },
}

#[derive(Deserialize)]
struct Document {
    schema: Option<u32>,
    head: Option<String>,
    agent: Option<String>,
    summary: Option<String>,
    #[serde(default)]
    comments: Vec<CommentDocument>,
}

#[derive(Deserialize)]
struct CommentDocument {
    path: String,
    line: usize,
    #[serde(default = "new_side")]
    side: Side,
    body: String,
    id: Option<String>,
}

const fn new_side() -> Side {
    Side::New
}

/// What a document proposes, each part made through its own rule.
#[derive(Debug)]
pub struct Parsed {
    pub head: CommitOid,
    pub comments: Vec<Proposal>,
    pub summary: Option<Summary>,
}

fn unknown_field(value: &serde_json::Value, known: &[&str], at: &str) -> Option<String> {
    let object = value.as_object()?;
    object
        .keys()
        .find(|key| !known.contains(&key.as_str()))
        .map(|key| format!("{at}unknown field `{key}`"))
}

pub fn parse(bytes: &[u8], source: Source<'_>) -> Result<Parsed, String> {
    let unreadable = |e: serde_json::Error| format!("the document cannot be read: {e}");
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(unreadable)?;
    if matches!(source, Source::Caller) {
        if let Some(message) = unknown_field(&value, DOCUMENT_FIELDS, "") {
            return Err(message);
        }
        let comments = value.get("comments").and_then(serde_json::Value::as_array);
        for (at, comment) in comments.into_iter().flatten().enumerate() {
            if let Some(message) =
                unknown_field(comment, COMMENT_FIELDS, &format!("comments[{at}]: "))
            {
                return Err(message);
            }
        }
    }
    let document: Document = serde_json::from_value(value).map_err(unreadable)?;
    match (document.schema, source) {
        (Some(1), _) | (None, Source::Asked { .. }) => {}
        (Some(other), _) => return Err(format!("`schema` is {other}; this slussa reads 1")),
        (None, Source::Caller) => return Err("`schema` is missing; this slussa reads 1".into()),
    }
    if document.comments.len() > MAX_COMMENTS {
        return Err(format!(
            "the document has more than {MAX_COMMENTS} comments"
        ));
    }
    let (head, agent) = match source {
        Source::Caller => {
            let head = document.head.as_deref().unwrap_or_default();
            let head =
                CommitOid::parse(head).ok_or_else(|| format!("`head`: {}", ProposalError::Head))?;
            (head, document.agent)
        }
        Source::Asked { head, agent } => (
            head.clone(),
            document.agent.or_else(|| Some(agent.to_owned())),
        ),
    };
    let mut comments = Vec::with_capacity(document.comments.len());
    for (at, comment) in document.comments.into_iter().enumerate() {
        comments.push(
            Proposal::new(ProposalInput {
                head: head.to_string(),
                path: comment.path,
                line: comment.line,
                side: comment.side,
                body: comment.body,
                id: comment.id,
                agent: agent.clone(),
            })
            .map_err(|e| format!("comments[{at}]: {e}"))?,
        );
    }
    let summary = document
        .summary
        .map(|text| Summary::new(head.as_str(), text, agent.clone()))
        .transpose()
        .map_err(|e| format!("`summary`: {e}"))?;
    Ok(Parsed {
        head,
        comments,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOCUMENT: &str = r#"{
        "schema": 1, "head": "abc123", "agent": "reviewer", "summary": "Two things.",
        "comments": [
            {"path": "src/a.rs", "line": 12, "body": "This can panic.", "id": "F1"},
            {"path": "src/a.rs", "line": 3, "side": "old", "body": "Why was this removed?"}
        ]
    }"#;

    fn oid(text: &str) -> CommitOid {
        CommitOid::parse(text).unwrap()
    }

    #[test]
    fn a_document_becomes_proposals_bound_to_its_head() {
        let parsed = parse(DOCUMENT.as_bytes(), Source::Caller).unwrap();
        assert_eq!(parsed.head.as_str(), "abc123");
        assert_eq!(parsed.comments.len(), 2);
        assert!(parsed.summary.is_some());
    }

    #[test]
    fn what_is_wrong_with_a_document_from_a_caller_is_named() {
        let wrong = |text: &str| parse(text.as_bytes(), Source::Caller).unwrap_err();
        assert!(wrong("not json").contains("cannot be read"));
        assert!(wrong(r#"{"schema": 2, "head": "abc123"}"#).contains("reads 1"));
        assert!(wrong(r#"{"head": "abc123"}"#).contains("`schema` is missing"));
        assert!(wrong(r#"{"schema": 1, "head": "main"}"#).contains("`head`"));
        assert!(wrong(r#"{"schema": 1}"#).contains("`head`"));
        let message = wrong(
            r#"{"schema": 1, "head": "abc123", "comments": [
                {"path": "a", "line": 1, "body": "ok"},
                {"path": "a", "line": 0, "body": "no"}]}"#,
        );
        assert!(
            message.contains("comments[1]") && message.contains("`line`"),
            "{message}"
        );
        assert!(wrong(r#"{"schema": 1, "head": "abc123", "coments": []}"#).contains("coments"));
        let message = wrong(
            r#"{"schema": 1, "head": "abc123", "comments": [
                {"path": "a", "line": 1, "body": "x", "severity": "high"}]}"#,
        );
        assert!(
            message.contains("comments[0]") && message.contains("severity"),
            "{message}"
        );
    }

    #[test]
    fn an_answer_to_a_review_is_read_leniently_and_bound_to_slussas_head() {
        let head = oid("fedcba");
        let asked = Source::Asked {
            head: &head,
            agent: "claude",
        };
        // No schema, an extra field, and a head of the agent's own that is ignored.
        let parsed = parse(
            br#"{"head": "0123456789", "mood": "keen",
                "comments": [{"path": "a.rs", "line": 2, "body": "Look here.", "confidence": 3}]}"#,
            asked,
        )
        .unwrap();
        assert_eq!(parsed.head, head);
        assert_eq!(parsed.comments.len(), 1);
        // The agent's own name is kept when it gave one, else the command's.
        let named = parse(br#"{"agent": "gator", "comments": []}"#, asked).unwrap();
        assert!(named.comments.is_empty());
        // Wrong in a way that matters is still wrong.
        assert!(parse(br#"{"schema": 2}"#, asked).is_err());
        assert!(
            parse(
                br#"{"comments": [{"path": "a", "line": 0, "body": "x"}]}"#,
                asked
            )
            .is_err()
        );
    }

    #[test]
    fn too_many_comments_are_refused() {
        let comment = r#"{"path": "a", "line": 1, "body": "x"}"#;
        let many = vec![comment; MAX_COMMENTS + 1].join(",");
        let text = format!(r#"{{"schema": 1, "head": "abc123", "comments": [{many}]}}"#);
        assert!(
            parse(text.as_bytes(), Source::Caller)
                .unwrap_err()
                .contains("more than")
        );
    }
}

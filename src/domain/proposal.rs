//! What an agent proposes on a PR, kept apart from what the reader writes: a
//! comment on a line, or a summary, both bound to the commit the agent read.
//! Nothing here is posted. The reader sends, edits or discards each one.
//!
//! A proposal can only be made through `Proposal::new` and `Summary::new`, so
//! one that is blank, points at no line or at a commit that is not one does not
//! exist, and the file it is kept in is read through the same rule.

use super::{comment::NonBlank, commit::CommitOid};
use serde::{Deserialize, Serialize};
use std::num::NonZeroUsize;

/// How long an agent's text may be, so that a runaway one cannot fill the file
/// or the screen.
pub const MAX_BODY: usize = 16_000;
pub(super) const MAX_NAME: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    /// A line of the file as the PR leaves it.
    New,
    /// A line of the file as it was.
    Old,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProposalError {
    #[error("`head` is not a commit id")]
    Head,
    #[error("`path` is empty")]
    Path,
    #[error("`line` is not a line number: it starts at 1")]
    Line,
    #[error("`body` is empty")]
    Body,
    #[error("`body` is longer than {MAX_BODY} characters")]
    BodyTooLong,
    #[error("`agent` is empty or longer than {MAX_NAME} characters")]
    Agent,
    #[error("`id` is empty or longer than {MAX_NAME} characters")]
    Id,
}

/// Who proposed it, as the agent names itself: shown beside the proposal, never
/// trusted for anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AgentName(String);

impl TryFrom<String> for AgentName {
    type Error = ProposalError;

    fn try_from(text: String) -> Result<Self, ProposalError> {
        let text = text.trim().to_owned();
        let fits = !text.is_empty()
            && text.chars().count() <= MAX_NAME
            && !text.chars().any(char::is_control);
        fits.then_some(Self(text)).ok_or(ProposalError::Agent)
    }
}

impl From<AgentName> for String {
    fn from(name: AgentName) -> Self {
        name.0
    }
}

impl AgentName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The agent's own name for a finding, kept so that the same finding sent again
/// in other words is not shown twice (`Proposal::same_as`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct FindingId(String);

impl TryFrom<String> for FindingId {
    type Error = ProposalError;

    fn try_from(text: String) -> Result<Self, ProposalError> {
        let fits = !text.trim().is_empty()
            && text.chars().count() <= MAX_NAME
            && !text.chars().any(char::is_control);
        fits.then_some(Self(text)).ok_or(ProposalError::Id)
    }
}

impl From<FindingId> for String {
    fn from(id: FindingId) -> Self {
        id.0
    }
}

fn body(text: String) -> Result<NonBlank, ProposalError> {
    if text.chars().count() > MAX_BODY {
        return Err(ProposalError::BodyTooLong);
    }
    NonBlank::new(text).ok_or(ProposalError::Body)
}

/// What a proposal is made from, as an agent hands it in.
#[derive(Debug, Clone)]
pub struct ProposalInput {
    pub head: String,
    pub path: String,
    pub line: usize,
    pub side: Side,
    pub body: String,
    pub id: Option<String>,
    pub agent: Option<String>,
}

/// A comment on one line, as the agent read the PR at `head`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawProposal", into = "RawProposal")]
pub struct Proposal {
    head: CommitOid,
    path: NonBlank,
    line: NonZeroUsize,
    side: Side,
    body: NonBlank,
    id: Option<FindingId>,
    agent: Option<AgentName>,
}

/// The spelling on disk of a proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawProposal {
    head: String,
    path: String,
    line: usize,
    side: Side,
    body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    agent: Option<String>,
}

impl TryFrom<RawProposal> for Proposal {
    type Error = ProposalError;

    fn try_from(raw: RawProposal) -> Result<Self, ProposalError> {
        Self::new(ProposalInput {
            head: raw.head,
            path: raw.path,
            line: raw.line,
            side: raw.side,
            body: raw.body,
            id: raw.id,
            agent: raw.agent,
        })
    }
}

impl From<Proposal> for RawProposal {
    fn from(proposal: Proposal) -> Self {
        Self {
            head: proposal.head.to_string(),
            path: proposal.path.into_string(),
            line: proposal.line.get(),
            side: proposal.side,
            body: proposal.body.into_string(),
            id: proposal.id.map(String::from),
            agent: proposal.agent.map(String::from),
        }
    }
}

impl Proposal {
    pub fn new(input: ProposalInput) -> Result<Self, ProposalError> {
        Ok(Self {
            head: CommitOid::parse(&input.head).ok_or(ProposalError::Head)?,
            path: NonBlank::new(input.path).ok_or(ProposalError::Path)?,
            line: NonZeroUsize::new(input.line).ok_or(ProposalError::Line)?,
            side: input.side,
            body: body(input.body)?,
            id: input.id.map(FindingId::try_from).transpose()?,
            agent: input.agent.map(AgentName::try_from).transpose()?,
        })
    }

    pub const fn head(&self) -> &CommitOid {
        &self.head
    }

    /// The same proposal with its commit written out, when `full` is the commit
    /// its `head` abbreviates: what an agent wrote short is still that commit,
    /// and is shown on its diff.
    #[must_use]
    pub fn written_out(mut self, full: &CommitOid) -> Self {
        if self.head.abbreviates(full) {
            self.head = full.clone();
        }
        self
    }

    pub fn path(&self) -> &str {
        self.path.as_str()
    }

    pub const fn line(&self) -> usize {
        self.line.get()
    }

    pub const fn side(&self) -> Side {
        self.side
    }

    pub fn body(&self) -> &str {
        self.body.as_str()
    }

    pub fn agent(&self) -> Option<&str> {
        self.agent.as_ref().map(AgentName::as_str)
    }

    /// The same finding again: on the same line of the same commit, in the same
    /// words or under the id the same agent gave it before. An id alone does not
    /// say: agents number what they find from the start each time, so two
    /// reviews both have an `F1`, and they are not the same finding.
    pub fn same_as(&self, other: &Self) -> bool {
        let same_place = self.head == other.head
            && self.path == other.path
            && self.line == other.line
            && self.side == other.side;
        let named_again = self.agent == other.agent
            && matches!((&self.id, &other.id), (Some(a), Some(b)) if a == b);
        same_place && (named_again || self.body == other.body)
    }
}

/// What the agent makes of the PR as a whole, as it read it at `head`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawSummary", into = "RawSummary")]
pub struct Summary {
    head: CommitOid,
    text: NonBlank,
    agent: Option<AgentName>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawSummary {
    head: String,
    text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    agent: Option<String>,
}

impl TryFrom<RawSummary> for Summary {
    type Error = ProposalError;

    fn try_from(raw: RawSummary) -> Result<Self, ProposalError> {
        Self::new(&raw.head, raw.text, raw.agent)
    }
}

impl From<Summary> for RawSummary {
    fn from(summary: Summary) -> Self {
        Self {
            head: summary.head.to_string(),
            text: summary.text.into_string(),
            agent: summary.agent.map(String::from),
        }
    }
}

impl Summary {
    pub fn new(head: &str, text: String, agent: Option<String>) -> Result<Self, ProposalError> {
        Ok(Self {
            head: CommitOid::parse(head).ok_or(ProposalError::Head)?,
            text: body(text)?,
            agent: agent.map(AgentName::try_from).transpose()?,
        })
    }

    pub const fn head(&self) -> &CommitOid {
        &self.head
    }

    /// The same summary with its commit written out, as `Proposal::written_out`.
    #[must_use]
    pub fn written_out(mut self, full: &CommitOid) -> Self {
        if self.head.abbreviates(full) {
            self.head = full.clone();
        }
        self
    }

    pub fn text(&self) -> &str {
        self.text.as_str()
    }

    pub fn agent(&self) -> Option<&str> {
        self.agent.as_ref().map(AgentName::as_str)
    }

    pub fn same_as(&self, other: &Self) -> bool {
        self.head == other.head && self.text == other.text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> ProposalInput {
        ProposalInput {
            head: "abc123".into(),
            path: "src/a.rs".into(),
            line: 12,
            side: Side::New,
            body: "This can panic.".into(),
            id: None,
            agent: Some("reviewer".into()),
        }
    }

    #[test]
    fn a_proposal_with_nothing_wrong_with_it_is_made() {
        assert!(Proposal::new(input()).is_ok());
    }

    #[test]
    fn what_cannot_be_a_proposal_is_refused_with_the_field_that_is_wrong() {
        let wrong = |change: fn(&mut ProposalInput)| {
            let mut input = input();
            change(&mut input);
            Proposal::new(input).unwrap_err()
        };
        assert_eq!(wrong(|i| i.head = "main".into()), ProposalError::Head);
        assert_eq!(wrong(|i| i.path = "  ".into()), ProposalError::Path);
        assert_eq!(wrong(|i| i.line = 0), ProposalError::Line);
        assert_eq!(wrong(|i| i.body = " \n".into()), ProposalError::Body);
        assert_eq!(
            wrong(|i| i.body = "x".repeat(MAX_BODY + 1)),
            ProposalError::BodyTooLong
        );
        assert_eq!(
            wrong(|i| i.agent = Some("a\u{1b}[2J".into())),
            ProposalError::Agent
        );
        assert_eq!(wrong(|i| i.id = Some(String::new())), ProposalError::Id);
    }

    #[test]
    fn the_same_finding_is_one_on_the_same_line_by_its_words_or_its_agents_id() {
        let first = Proposal::new(input()).unwrap();
        assert!(first.same_as(&Proposal::new(input()).unwrap()));

        let mut other = input();
        other.body = "Another thing.".into();
        assert!(!first.same_as(&Proposal::new(other).unwrap()));

        // The agent's own id says it is the same finding in other words, on the
        // line it was on.
        let with_id = |id: &str, body: &str, line: usize| {
            let mut input = input();
            input.id = Some(id.into());
            input.body = body.into();
            input.line = line;
            Proposal::new(input).unwrap()
        };
        assert!(with_id("F1", "words", 3).same_as(&with_id("F1", "other words", 3)));
        assert!(!with_id("F1", "words", 3).same_as(&with_id("F2", "other words", 3)));
        // Two reviews both number from F1: the id on another line, or from
        // another agent, is another finding.
        assert!(!with_id("F1", "words", 3).same_as(&with_id("F1", "other words", 9)));
        let mut another = input();
        another.id = Some("F1".into());
        another.line = 3;
        another.body = "other words".into();
        another.agent = Some("second".into());
        assert!(!with_id("F1", "words", 3).same_as(&Proposal::new(another).unwrap()));
        // The same words on the same line are one finding, whatever it is called.
        assert!(with_id("F1", "words", 3).same_as(&with_id("F2", "words", 3)));

        // Another commit is another finding.
        let mut later = input();
        later.head = "def456".into();
        assert!(!first.same_as(&Proposal::new(later).unwrap()));
    }

    #[test]
    fn a_proposal_is_read_back_through_the_same_rule() {
        let text = serde_json::to_string(&Proposal::new(input()).unwrap()).unwrap();
        assert_eq!(
            text,
            r#"{"head":"abc123","path":"src/a.rs","line":12,"side":"new","body":"This can panic.","agent":"reviewer"}"#
        );
        assert!(serde_json::from_str::<Proposal>(&text).is_ok());
        let bad = text.replace("\"line\":12", "\"line\":0");
        assert!(serde_json::from_str::<Proposal>(&bad).is_err());
    }

    #[test]
    fn a_summary_needs_words_and_a_commit() {
        assert!(Summary::new("abc123", "Looks fine.".into(), None).is_ok());
        assert_eq!(
            Summary::new("abc123", " ".into(), None).unwrap_err(),
            ProposalError::Body
        );
        assert_eq!(
            Summary::new("nope", "x".into(), None).unwrap_err(),
            ProposalError::Head
        );
    }
}

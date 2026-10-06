//! Asking an agent to review a PR: what the reader asks for with `A`. The PR's
//! title, description and diff go to the configured command, off the UI thread,
//! and what it answers is kept as proposals for the reader (see `proposals`).
//! Nothing is posted. The text it is given and the running of the command are
//! `crate::agent`.

use std::path::PathBuf;

use super::{
    App,
    effect::{Read, TaskResult},
    store::{FetchKey, Notice, PrResource},
};
use crate::{
    agent::{self, AgentError, Material, MaterialError, Subject},
    domain::{
        pr::PrId,
        proposal_document::{self, Source},
    },
    local::proposals::{Batch, import},
    providers::{FetchError, Provider},
};

/// What the review is made from besides the PR: where the repository's rules
/// are, and the reader's own instructions, if they wrote any.
struct Sources {
    root: Option<PathBuf>,
    instructions: Option<PathBuf>,
}

/// How long leaving waits for the agent commands to end.
const STOP_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// What an asked-for review came to.
#[derive(Debug)]
pub struct Outcome {
    /// Who proposed: the command's program.
    pub agent: String,
    /// Comments added, not counting a summary.
    pub added: usize,
    pub duplicates: usize,
    /// Comments on a line that is not in the diff, which nothing could show.
    pub skipped: usize,
    /// Issues the PR closes that could not be read, so that the agent did not get
    /// what they asked for.
    pub issues_missed: usize,
    /// Whether the diff was too long to be given whole.
    pub diff_cut: bool,
}

/// Why it came to nothing.
#[derive(Debug, thiserror::Error)]
pub enum Failure {
    #[error(transparent)]
    Provider(#[from] FetchError),
    #[error(transparent)]
    Agent(#[from] AgentError),
    #[error("{0}")]
    Other(String),
}

impl From<MaterialError> for Failure {
    fn from(error: MaterialError) -> Self {
        match error {
            MaterialError::Provider(error) => Self::Provider(error),
            MaterialError::NoHead => Self::Other(error.to_string()),
        }
    }
}

impl Failure {
    /// What the reader is told, without what the server or the agent printed
    /// beyond its last line.
    pub fn user_message(&self) -> String {
        match self {
            Self::Provider(error) => error.user_message(),
            Self::Agent(error) => error.to_string(),
            Self::Other(message) => message.clone(),
        }
    }
}

impl App {
    /// Ask the configured agent to review the PR, unless one is already at it.
    pub(super) fn spawn_agent_review(&mut self, pr_id: PrId) {
        let store = &self.state.store;
        let command = store.agent_review.clone();
        let Some(subject) = subject_of(store, pr_id) else {
            return;
        };
        let Some(source) = &self.proposals_source else {
            self.state.store.errors.insert(
                pr_id,
                "There is nowhere to keep what the agent proposes.".into(),
            );
            return;
        };
        let place = (source.root.clone(), source.scope.clone());
        let sources = Sources {
            root: self.review_root.clone(),
            instructions: self.state.store.agent_review_instructions.clone(),
        };
        let Some(ticket) = self
            .state
            .store
            .begin_fetch(FetchKey::Pr(PrResource::AgentReview, pr_id))
        else {
            return;
        };
        // The read is registered under its key, and `apply_read` ends it.
        drop(ticket);
        let provider = self.provider.clone();
        let cancel = agent::Cancel::default();
        self.agent_reviews.insert(pr_id, cancel.clone());
        self.spawn_fetch(
            move || {
                review(
                    &provider, pr_id, &subject, &command, &place, &sources, &cancel,
                )
            },
            move |returned| {
                TaskResult::Read(Read::AgentReview(
                    pr_id,
                    returned.unwrap_or_else(|panic| Err(Failure::Other(panic.to_string()))),
                ))
            },
        );
    }

    /// Stop the review of the PR that is running; what it was going to say is not
    /// kept. The result that comes back says it was stopped.
    pub(super) fn stop_agent_review(&self, pr_id: PrId) {
        if let Some(cancel) = self.agent_reviews.get(&pr_id) {
            cancel.cancel();
        }
    }

    /// Leaving: stop every review that is running, and wait a moment for their
    /// processes to end, so that none goes on with nobody to read it.
    pub(super) fn stop_agent_reviews(&self) {
        for cancel in self.agent_reviews.values() {
            cancel.cancel();
        }
        if !self.agent_reviews.is_empty() && !agent::wait_until_stopped(STOP_WAIT) {
            tracing::warn!("an agent command was still running when slussa left");
        }
    }

    /// An asked-for review is over: its proposals are read from the file, or its
    /// failure is the PR's error.
    pub(super) fn agent_review_done(&mut self, pr_id: PrId, result: Result<Outcome, Failure>) {
        self.agent_reviews.remove(&pr_id);
        match result {
            // What the reader asked for is not an error on the PR.
            Err(Failure::Agent(AgentError::Cancelled { .. })) => {
                self.state.store.notice = Some(Notice::info(format!(
                    "PR #{pr_id} · the review was stopped"
                )));
            }
            Ok(outcome) => {
                tracing::info!(
                    "agent review: pr={pr_id} added={} duplicates={} skipped={} cut={}",
                    outcome.added,
                    outcome.duplicates,
                    outcome.skipped,
                    outcome.diff_cut
                );
                self.read_proposals();
                self.state.store.notice = Some(Notice::info(notice_text(pr_id, &outcome)));
            }
            Err(failure) => {
                tracing::warn!("agent review failed: pr={pr_id}: {failure}");
                self.state.store.errors.insert(
                    pr_id,
                    format!("The review failed: {}", failure.user_message()),
                );
            }
        }
    }
}

fn subject_of(store: &super::store::Store, pr_id: PrId) -> Option<Subject> {
    let pr = store.cache.prs.loaded()?.iter().find(|pr| pr.id == pr_id)?;
    // The list may leave the description out; it is read with the PR's info.
    let described = store
        .cache
        .details
        .get(&pr_id)
        .and_then(|data| data.info.loaded())
        .and_then(|info| info.description.clone())
        .or_else(|| pr.description.clone());
    let issues = store
        .cache
        .details
        .get(&pr_id)
        .and_then(|data| data.info.loaded())
        .map_or_else(Vec::new, |info| info.issues.clone());
    Some(Subject {
        title: pr.title.clone(),
        description: described,
        source_branch: pr.source_branch.clone(),
        target_branch: pr.target_branch.clone(),
        issues,
    })
}

fn notice_text(pr_id: PrId, outcome: &Outcome) -> String {
    let mut text = match (outcome.added, outcome.duplicates) {
        (0, 0) => format!("PR #{pr_id} · {} found nothing to propose", outcome.agent),
        (0, _) => format!("PR #{pr_id} · {} proposed nothing new", outcome.agent),
        (1, _) => format!("PR #{pr_id} · {} proposed 1 comment", outcome.agent),
        (added, _) => format!("PR #{pr_id} · {} proposed {added} comments", outcome.agent),
    };
    if outcome.skipped > 0 {
        text = format!("{text} ({} not on a line of the diff)", outcome.skipped);
    }
    if outcome.issues_missed > 0 {
        text = format!(
            "{text} · {} issue(s) could not be read",
            outcome.issues_missed
        );
    }
    if outcome.diff_cut {
        text.push_str(" · the diff was cut");
    }
    text
}

/// The whole review, blocking: read the diff, ask the agent, keep what it says.
fn review(
    provider: &Provider,
    pr_id: PrId,
    subject: &Subject,
    command: &[String],
    (root, scope): &(PathBuf, String),
    sources: &Sources,
    cancel: &agent::Cancel,
) -> Result<Outcome, Failure> {
    let material = Material::gather(provider, pr_id, subject, sources.root.as_deref())?;
    // The agent is given the diff of this commit, so what it proposes is of it,
    // whatever it writes for `head` itself.
    let head = &material.head;
    let agent = command.first().cloned().unwrap_or_default();
    let instructions = read_instructions(sources.instructions.as_deref())?;
    let prompt = agent::prompt(&material.request(subject, instructions.as_deref()));
    let answer = agent::run(command, &prompt.text, agent::TIMEOUT, cancel)?;
    let json = agent::find_json(&answer).ok_or_else(|| AgentError::NoJson {
        program: agent.clone(),
    })?;
    let parsed = proposal_document::parse(
        json.as_bytes(),
        Source::Asked {
            head,
            agent: &agent,
        },
    )
    .map_err(|reason| AgentError::Unusable {
        program: agent.clone(),
        reason,
    })?;
    let total = parsed.comments.len();
    let comments: Vec<_> = parsed
        .comments
        .into_iter()
        .filter(|comment| {
            material.diff.has_line(
                comment.path(),
                comment.line(),
                comment.side() == crate::domain::proposal::Side::Old,
            )
        })
        .collect();
    let skipped = total - comments.len();
    let imported = import(
        root,
        scope,
        pr_id,
        Batch {
            comments,
            summary: parsed.summary,
        },
    )
    .map_err(|e| Failure::Other(format!("The proposals could not be kept: {e}")))?;
    Ok(Outcome {
        agent,
        added: imported.comments,
        duplicates: imported.duplicates,
        skipped,
        issues_missed: material.issues_missed,
        diff_cut: prompt.diff_cut,
    })
}

/// The reader's own instructions, when they named a file. One that cannot be read
/// is an error and not a silent fall back to the built-in ones: they asked for it.
fn read_instructions(path: Option<&std::path::Path>) -> Result<Option<String>, Failure> {
    let Some(path) = path else {
        return Ok(None);
    };
    std::fs::read_to_string(path).map(Some).map_err(|e| {
        Failure::Other(format!(
            "The instructions in {} could not be read: {e}",
            path.display()
        ))
    })
}

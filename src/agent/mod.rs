//! Asking an agent for a review: the text it is given, running the configured
//! command, and finding the answer in what it printed. What it proposes is read
//! by `domain::proposal_document` and kept for the reader; nothing is posted.
//!
//! The command runs in slussa's working directory with the text on its standard
//! input, off the UI thread and with a deadline. What it is given is the PR's
//! title, description and diff, which another party wrote: the text tells the
//! agent to treat all of it as data, and what the agent answers is only ever a
//! proposal the reader may discard.

mod prompt;
mod run;

pub use prompt::{ReviewRequest, RulesFile, find_json, prompt};
pub use run::{AgentError, Cancel, TIMEOUT, run, wait_until_stopped};

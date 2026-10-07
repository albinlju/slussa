//! End-to-end flows through the real `App`, provider and fetchers, with a fake
//! `gh` underneath. These cover what the unit tests inject by hand: requests
//! really leave, results really return, and the store reacts to them.
//!
//! Each test holds the installed fake until its work has settled, because the
//! fake replaces `gh` for the whole process.

#![expect(
    clippy::significant_drop_tightening,
    reason = "the fake is held across awaits on purpose: dropped early, the next test would replace `gh` mid-flow"
)]

mod agent_review;
mod closed_groups;
mod open_group;
mod pr_info;
mod support;
mod writes;

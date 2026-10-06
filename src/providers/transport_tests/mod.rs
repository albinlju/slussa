//! Provider behaviour at the transport: what is sent to `gh` and to Bitbucket,
//! and how answers and failures come back. Everything runs through `Provider`
//! against `FakeGh` or `MockHttp`; nothing touches a real service.

#![expect(
    clippy::significant_drop_tightening,
    reason = "the fake is held for the whole test on purpose: dropped early, another test would replace `gh` mid-call"
)]

mod bitbucket;
mod bitbucket_reviews;
mod github_diffs;
mod github_merges;
mod github_one_pr;
mod github_reads;
mod github_writes;
mod merge_status;
mod support;

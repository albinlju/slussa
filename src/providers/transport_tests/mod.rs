//! Provider behaviour at the transport: what is sent to `gh` and to Bitbucket,
//! and how answers and failures come back. Everything runs through `Provider`
//! against `FakeGh` or `MockHttp`; nothing touches a real service.

// The fake `gh` guard is held for the whole test on purpose; dropping it early
// would let another test replace `gh` mid-call.
#![allow(clippy::significant_drop_tightening)]

mod bitbucket;
mod github_reads;
mod github_writes;
mod merge_status;
mod support;

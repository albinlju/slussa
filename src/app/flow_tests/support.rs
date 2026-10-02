//! Imports and helpers shared by the files in this directory.

pub(super) use crate::app::{
    App,
    action::{Action, Command, Effect, ListAction},
    reviews::CommentTarget,
    store::{FetchKey, LoadState, OpenChain},
};
pub(super) use crate::{
    domain::pr::{PrGroup, PrId},
    providers::Provider,
    test_support::{FakeGh, gh_closed_pr, gh_list_page, gh_pr},
    tui::screens::pr_list::StatusFilter,
};
pub(super) use serde_json::json;
pub(super) use std::time::Duration;

pub(super) fn app() -> App {
    App::new(
        crate::session::Session::for_test(Provider::GitHub, "me"),
        crate::app::drafts::Drafts::Nowhere,
    )
}

/// Apply provider results until nothing is in flight.
pub(super) async fn settle(app: &mut App) {
    while !app.state.store.fetches.is_empty() || !app.state.store.operations.is_empty() {
        let action = tokio::time::timeout(Duration::from_secs(10), app.results_rx.recv())
            .await
            .expect("a provider result within 10 seconds")
            .expect("the action channel stays open");
        app.apply_result(action);
    }
}

pub(super) fn one_pr_page() -> String {
    gh_list_page(&[gh_pr(1, "2026-09-01T10:00:00Z")], None)
}

/// Each group of PRs is a read of its own: the open ones, the merged ones and
/// the declined ones. A view reads only the groups it shows.
pub(super) const OPEN_QUERY: &str = "states: OPEN";

pub(super) const MERGED_QUERY: &str = "states: MERGED";

pub(super) const DECLINED_QUERY: &str = "states: CLOSED";

/// How many reads of the open, merged and declined groups were made.
pub(super) fn list_reads(calls: &[String]) -> (usize, usize, usize) {
    let count = |needle: &str| calls.iter().filter(|call| call.contains(needle)).count();
    (
        count(OPEN_QUERY),
        count(MERGED_QUERY),
        count(DECLINED_QUERY),
    )
}

/// An answer shaped like every connection the refetch touches, so each read
/// gets something parseable without modelling GitHub.
pub(super) fn any_connection() -> String {
    let connection = json!({"nodes": [], "pageInfo": {"hasNextPage": false, "endCursor": null}});
    json!({"data": {
        "repository": {
            "connection": connection,
            "item": {"connection": connection},
            "pullRequest": {"mergeable": "MERGEABLE", "id": "PR_1", "body": null, "labels": connection}
        },
        "item": {"connection": connection}
    }})
    .to_string()
}

pub(super) fn loaded_ids(app: &App) -> Vec<u64> {
    match &app.state.store.cache.prs {
        LoadState::Loaded(prs) => prs.iter().map(|pr| pr.id.0).collect(),
        other => panic!("PR list is not loaded: {other:?}"),
    }
}

pub(super) fn merged(id: u64, at: &str) -> serde_json::Value {
    gh_closed_pr(id, at, "MERGED")
}

//! The description and labels are read when a PR is opened.

use super::support::*;

fn info_reads(calls: &[String]) -> usize {
    calls
        .iter()
        .filter(|call| call.contains("pullRequest(number: $pr) { id body"))
        .count()
}

fn info_answer() -> String {
    json!({"data": {"repository": {"pullRequest": {
        "id": "PR_1",
        "body": "Why this change.",
        "labels": {"nodes": [{"name": "bug"}], "pageInfo": {"hasNextPage": false}}
    }}}})
    .to_string()
}

#[tokio::test]
async fn opening_a_pr_reads_its_description_and_labels_once() {
    let gh = FakeGh::new()
        .on(OPEN_QUERY, &one_pr_page())
        .on("pullRequest(number: $pr) { id body", &info_answer())
        .on("graphql", &any_connection())
        .install();
    let mut app = app();
    app.spawn_load_prs(PrGroup::Open, None);
    settle(&mut app).await;
    assert_eq!(loaded_ids(&app), vec![1]);
    assert_eq!(info_reads(&gh.calls()), 0, "the list does not read them");

    app.apply(Action::List(ListAction::OpenPr(1)));
    app.apply(Action::List(ListAction::OpenPr(1)));
    settle(&mut app).await;

    let info = match &app.state.store.cache.details[&1].info {
        LoadState::Loaded(info) => info.clone(),
        other => panic!("expected the info to be loaded, got {other:?}"),
    };
    assert_eq!(info.description.as_deref(), Some("Why this change."));
    assert_eq!(info.labels, vec!["bug"]);
    assert_eq!(info_reads(&gh.calls()), 1, "{:?}", gh.calls());
}

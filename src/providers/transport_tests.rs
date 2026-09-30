//! Provider behaviour at the transport: what is sent to `gh` and to Bitbucket,
//! and how answers and failures come back. Everything runs through `Provider`
//! against `FakeGh` or `MockHttp`; nothing touches a real service.

// The fake `gh` guard is held for the whole test on purpose; dropping it early
// would let another test replace `gh` mid-call.
#![allow(clippy::significant_drop_tightening)]

use serde_json::{Value, json};

use super::{
    FetchError, Provider,
    bitbucket_dc::{Config, RepoLocation},
};
use crate::{
    domain::{
        diff::DiffRevision,
        pr::{MergeStatus, MergeStrategy, Mergeability, PrGroup, PrStatus},
        review::{ReviewComment, ReviewVerdict, ReviewerState},
    },
    test_support::{FakeGh, InstalledGh, MockHttp, Route, gh_closed_pr, gh_list_page, gh_pr},
};

// ---------------------------------------------------------------------------
// GitHub through a fake `gh`
// ---------------------------------------------------------------------------

fn fetch_group_with(
    gh: FakeGh,
    group: PrGroup,
    after: Option<&str>,
) -> (Result<crate::domain::pr::PrBatch, FetchError>, InstalledGh) {
    let installed = gh.install();
    (Provider::GitHub.fetch_prs(group, after), installed)
}

fn fetch_prs_with(gh: FakeGh) -> (Result<crate::domain::pr::PrBatch, FetchError>, InstalledGh) {
    fetch_group_with(gh, PrGroup::Open, None)
}

#[test]
fn github_open_group_is_read_a_page_at_a_time_with_drafts_and_nothing_closed() {
    let mut draft = gh_pr(2, "2026-09-03T10:00:00Z");
    draft["isDraft"] = json!(true);
    let gh = FakeGh::new()
        .on(
            "states: OPEN, first: 30, after: null",
            &gh_list_page(&[gh_pr(1, "2026-09-01T10:00:00Z")], Some("c1")),
        )
        .on(
            "states: OPEN, first: 30, after: \"c1\"",
            &gh_list_page(&[draft], None),
        )
        .install();

    let first = Provider::GitHub.fetch_prs(PrGroup::Open, None).unwrap();
    assert_eq!(first.more.as_deref(), Some("c1"), "the caller reads on");
    assert_eq!(
        first.prs.iter().map(|pr| pr.id).collect::<Vec<_>>(),
        vec![1]
    );
    let pr = &first.prs[0];
    assert_eq!(pr.status, PrStatus::Open);
    assert_eq!(pr.author.username, "alice");
    assert!(
        pr.labels.is_empty() && pr.description.is_none(),
        "the list leaves out the description and the labels"
    );
    assert_eq!(
        pr.comment_count, 6,
        "four conversation comments and two threads on code"
    );
    assert_eq!(pr.reviewers.len(), 1);

    let last = Provider::GitHub
        .fetch_prs(PrGroup::Open, Some("c1"))
        .unwrap();
    assert_eq!(last.more, None);
    assert_eq!(last.prs[0].id, 2);
    assert_eq!(
        last.prs[0].status,
        PrStatus::Draft,
        "drafts come with the open ones"
    );

    let calls = gh.calls();
    assert_eq!(calls.len(), 2, "{calls:?}");
    assert!(calls[0].starts_with("api graphql -F owner={owner} -F name={repo} -f query="));
    assert!(calls[0].contains("pullRequests(states: OPEN, first: 30, after: null)"));
    assert!(calls[1].contains("pullRequests(states: OPEN, first: 30, after: \"c1\")"));
    assert!(
        calls
            .iter()
            .all(|call| !call.contains("MERGED") && !call.contains("CLOSED")),
        "no closed PRs are asked for: {calls:?}"
    );
}

#[test]
fn github_closed_groups_read_one_page_each_with_their_own_state() {
    for (group, state, args) in [
        (
            PrGroup::Merged,
            "MERGED",
            "states: MERGED, orderBy: {field: UPDATED_AT, direction: DESC}, first: 30, after: null",
        ),
        (
            PrGroup::Declined,
            "CLOSED",
            "states: CLOSED, orderBy: {field: UPDATED_AT, direction: DESC}, first: 30, after: null",
        ),
    ] {
        // More exist, but reading the group must not follow that cursor.
        let gh = FakeGh::new().on(
            "after: null",
            &gh_list_page(&[gh_closed_pr(3, "2026-08-01T10:00:00Z", state)], Some("x")),
        );
        let (result, installed) = fetch_group_with(gh, group, None);
        let batch = result.unwrap();

        assert_eq!(batch.prs.len(), 1);
        assert_eq!(PrGroup::of(&batch.prs[0].status), group, "{group:?}");
        assert_eq!(batch.more.as_deref(), Some("x"));
        let calls = installed.calls();
        assert_eq!(calls.len(), 1, "{calls:?}");
        assert!(calls[0].contains(args), "{group:?}: {}", calls[0]);
    }
}

#[test]
fn github_a_closed_pr_still_flagged_as_a_draft_is_not_shown_as_one() {
    // GitHub keeps the draft flag on a closed PR. Seen on a real repository.
    for (state, expected) in [("CLOSED", PrStatus::Declined), ("MERGED", PrStatus::Merged)] {
        let mut pr = gh_closed_pr(3, "2026-08-01T10:00:00Z", state);
        pr["isDraft"] = json!(true);
        let gh = FakeGh::new().on("states:", &gh_list_page(&[pr], None));
        let group = PrGroup::of(&expected);
        let (result, _installed) = fetch_group_with(gh, group, None);

        assert_eq!(result.unwrap().prs[0].status, expected, "{state}");
    }
}

#[test]
fn github_a_failed_read_is_an_error_for_that_group() {
    for group in PrGroup::ALL {
        let gh = FakeGh::new().fail("states:", 1, "gh: HTTP 502");
        let (result, installed) = fetch_group_with(gh, group, None);

        assert!(
            matches!(result, Err(FetchError::GhFailed { .. })),
            "{group:?}: {result:?}"
        );
        assert_eq!(installed.calls().len(), 1);
    }
}

#[test]
fn github_review_requests_become_pending_reviewers_and_replace_an_earlier_review() {
    let mut node = gh_pr(1, "2026-09-01T10:00:00Z");
    node["latestReviews"]["nodes"] = json!([
        {"state": "APPROVED", "author": {"login": "me"}},
        {"state": "APPROVED", "author": {"login": "bob"}}
    ]);
    // `me` is asked again; the team request and the null have no login.
    node["reviewRequests"] = json!({"nodes": [
        {"requestedReviewer": {"login": "me"}},
        {"requestedReviewer": {}},
        {"requestedReviewer": null}
    ]});
    let (result, installed) = fetch_prs_with(
        FakeGh::new()
            .on("states: OPEN", &gh_list_page(&[node], None))
            .on("states: [MERGED, CLOSED]", &gh_list_page(&[], None)),
    );

    let reviewers = &result.unwrap().prs[0].reviewers;
    let summary: Vec<_> = reviewers
        .iter()
        .map(|r| (r.author.username.as_str(), r.state.clone()))
        .collect();
    assert_eq!(
        summary,
        [
            ("bob", ReviewerState::Approved),
            ("me", ReviewerState::Requested)
        ]
    );
    assert!(installed.calls()[0].contains("reviewRequests(first: 100)"));
}

#[test]
fn github_list_query_leaves_out_the_body_and_the_labels() {
    let gh = FakeGh::new()
        .on(
            "states: OPEN",
            &gh_list_page(&[gh_pr(1, "2026-09-01T10:00:00Z")], None),
        )
        .install();
    Provider::GitHub.fetch_prs(PrGroup::Open, None).unwrap();

    let calls = gh.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(
        !calls[0].contains("body") && !calls[0].contains("labels"),
        "{calls:?}"
    );
}

fn info_answer(labels: &[&str], more: bool) -> String {
    json!({"data": {"repository": {"pullRequest": {
        "id": "PR_1",
        "body": "Explains the change.",
        "labels": {
            "nodes": labels.iter().map(|name| json!({"name": name})).collect::<Vec<_>>(),
            "pageInfo": {"hasNextPage": more}
        }
    }}}})
    .to_string()
}

#[test]
fn github_info_reads_the_description_and_the_labels_of_one_pr() {
    let gh = FakeGh::new()
        .on("pullRequest(number: $pr)", &info_answer(&["bug"], false))
        .install();
    let info = Provider::GitHub.fetch_info(7).unwrap();

    assert_eq!(info.description.as_deref(), Some("Explains the change."));
    assert_eq!(info.labels, vec!["bug"]);
    assert_eq!(gh.calls().len(), 1);
    assert!(gh.calls()[0].contains("pr=7"));
}

#[test]
fn github_info_reads_on_when_the_labels_are_truncated() {
    let node_page = json!({"data": {"item": {"connection": {
        "nodes": [{"name": "bug"}, {"name": "ux"}],
        "pageInfo": {"hasNextPage": false, "endCursor": null}
    }}}})
    .to_string();
    let gh = FakeGh::new()
        .on("node(id: \"PR_1\")", &node_page)
        .on("pullRequest(number: $pr)", &info_answer(&["bug"], true))
        .install();
    let info = Provider::GitHub.fetch_info(1).unwrap();

    assert_eq!(info.labels, vec!["bug", "ux"]);
    assert_eq!(gh.calls().len(), 2);
}

#[test]
fn only_github_reads_the_description_and_labels_apart_from_the_list() {
    use crate::domain::capabilities::Feature;
    assert!(Provider::GitHub.capabilities().supports(Feature::PrInfo));
}

#[test]
fn github_failure_carries_the_exit_code_and_stderr() {
    let gh = FakeGh::new().fail("graphql", 1, "gh: HTTP 401: Bad credentials");
    let (result, _installed) = fetch_prs_with(gh);

    match result.unwrap_err() {
        FetchError::GhFailed { code, stderr } => {
            assert_eq!(code, Some(1));
            assert!(stderr.contains("Bad credentials"), "{stderr}");
        }
        other => panic!("expected GhFailed, got {other:?}"),
    }
}

#[test]
fn github_reports_a_missing_gh() {
    let _installed = InstalledGh::missing();
    assert!(matches!(
        Provider::GitHub.fetch_prs(PrGroup::Open, None),
        Err(FetchError::GhMissing)
    ));
}

#[test]
fn github_rejects_unparseable_output_and_graphql_errors() {
    // One fake at a time: the first guard must be gone before the second install.
    let (result, installed) = fetch_prs_with(FakeGh::new().on("graphql", "not json"));
    drop(installed);
    assert!(
        matches!(result, Err(FetchError::ParseFailed(_))),
        "{result:?}"
    );

    let errors = json!({"errors": [{"message": "rate limited"}]}).to_string();
    let (result, _installed) = fetch_prs_with(FakeGh::new().on("graphql", &errors));
    assert!(
        matches!(result, Err(FetchError::InvalidInput(_))),
        "{result:?}"
    );
}

#[test]
fn github_merge_sends_the_chosen_strategy() {
    let installed = FakeGh::new().on("api", "{}").install();
    Provider::GitHub.merge(7, MergeStrategy::Squash).unwrap();

    assert_eq!(
        installed.calls(),
        vec!["api --method PUT repos/{owner}/{repo}/pulls/7/merge -f merge_method=squash"]
    );
}

#[test]
fn github_decline_closes_the_pull_request() {
    let installed = FakeGh::new().on("api", "{}").install();
    Provider::GitHub.decline(7).unwrap();

    assert_eq!(
        installed.calls(),
        vec!["api --method PATCH repos/{owner}/{repo}/pulls/7 -f state=closed"]
    );
}

#[test]
fn github_pr_comment_passes_the_body_as_a_literal_argument() {
    let installed = FakeGh::new().on("api", "{}").install();
    let body = "thanks $(whoami) `id` \"quoted\" & more";
    Provider::GitHub.post_pr_comment(7, body).unwrap();

    assert_eq!(
        installed.calls(),
        vec![format!(
            "api --method POST repos/{{owner}}/{{repo}}/issues/7/comments -f body={body}"
        )]
    );
}

fn revision(head: &str) -> DiffRevision {
    DiffRevision {
        head: head.into(),
        base: Some("base".into()),
        commit: false,
    }
}

fn review_comment(head: Option<&str>, line: usize, removed: bool) -> ReviewComment {
    ReviewComment {
        revision: head.map(revision),
        path: "src/lib.rs".into(),
        line,
        removed,
        body: format!("note on line {line}"),
    }
}

#[test]
fn github_batched_review_is_one_call_with_the_comments_on_stdin() {
    let installed = FakeGh::new().on("api", "{}").install();
    let comments = [
        review_comment(Some("abc"), 3, false),
        review_comment(Some("abc"), 9, true),
    ];
    Provider::GitHub
        .submit_full_review(
            7,
            ReviewVerdict::RequestChanges,
            "please fix",
            "me",
            &comments,
        )
        .unwrap();

    assert_eq!(
        installed.calls(),
        vec!["api --method POST repos/{owner}/{repo}/pulls/7/reviews --input -"]
    );
    let sent: Value = serde_json::from_str(&installed.stdin()).unwrap();
    assert_eq!(
        sent,
        json!({
            "commit_id": "abc",
            "event": "REQUEST_CHANGES",
            "body": "please fix",
            "comments": [
                {"path": "src/lib.rs", "line": 3, "side": "RIGHT", "body": "note on line 3"},
                {"path": "src/lib.rs", "line": 9, "side": "LEFT", "body": "note on line 9"}
            ]
        })
    );
}

#[test]
fn github_batched_review_refuses_unsafe_batches_without_calling_gh() {
    let installed = FakeGh::new().on("api", "{}").install();
    let mixed = [
        review_comment(Some("abc"), 1, false),
        review_comment(Some("def"), 2, false),
    ];
    let unknown = [review_comment(None, 1, false)];
    for comments in [&mixed[..], &unknown[..]] {
        let result =
            Provider::GitHub.submit_full_review(7, ReviewVerdict::Comment, "", "me", comments);
        assert!(
            matches!(result, Err(FetchError::InvalidInput(_))),
            "{result:?}"
        );
    }
    assert!(installed.calls().is_empty(), "{:?}", installed.calls());
}

// ---------------------------------------------------------------------------
// Bitbucket Data Center through a loopback HTTP server
// ---------------------------------------------------------------------------

const PR_BASE: &str = "/rest/api/1.0/projects/PROJ/repos/repo/pull-requests";

/// The open PRs, `start` entries in.
fn open_url(start: u64) -> String {
    format!("{PR_BASE}?state=OPEN&limit=50&start={start}")
}

/// The one page of merged or declined PRs the list reads.
fn closed_url(state: &str) -> String {
    format!("{PR_BASE}?state={state}&limit=25&start=0")
}

/// The merged or declined page that starts `start` entries in.
fn closed_url_at(state: &str, start: u64) -> String {
    format!("{PR_BASE}?state={state}&limit=25&start={start}")
}

fn last_page(values: &[Value]) -> String {
    json!({"values": values, "isLastPage": true}).to_string()
}

fn bitbucket(server: &MockHttp) -> Provider {
    Provider::BitbucketDc(Config {
        repo: RepoLocation {
            base_url: server.base_url(),
            project_key: "PROJ".into(),
            repo_slug: "repo".into(),
        },
        pat: "secret-token".into(),
    })
}

fn bb_pr(id: u64) -> Value {
    json!({
        "id": id,
        "title": format!("Change {id}"),
        "state": "OPEN",
        "createdDate": 1_790_000_000_000_i64,
        "updatedDate": 1_790_000_100_000_i64,
        "fromRef": {"displayId": "feature"},
        "toRef": {"displayId": "main"},
        "author": {"user": {"name": "alice"}}
    })
}

#[test]
fn bitbucket_open_group_reads_every_page_with_drafts_and_nothing_closed() {
    let page_one = json!({"values": [bb_pr(1)], "isLastPage": false, "nextPageStart": 7});
    let mut draft = bb_pr(2);
    draft["draft"] = json!(true);
    let server = MockHttp::start(vec![
        Route::get(&open_url(0), 200, &page_one.to_string()),
        Route::get(&open_url(7), 200, &last_page(&[draft])),
    ]);
    let batch = bitbucket(&server).fetch_prs(PrGroup::Open, None).unwrap();

    assert_eq!(batch.more, None, "the open group is read in full");
    assert_eq!(
        batch.prs.iter().map(|pr| pr.id).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(batch.prs[0].source_branch, "feature");
    assert_eq!(batch.prs[1].status, PrStatus::Draft);
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    for request in &requests {
        assert!(request.target.contains("state=OPEN"), "{}", request.target);
        assert_eq!(request.headers["authorization"], "Bearer secret-token");
        assert!(request.headers["user-agent"].starts_with("tuipr/"));
    }
}

#[test]
fn bitbucket_closed_groups_read_one_page_at_a_time_from_their_offset() {
    for (group, state, status) in [
        (PrGroup::Merged, "MERGED", PrStatus::Merged),
        (PrGroup::Declined, "DECLINED", PrStatus::Declined),
    ] {
        let mut first = bb_pr(3);
        first["state"] = json!(state);
        // A closed PR can still carry the draft flag.
        first["draft"] = json!(true);
        let mut second = bb_pr(4);
        second["state"] = json!(state);
        let server = MockHttp::start(vec![
            Route::get(
                &closed_url(state),
                200,
                &json!({"values": [first], "isLastPage": false, "nextPageStart": 25}).to_string(),
            ),
            Route::get(&closed_url_at(state, 25), 200, &last_page(&[second])),
        ]);
        let provider = bitbucket(&server);

        let page = provider.fetch_prs(group, None).unwrap();
        assert_eq!(page.prs[0].status, status, "a closed draft is not a draft");
        assert_eq!(page.more.as_deref(), Some("25"));
        assert_eq!(
            server.requests().len(),
            1,
            "the next page is not followed on its own"
        );

        let next = provider.fetch_prs(group, Some("25")).unwrap();
        assert_eq!(next.prs.iter().map(|pr| pr.id).collect::<Vec<_>>(), vec![4]);
        assert_eq!(next.more, None, "the end is reported");
        assert!(
            server
                .requests()
                .iter()
                .all(|request| request.target.contains(&format!("state={state}"))),
            "each closed group reads only its own state"
        );
    }
}

#[test]
fn bitbucket_rejected_token_is_reported_as_not_authenticated() {
    let server = MockHttp::start(vec![Route::get(&open_url(0), 401, "{}")]);
    let error = bitbucket(&server)
        .fetch_prs(PrGroup::Open, None)
        .unwrap_err();

    match error {
        FetchError::NotAuthenticated { host } => assert_eq!(host, server.host()),
        other => panic!("expected NotAuthenticated, got {other:?}"),
    }
}

#[test]
fn bitbucket_server_error_keeps_status_and_body() {
    let body = json!({"errors": [{"message": "Repository is being migrated"}]}).to_string();
    let server = MockHttp::start(vec![Route::get(&open_url(0), 500, &body)]);
    let error = bitbucket(&server)
        .fetch_prs(PrGroup::Open, None)
        .unwrap_err();

    match &error {
        FetchError::HttpFailed { status, body } => {
            assert_eq!(*status, 500);
            assert!(body.contains("migrated"));
        }
        other => panic!("expected HttpFailed, got {other:?}"),
    }
    assert!(
        error
            .user_message()
            .contains("Repository is being migrated")
    );
}

#[test]
fn bitbucket_unreachable_server_is_a_network_error() {
    let server = MockHttp::start(Vec::new());
    let provider = bitbucket(&server);
    drop(server);

    assert!(matches!(
        provider.fetch_prs(PrGroup::Open, None),
        Err(FetchError::Network(_))
    ));
}

#[test]
fn bitbucket_rejects_a_non_advancing_page_cursor() {
    let stuck = json!({"values": [bb_pr(1)], "isLastPage": false, "nextPageStart": 0});
    let server = MockHttp::start(vec![Route::get(&open_url(0), 200, &stuck.to_string())]);

    assert!(matches!(
        bitbucket(&server).fetch_prs(PrGroup::Open, None),
        Err(FetchError::ParseFailed(_))
    ));
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn bitbucket_pr_comment_posts_json_to_the_comments_endpoint() {
    let target = "/rest/api/1.0/projects/PROJ/repos/repo/pull-requests/9/comments";
    let server = MockHttp::start(vec![Route::post(target, 201, "{}")]);
    bitbucket(&server).post_pr_comment(9, "looks good").unwrap();

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].target, target);
    assert_eq!(requests[0].headers["authorization"], "Bearer secret-token");
    let sent: Value = serde_json::from_str(&requests[0].body).unwrap();
    assert_eq!(sent, json!({"text": "looks good"}));
}

const PR_9: &str = "/rest/api/1.0/projects/PROJ/repos/repo/pull-requests/9";

#[test]
fn bitbucket_review_posts_comments_then_summary_then_the_verdict() {
    let comments_target = format!("{PR_9}/comments");
    let server = MockHttp::start(vec![
        Route::post(&comments_target, 201, "{}"),
        Route::put(&format!("{PR_9}/participants/me"), 200, "{}"),
    ]);
    let comments = [
        review_comment(Some("abc"), 3, false),
        review_comment(Some("abc"), 9, true),
    ];
    bitbucket(&server)
        .submit_full_review(9, ReviewVerdict::Approve, "ship it", "me", &comments)
        .unwrap();

    let requests = server.requests();
    let order: Vec<_> = requests
        .iter()
        .map(|r| format!("{} {}", r.method, r.target.rsplit('/').next().unwrap()))
        .collect();
    assert_eq!(
        order,
        ["POST comments", "POST comments", "POST comments", "PUT me"]
    );
    let first: Value = serde_json::from_str(&requests[0].body).unwrap();
    assert_eq!(first["anchor"]["lineType"], "ADDED");
    assert_eq!(first["anchor"]["toHash"], "abc");
    let second: Value = serde_json::from_str(&requests[1].body).unwrap();
    assert_eq!(second["anchor"]["lineType"], "REMOVED");
    let summary: Value = serde_json::from_str(&requests[2].body).unwrap();
    assert_eq!(summary, json!({"text": "ship it"}));
    let verdict: Value = serde_json::from_str(&requests[3].body).unwrap();
    assert_eq!(verdict, json!({"status": "APPROVED"}));
}

#[test]
fn bitbucket_review_reports_how_many_comments_landed_before_a_failure() {
    let target = format!("{PR_9}/comments");
    let server = MockHttp::start(vec![
        Route::post(&target, 201, "{}").times(1),
        Route::post(
            &target,
            500,
            &json!({"errors": [{"message": "boom"}]}).to_string(),
        ),
    ]);
    let comments = [
        review_comment(Some("abc"), 3, false),
        review_comment(Some("abc"), 4, false),
        review_comment(Some("abc"), 5, false),
    ];
    let error = bitbucket(&server)
        .submit_full_review(9, ReviewVerdict::Approve, "ship it", "me", &comments)
        .unwrap_err();

    match error {
        FetchError::PartialReview {
            posted_comments,
            summary_posted,
            source,
        } => {
            assert_eq!(posted_comments, 1);
            assert!(!summary_posted);
            assert!(matches!(
                *source,
                FetchError::HttpFailed { status: 500, .. }
            ));
        }
        other => panic!("expected PartialReview, got {other:?}"),
    }
    let requests = server.requests();
    assert_eq!(requests.len(), 2, "stops at the first failure");
    assert!(
        requests.iter().all(|r| r.method == "POST"),
        "no verdict is sent"
    );
}

#[test]
fn bitbucket_reviewers_without_a_verdict_are_pending_requests() {
    let mut pr = bb_pr(1);
    pr["reviewers"] = json!([
        {"user": {"name": "me"}, "status": "UNAPPROVED"},
        {"user": {"name": "bob"}, "status": "APPROVED"},
        {"user": {"name": "carol"}, "status": "NEEDS_WORK"}
    ]);
    let page = json!({"values": [pr], "isLastPage": true});
    let server = MockHttp::start(vec![Route::get(&open_url(0), 200, &page.to_string())]);
    let prs = bitbucket(&server)
        .fetch_prs(PrGroup::Open, None)
        .unwrap()
        .prs;

    let states: Vec<_> = prs[0].reviewers.iter().map(|r| r.state.clone()).collect();
    assert_eq!(
        states,
        [
            ReviewerState::Requested,
            ReviewerState::Approved,
            ReviewerState::ChangesRequested
        ]
    );
}

// ---------------------------------------------------------------------------
// Merge status: what stands in the way of merging
// ---------------------------------------------------------------------------

fn gh_merge_fields(mergeable: &str, state: &str, decision: Option<&str>) -> String {
    json!({"data": {"repository": {"pullRequest": {
        "mergeable": mergeable,
        "mergeStateStatus": state,
        "reviewDecision": decision
    }}}})
    .to_string()
}

#[test]
fn github_merge_status_reads_the_state_and_explains_a_block() {
    let cases: [(&str, &str, Option<&str>, Mergeability, &str); 9] = [
        (
            "MERGEABLE",
            "CLEAN",
            Some("APPROVED"),
            Mergeability::Mergeable,
            "",
        ),
        ("MERGEABLE", "UNSTABLE", None, Mergeability::Mergeable, ""),
        (
            "CONFLICTING",
            "DIRTY",
            None,
            Mergeability::Conflicts,
            "conflicts with the base",
        ),
        (
            "MERGEABLE",
            "BEHIND",
            None,
            Mergeability::Blocked,
            "behind its base",
        ),
        ("MERGEABLE", "DRAFT", None, Mergeability::Blocked, "draft"),
        (
            "MERGEABLE",
            "BLOCKED",
            Some("REVIEW_REQUIRED"),
            Mergeability::Blocked,
            "approving review",
        ),
        (
            "MERGEABLE",
            "BLOCKED",
            Some("CHANGES_REQUESTED"),
            Mergeability::Blocked,
            "requested changes",
        ),
        (
            "MERGEABLE",
            "BLOCKED",
            Some("APPROVED"),
            Mergeability::Blocked,
            "Required checks",
        ),
        ("UNKNOWN", "UNKNOWN", None, Mergeability::Unknown, ""),
    ];
    for (mergeable, state, decision, expected, reason) in cases {
        let installed = FakeGh::new()
            .on("graphql", &gh_merge_fields(mergeable, state, decision))
            .install();
        let status = Provider::GitHub.fetch_mergeability(7).unwrap();

        assert_eq!(status.state, expected, "{mergeable} {state} {decision:?}");
        if reason.is_empty() {
            assert!(status.blockers.is_empty(), "{:?}", status.blockers);
        } else {
            assert!(
                status.blockers.iter().any(|b| b.contains(reason)),
                "{state} {decision:?}: {:?}",
                status.blockers
            );
        }
        let calls = installed.calls();
        assert!(
            calls[0].contains("mergeStateStatus reviewDecision"),
            "{calls:?}"
        );
        drop(installed);
    }
}

fn merge_url(pr: u64) -> String {
    format!("{PR_BASE}/{pr}/merge")
}

#[test]
fn bitbucket_merge_checks_become_blockers_with_the_servers_words() {
    let blocked = json!({"canMerge": false, "conflicted": false, "vetoes": [
        {"summaryMessage": "Not all required builds are successful yet", "detailedMessage": "x"},
        {"summaryMessage": "At least 1 approval is required", "detailedMessage": "y"}
    ]});
    let server = MockHttp::start(vec![Route::get(&merge_url(9), 200, &blocked.to_string())]);
    let status = bitbucket(&server).fetch_mergeability(9).unwrap();

    assert_eq!(
        status,
        MergeStatus::with(
            Mergeability::Blocked,
            vec![
                "Not all required builds are successful yet".into(),
                "At least 1 approval is required".into()
            ]
        )
    );
}

#[test]
fn bitbucket_merge_status_covers_conflict_clean_and_a_veto_without_words() {
    let cases = [
        (
            json!({"canMerge": false, "conflicted": true, "vetoes": []}),
            Mergeability::Conflicts,
            "merge conflicts",
        ),
        (
            json!({"canMerge": false, "conflicted": false}),
            Mergeability::Blocked,
            "Merge checks have not passed",
        ),
        (
            json!({"canMerge": true, "conflicted": false}),
            Mergeability::Mergeable,
            "",
        ),
    ];
    for (body, expected, reason) in cases {
        let server = MockHttp::start(vec![Route::get(&merge_url(9), 200, &body.to_string())]);
        let status = bitbucket(&server).fetch_mergeability(9).unwrap();

        assert_eq!(status.state, expected, "{body}");
        assert_eq!(
            status.blockers.iter().any(|b| b.contains(reason)),
            !reason.is_empty(),
            "{body}: {:?}",
            status.blockers
        );
    }
}

#[test]
fn bitbucket_refused_merge_reports_the_vetoes() {
    let refusal = json!({"errors": [{
        "message": "Merging is vetoed",
        "exceptionName": "PullRequestMergeVetoedException",
        "vetoes": [
            {"summaryMessage": "Not all required builds are successful yet"},
            {"summaryMessage": "At least 1 approval is required"}
        ]
    }]});
    let server = MockHttp::start(vec![
        Route::get(
            &format!("{PR_BASE}/9"),
            200,
            &json!({"version": 3}).to_string(),
        ),
        Route::post(
            &format!("{}?version=3", merge_url(9)),
            409,
            &refusal.to_string(),
        ),
    ]);
    let error = bitbucket(&server)
        .merge(9, MergeStrategy::Merge)
        .unwrap_err();

    assert_eq!(
        error.user_message(),
        "Merging is vetoed: Not all required builds are successful yet; At least 1 approval is required"
    );
}

// ---------------------------------------------------------------------------
// Older merged and declined PRs, read on request
// ---------------------------------------------------------------------------

#[test]
fn github_older_prs_continue_from_the_cursor_and_report_when_they_end() {
    let gh = FakeGh::new()
        .on(
            "after: \"x\"",
            &gh_list_page(
                &[gh_closed_pr(9, "2026-08-01T10:00:00Z", "CLOSED")],
                Some("y"),
            ),
        )
        .on(
            "after: \"y\"",
            &gh_list_page(&[gh_closed_pr(8, "2026-07-01T10:00:00Z", "CLOSED")], None),
        )
        .install();

    let first = Provider::GitHub
        .fetch_prs(PrGroup::Declined, Some("x"))
        .unwrap();
    assert_eq!(
        first.prs.iter().map(|pr| pr.id).collect::<Vec<_>>(),
        vec![9]
    );
    assert_eq!(first.prs[0].status, PrStatus::Declined);
    assert_eq!(first.more.as_deref(), Some("y"));

    let last = Provider::GitHub
        .fetch_prs(PrGroup::Declined, Some("y"))
        .unwrap();
    assert_eq!(last.prs.iter().map(|pr| pr.id).collect::<Vec<_>>(), vec![8]);
    assert_eq!(last.more, None, "the end is reported, not guessed");

    let calls = gh.calls();
    assert_eq!(calls.len(), 2, "{calls:?}");
    for call in &calls {
        assert!(call.contains("states: CLOSED"), "{call}");
        assert!(call.contains("first: 30"), "{call}");
        assert!(
            !call.contains("states: OPEN"),
            "older reads skip open PRs: {call}"
        );
    }
}

#[test]
fn bitbucket_rejects_an_unreadable_older_position_without_a_request() {
    let server = MockHttp::start(Vec::new());
    for position in ["", "x", "-1", "2.5", "25|10"] {
        let result = bitbucket(&server).fetch_prs(PrGroup::Merged, Some(position));
        assert!(
            matches!(result, Err(FetchError::InvalidInput(_))),
            "{position:?}: {result:?}"
        );
    }
    assert!(server.requests().is_empty());
}

// ---------------------------------------------------------------------------
// Reopening a closed PR
// ---------------------------------------------------------------------------

#[test]
fn github_reopen_sets_the_state_back_to_open() {
    let installed = FakeGh::new().on("api", "{}").install();
    Provider::GitHub.reopen(7).unwrap();

    assert_eq!(
        installed.calls(),
        vec!["api --method PATCH repos/{owner}/{repo}/pulls/7 -f state=open"]
    );
}

#[test]
fn github_refusing_a_reopen_reaches_the_user_in_githubs_words() {
    let _installed = FakeGh::new()
        .fail(
            "api",
            1,
            "gh: Validation Failed: the head branch was deleted (HTTP 422)",
        )
        .install();
    let error = Provider::GitHub.reopen(7).unwrap_err();

    assert!(matches!(error, FetchError::GhFailed { .. }), "{error:?}");
    assert!(
        error.user_message().contains("head branch was deleted"),
        "{}",
        error.user_message()
    );
}

#[test]
fn bitbucket_reopen_reads_the_version_then_posts_it() {
    let server = MockHttp::start(vec![
        Route::get(
            &format!("{PR_BASE}/9"),
            200,
            &json!({"version": 3}).to_string(),
        ),
        Route::post(&format!("{PR_BASE}/9/reopen?version=3"), 200, "{}"),
    ]);
    bitbucket(&server).reopen(9).unwrap();

    let requests = server.requests();
    let sent: Vec<_> = requests
        .iter()
        .map(|r| format!("{} {}", r.method, r.target))
        .collect();
    assert_eq!(
        sent,
        [
            format!("GET {PR_BASE}/9"),
            format!("POST {PR_BASE}/9/reopen?version=3")
        ]
    );
    assert_eq!(requests[1].headers["authorization"], "Bearer secret-token");
}

#[test]
fn bitbucket_refusing_a_reopen_shows_the_servers_reason() {
    let refusal = json!({"errors": [{"message": "Only declined pull requests can be reopened"}]});
    let server = MockHttp::start(vec![
        Route::get(
            &format!("{PR_BASE}/9"),
            200,
            &json!({"version": 3}).to_string(),
        ),
        Route::post(
            &format!("{PR_BASE}/9/reopen?version=3"),
            409,
            &refusal.to_string(),
        ),
    ]);
    let error = bitbucket(&server).reopen(9).unwrap_err();

    assert_eq!(
        error.user_message(),
        "Only declined pull requests can be reopened"
    );
}

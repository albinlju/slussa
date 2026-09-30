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
        pr::MergeStrategy,
        review::{ReviewComment, ReviewVerdict, ReviewerState},
    },
    test_support::{FakeGh, InstalledGh, MockHttp, Route, gh_list_page, gh_pr},
};

// ---------------------------------------------------------------------------
// GitHub through a fake `gh`
// ---------------------------------------------------------------------------

fn fetch_prs_with(
    gh: FakeGh,
) -> (
    Result<Vec<crate::domain::pr::PullRequest>, FetchError>,
    InstalledGh,
) {
    let installed = gh.install();
    (Provider::GitHub.fetch_prs(), installed)
}

#[test]
fn github_list_follows_cursors_and_lists_newest_first() {
    let gh = FakeGh::new()
        .on(
            "after: null",
            &gh_list_page(&[gh_pr(1, "2026-09-01T10:00:00Z")], Some("c1")),
        )
        .on(
            "after: \"c1\"",
            &gh_list_page(&[gh_pr(2, "2026-09-03T10:00:00Z")], None),
        );
    let (result, installed) = fetch_prs_with(gh);
    let prs = result.unwrap();

    assert_eq!(prs.iter().map(|pr| pr.id).collect::<Vec<_>>(), vec![2, 1]);
    assert_eq!(prs[1].author.username, "alice");
    assert_eq!(prs[1].labels, vec!["bug"]);
    assert_eq!(prs[1].comment_count, 4);
    assert_eq!(prs[1].reviewers.len(), 1);

    let calls = installed.calls();
    assert_eq!(calls.len(), 2, "{calls:?}");
    assert!(calls[0].starts_with("api graphql -F owner={owner} -F name={repo} -f query="));
    assert!(calls[0].contains("pullRequests(first: 100, after: null)"));
    assert!(calls[1].contains("after: \"c1\""));
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
    let (result, installed) =
        fetch_prs_with(FakeGh::new().on("pullRequests(", &gh_list_page(&[node], None)));

    let reviewers = &result.unwrap()[0].reviewers;
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
fn github_list_refetches_labels_when_the_embedded_page_is_truncated() {
    let mut truncated = gh_pr(1, "2026-09-01T10:00:00Z");
    truncated["labels"] = json!({
        "nodes": [{"name": "bug"}],
        "pageInfo": {"hasNextPage": true}
    });
    let node_page = json!({"data": {"item": {"connection": {
        "nodes": [{"name": "bug"}, {"name": "ux"}],
        "pageInfo": {"hasNextPage": false, "endCursor": null}
    }}}})
    .to_string();
    let gh = FakeGh::new()
        .on("pullRequests(", &gh_list_page(&[truncated], None))
        .on("node(id: \"PR_1\")", &node_page);
    let (result, installed) = fetch_prs_with(gh);

    assert_eq!(result.unwrap()[0].labels, vec!["bug", "ux"]);
    assert_eq!(installed.calls().len(), 2);
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
        Provider::GitHub.fetch_prs(),
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

const PR_LIST: &str = "/rest/api/1.0/projects/PROJ/repos/repo/pull-requests?state=ALL&limit=50";

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
fn bitbucket_list_follows_server_offsets_and_sends_the_token() {
    let page_one = json!({"values": [bb_pr(1)], "isLastPage": false, "nextPageStart": 7});
    let page_two = json!({"values": [bb_pr(2)], "isLastPage": true});
    let server = MockHttp::start(vec![
        Route::get(&format!("{PR_LIST}&start=0"), 200, &page_one.to_string()),
        Route::get(&format!("{PR_LIST}&start=7"), 200, &page_two.to_string()),
    ]);
    let prs = bitbucket(&server).fetch_prs().unwrap();

    assert_eq!(prs.iter().map(|pr| pr.id).collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(prs[0].source_branch, "feature");
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    for request in &requests {
        assert_eq!(request.headers["authorization"], "Bearer secret-token");
        assert!(request.headers["user-agent"].starts_with("tuipr/"));
    }
}

#[test]
fn bitbucket_rejected_token_is_reported_as_not_authenticated() {
    let server = MockHttp::start(vec![Route::get(&format!("{PR_LIST}&start=0"), 401, "{}")]);
    let error = bitbucket(&server).fetch_prs().unwrap_err();

    match error {
        FetchError::NotAuthenticated { host } => assert_eq!(host, server.host()),
        other => panic!("expected NotAuthenticated, got {other:?}"),
    }
}

#[test]
fn bitbucket_server_error_keeps_status_and_body() {
    let body = json!({"errors": [{"message": "Repository is being migrated"}]}).to_string();
    let server = MockHttp::start(vec![Route::get(&format!("{PR_LIST}&start=0"), 500, &body)]);
    let error = bitbucket(&server).fetch_prs().unwrap_err();

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

    assert!(matches!(provider.fetch_prs(), Err(FetchError::Network(_))));
}

#[test]
fn bitbucket_rejects_a_non_advancing_page_cursor() {
    let stuck = json!({"values": [bb_pr(1)], "isLastPage": false, "nextPageStart": 0});
    let server = MockHttp::start(vec![Route::get(
        &format!("{PR_LIST}&start=0"),
        200,
        &stuck.to_string(),
    )]);

    assert!(matches!(
        bitbucket(&server).fetch_prs(),
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
    let server = MockHttp::start(vec![Route::get(
        &format!("{PR_LIST}&start=0"),
        200,
        &page.to_string(),
    )]);
    let prs = bitbucket(&server).fetch_prs().unwrap();

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

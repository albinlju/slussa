//! Bitbucket Data Center through a loopback HTTP server.

use super::support::*;

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
        batch.prs.iter().map(|pr| pr.id.0).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(batch.prs[0].source_branch, "feature");
    assert_eq!(batch.prs[1].status, PrStatus::Draft);
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    for request in &requests {
        assert!(request.target.contains("state=OPEN"), "{}", request.target);
        assert_eq!(request.headers["authorization"], "Bearer secret-token");
        assert!(request.headers["user-agent"].starts_with("slussa/"));
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
        assert_eq!(
            next.prs.iter().map(|pr| pr.id.0).collect::<Vec<_>>(),
            vec![4]
        );
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
fn bitbucket_a_refused_action_is_not_reported_as_a_missing_login() {
    let body = json!({"errors": [{"message": "You are not permitted to merge this pull request"}]})
        .to_string();
    let server = MockHttp::start(vec![Route::get(&open_url(0), 403, &body)]);
    let error = bitbucket(&server)
        .fetch_prs(PrGroup::Open, None)
        .unwrap_err();

    assert!(
        matches!(error, FetchError::HttpFailed { status: 403, .. }),
        "{error:?}"
    );
    assert_eq!(
        error.user_message(),
        "You are not permitted to merge this pull request"
    );

    let server = MockHttp::start(vec![Route::get(&open_url(0), 403, "")]);
    let message = bitbucket(&server)
        .fetch_prs(PrGroup::Open, None)
        .unwrap_err()
        .user_message();
    assert!(message.contains("permissions"), "{message}");
    assert!(!message.contains("auth login"), "{message}");
}

#[test]
fn bitbucket_asking_who_is_logged_in_with_a_rejected_token_says_so() {
    let server = MockHttp::start(vec![Route::get(
        "/rest/api/1.0/application-properties",
        401,
        "",
    )]);
    let error = bitbucket(&server).current_user().unwrap_err();

    assert!(
        matches!(&error, FetchError::NotAuthenticated { host } if *host == server.host()),
        "{error:?}"
    );
    assert!(error.user_message().contains("slussa auth login"));
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
    bitbucket(&server)
        .post_pr_comment(PrId(9), "looks good")
        .unwrap();

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
        review_comment("abc", 3, false),
        review_comment("abc", 9, true),
    ];
    bitbucket(&server)
        .submit_full_review(PrId(9), ReviewVerdict::Approve, "ship it", "me", &comments)
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
        review_comment("abc", 3, false),
        review_comment("abc", 4, false),
        review_comment("abc", 5, false),
    ];
    let error = bitbucket(&server)
        .submit_full_review(PrId(9), ReviewVerdict::Approve, "ship it", "me", &comments)
        .unwrap_err();

    match error {
        ReviewError::Partial {
            posted_comments,
            summary_posted,
            source,
        } => {
            assert_eq!(posted_comments, 1);
            assert!(!summary_posted);
            assert!(matches!(source, FetchError::HttpFailed { status: 500, .. }));
        }
        other @ ReviewError::Failed(_) => panic!("expected a partial review, got {other:?}"),
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
    bitbucket(&server).reopen(PrId(9)).unwrap();

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
    let error = bitbucket(&server).reopen(PrId(9)).unwrap_err();

    assert_eq!(
        error.user_message(),
        "Only declined pull requests can be reopened"
    );
}

#[test]
fn bitbucket_token_is_sent_as_a_bearer_and_never_printed() {
    let server = MockHttp::start(vec![Route::get(
        &format!("{PR_BASE}/9/merge"),
        200,
        &json!({"canMerge": true, "conflicted": false}).to_string(),
    )]);
    let provider = bitbucket(&server);
    provider.fetch_mergeability(PrId(9)).unwrap();
    assert_eq!(
        server.requests()[0].headers["authorization"],
        "Bearer secret-token"
    );

    let printed = format!("{provider:?}");
    assert!(!printed.contains("secret-token"), "{printed}");
    assert!(printed.contains("redacted"), "{printed}");
}

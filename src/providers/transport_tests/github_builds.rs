//! GitHub builds through a fake `gh`: which checks have a log, and reading it.

use super::support::*;
use crate::domain::{build_log::LogKind, ci::JobId};

#[test]
fn only_a_github_actions_check_has_a_log_to_open() {
    let checks = json!([{"check_runs": [
        {"id": 99, "app": {"slug": "github-actions"}, "name": "test",
         "status": "completed", "conclusion": "failure"},
        {"id": 100, "app": {"slug": "codecov"}, "name": "coverage",
         "status": "completed", "conclusion": "failure"},
        {"id": 101, "app": null, "name": "legacy",
         "status": "completed", "conclusion": "success"},
    ]}]);
    let _installed = FakeGh::new()
        .on("check-runs", &checks.to_string())
        .on(
            "/status",
            &json!([{"statuses": [{"context": "ci/other", "state": "failure"}]}]).to_string(),
        )
        .on("pulls/7", "abc123\n")
        .install();
    let builds = Provider::github_for_test().fetch_builds(PrId(7)).unwrap();
    let logs: Vec<_> = builds.iter().map(|build| build.log).collect();
    assert_eq!(logs, [Some(JobId(99)), None, None, None]);
}

#[test]
fn a_log_is_read_from_its_job_and_kept_as_lines() {
    let text = "2024-05-01T10:00:00.0000000Z ##[group]Run tests\n\
2024-05-01T10:00:01.0000000Z boom\n\
2024-05-01T10:00:02.0000000Z ##[error]exit 1\n";
    let installed = FakeGh::new().on("actions/jobs/99/logs", text).install();
    let log = Provider::github_for_test()
        .fetch_build_log(JobId(99))
        .unwrap();
    assert_eq!(log.errors, [2]);
    assert_eq!(
        log.lines.first().map(|line| line.kind),
        Some(LogKind::Group)
    );
    assert!(
        installed
            .calls()
            .iter()
            .any(|call| call.contains("repos/{owner}/{repo}/actions/jobs/99/logs")),
        "{:?}",
        installed.calls()
    );
}

#[test]
fn a_log_that_cannot_be_read_is_an_error() {
    let _installed = FakeGh::new()
        .fail("actions/jobs/99/logs", 1, "gh: HTTP 404: Not Found")
        .install();
    let result = Provider::github_for_test().fetch_build_log(JobId(99));
    assert!(
        matches!(result, Err(FetchError::GhFailed { .. })),
        "{result:?}"
    );
}

#[test]
fn bitbucket_has_no_log_to_read() {
    let server = MockHttp::start(vec![]);
    let result = bitbucket(&server).fetch_build_log(JobId(1));
    assert!(
        matches!(result, Err(FetchError::Unsupported(_))),
        "{result:?}"
    );
}

#[test]
fn a_log_is_read_with_an_older_gh_that_has_no_flag_for_escape_sequences() {
    let installed = FakeGh::new()
        .fail(
            "--allow-escape-sequences",
            1,
            "unknown flag: --allow-escape-sequences",
        )
        .on("actions/jobs/99/logs", "##[error]boom\n")
        .install();
    let log = Provider::github_for_test()
        .fetch_build_log(JobId(99))
        .unwrap();
    assert_eq!(log.errors, [0]);
    assert_eq!(installed.calls().len(), 2, "{:?}", installed.calls());
}

#[test]
fn a_log_is_read_with_the_flag_that_lets_gh_print_escape_sequences() {
    let installed = FakeGh::new()
        .on("--allow-escape-sequences", "\u{1b}[31m##[error]boom\n")
        .install();
    let log = Provider::github_for_test()
        .fetch_build_log(JobId(99))
        .unwrap();
    assert_eq!(log.errors, [0]);
    assert_eq!(installed.calls().len(), 1);
}

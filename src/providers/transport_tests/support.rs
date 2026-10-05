//! Imports and helpers shared by the files in this directory.

pub(super) use crate::providers::{
    FetchError, MergeError, Provider, ReviewError,
    bitbucket_dc::{Config, RepoLocation},
};
pub(super) use crate::{
    domain::{
        diff::{DiffRevision, LineRef},
        pr::{MergeStrategy, Mergeability, PrGroup, PrId, PrStatus},
        review::{ReviewComment, ReviewVerdict, ReviewedHead, ReviewerState},
    },
    test_support::{FakeGh, InstalledGh, MockHttp, Route, gh_closed_pr, gh_list_page, gh_pr},
};
pub(super) use serde_json::{Value, json};

/// The commit the reader has seen, in the tests that send a merge or a verdict.
pub(super) const HEAD: &str = "abc123";

pub(super) fn head_of(sha: &str) -> ReviewedHead {
    ReviewedHead::of(None, Some(sha)).expect("a head was listed")
}

pub(super) fn read_head() -> ReviewedHead {
    head_of(HEAD)
}

pub(super) fn revision(head: &str) -> DiffRevision {
    DiffRevision {
        head: head.into(),
        base: Some("base".into()),
        commit: false,
    }
}

pub(super) fn review_comment(head: &str, line: usize, removed: bool) -> ReviewComment {
    ReviewComment {
        revision: revision(head),
        path: "src/lib.rs".into(),
        line: if removed {
            LineRef::Old(line)
        } else {
            LineRef::New(line)
        },
        body: format!("note on line {line}"),
    }
}

pub(super) const PR_BASE: &str = "/rest/api/1.0/projects/PROJ/repos/repo/pull-requests";

pub(super) fn bitbucket(server: &MockHttp) -> Provider {
    Provider::BitbucketDc(Config {
        repo: RepoLocation {
            base_url: server.base_url(),
            project_key: "PROJ".into(),
            repo_slug: "repo".into(),
        },
        pat: crate::providers::bitbucket_dc::auth::Pat::new("secret-token".into()),
    })
}

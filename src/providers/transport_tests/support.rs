//! Imports and helpers shared by the files in this directory.

pub(super) use crate::providers::{
    FetchError, Provider, ReviewError,
    bitbucket_dc::{Config, RepoLocation},
};
pub(super) use crate::{
    domain::{
        diff::{DiffRevision, LineRef},
        pr::{MergeStrategy, Mergeability, PrGroup, PrStatus},
        review::{ReviewComment, ReviewVerdict, ReviewerState},
    },
    test_support::{FakeGh, InstalledGh, MockHttp, Route, gh_closed_pr, gh_list_page, gh_pr},
};
pub(super) use serde_json::{Value, json};

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

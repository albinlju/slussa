//! A reader who had one head open and finds the branch at another, and the
//! results that arrive for them.

pub(super) use super::super::support::*;
pub(super) use crate::{
    domain::{
        commit::CommitOid,
        diff::{Diff, DiffRange, DiffRevision, FileDiff},
    },
    tui::app::store::{FetchKey, PrResource},
};
use ratatui::{Terminal, backend::TestBackend};

pub(super) const MARK: &str = "new since you read it";
pub(super) const WHOLE: FetchKey = FetchKey::Pr(PrResource::Diff, PrId(42));

pub(super) fn oid(text: &str) -> CommitOid {
    CommitOid::parse(text).expect("a commit id")
}

pub(super) fn range(base: &str, head: &str) -> DiffRange {
    DiffRange {
        base: oid(base),
        head: oid(head),
    }
}

/// The diff the PR screen holds was made from this head.
pub(super) fn diff_of(app: &mut App, head: &str) {
    if let Some(data) = app.state.store.cache.details.get_mut(&PrId(42)) {
        data.diff = LoadState::Loaded(Diff {
            revision: Some(DiffRevision {
                head: head.into(),
                base: Some("0ba5e0".into()),
                commit: false,
            }),
            files: vec![],
        });
    }
}

/// The list says the branch is at `head` now.
pub(super) fn branch_at(app: &mut App, head: &str) {
    if let LoadState::Loaded(prs) = &mut app.state.store.cache.prs {
        for pr in prs.iter_mut().filter(|pr| pr.id == PrId(42)) {
            pr.head_oid = Some(head.into());
        }
    }
}

pub(super) fn screen_text(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(140, 30)).unwrap();
    terminal
        .draw(|frame| ui::render(frame, &mut app.state))
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

pub(super) fn read_head(app: &App) -> Option<&CommitOid> {
    app.state.store.seen.read_head(PrId(42))
}

/// A reader who had `read` open last time, and finds the branch at `now`.
pub(super) fn returning_reader(read: &str, now: &str) -> App {
    let mut app = app();
    branch_at(&mut app, read);
    diff_of(&mut app, read);
    detail(&mut app, DetailTab::Diff);
    assert_eq!(read_head(&app), Some(&oid(read)), "the diff was opened");
    press(&mut app, KeyCode::Esc);
    branch_at(&mut app, now);
    detail(&mut app, DetailTab::Overview);
    app
}

/// The Diff tab is where the reader is; the whole diff on it is of `head`.
pub(super) fn diff_of_files(app: &mut App, head: &str, paths: &[&str]) {
    if let Some(data) = app.state.store.cache.details.get_mut(&PrId(42)) {
        data.diff = LoadState::Loaded(Diff {
            revision: Some(DiffRevision {
                head: head.into(),
                base: Some("0ba5e0".into()),
                commit: false,
            }),
            files: paths
                .iter()
                .map(|path| FileDiff {
                    path: (*path).into(),
                    hunks: vec![],
                })
                .collect(),
        });
    }
}

pub(super) fn files(paths: &[&str]) -> Vec<FileDiff> {
    paths
        .iter()
        .map(|path| FileDiff {
            path: (*path).into(),
            hunks: vec![],
        })
        .collect()
}

/// The PR's diff is read again, and is of `head`.
pub(super) fn diff_arrives(app: &mut App, head: &str, paths: &[&str]) {
    app.apply_result(TaskResult::Read(Read::Diff(
        PrId(42),
        Ok(Diff {
            revision: Some(DiffRevision {
                head: head.into(),
                base: Some("0ba5e0".into()),
                commit: false,
            }),
            files: files(paths),
        }),
    )));
}

/// The compare of what is new answers with these files.
pub(super) fn compare_arrives(app: &mut App, paths: &[&str]) {
    app.apply_result(TaskResult::Read(Read::RangeDiff(
        PrId(42),
        range("aaa111", "bbb222"),
        Ok(Diff {
            revision: None,
            files: files(paths),
        }),
    )));
}

pub(super) fn what_is_new(app: &App) -> &LoadState<Diff> {
    &app.state.store.cache.details[&PrId(42)].range_diffs[&range("aaa111", "bbb222")]
}

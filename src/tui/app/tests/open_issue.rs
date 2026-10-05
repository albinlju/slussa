//! Choosing which issue to open when the PR closes several: `i`.

use super::support::*;
use crate::domain::pr::{LinkedIssue, PrInfo};

fn closing(app: &mut App, issues: &[(u64, &str)]) {
    if let Some(data) = app.state.store.cache.details.get_mut(&PrId(42)) {
        data.info = LoadState::Loaded(PrInfo {
            description: None,
            labels: vec![],
            issues: issues
                .iter()
                .map(|(number, url)| LinkedIssue {
                    number: *number,
                    title: format!("Issue {number}"),
                    url: Some((*url).to_owned()),
                })
                .collect(),
        });
    }
}

fn enter(app: &mut App) -> Option<Effect> {
    app.state.ui.update(
        Action::Detail(DetailAction::Issues(IssueAction::Select)),
        &app.state.store,
        app.state.screen,
    )
}

#[tokio::test]
async fn with_several_issues_i_opens_a_picker_and_enter_opens_the_one_chosen() {
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    closing(
        &mut app,
        &[
            (12, "https://example.com/o/r/issues/12"),
            (31, "https://example.com/o/other/issues/31"),
        ],
    );
    press(&mut app, KeyCode::Char('i'));
    assert!(app.state.ui.detail.issue_picker().is_some());
    press(&mut app, KeyCode::Char('j'));

    let Some(Effect::IssueLink { number, url }) = enter(&mut app) else {
        panic!("Enter opens the issue chosen");
    };
    assert_eq!(number, 31);
    assert_eq!(url, "https://example.com/o/other/issues/31");
    assert!(
        app.state.ui.detail.issue_picker().is_none(),
        "the picker closes"
    );
}

#[tokio::test]
async fn esc_closes_the_picker_without_opening_anything() {
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    closing(
        &mut app,
        &[
            (12, "https://example.com/o/r/issues/12"),
            (31, "https://example.com/o/r/issues/31"),
        ],
    );
    press(&mut app, KeyCode::Char('i'));
    press(&mut app, KeyCode::Esc);
    assert!(app.state.ui.detail.issue_picker().is_none());
    assert!(!app.state.store.link_pending);
}

#[tokio::test]
async fn with_one_issue_i_opens_no_picker() {
    let mut app = app();
    detail(&mut app, DetailTab::Overview);
    closing(&mut app, &[(12, "file:///not-http")]);
    press(&mut app, KeyCode::Char('i'));
    assert!(app.state.ui.detail.issue_picker().is_none());
    // The address is refused before anything is started.
    let notice = app.state.store.notice.as_ref().map(|n| n.message.as_str());
    assert_eq!(notice, Some("issue #12: There is no valid HTTP(S) link."));
}

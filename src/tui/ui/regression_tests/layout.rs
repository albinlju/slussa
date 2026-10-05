//! What each screen shows at a given size, and what a missing feature hides.

use super::support::*;

#[test]
fn screens_preserve_rendered_output() {
    let mut output = String::new();
    for (width, height) in [(100, 30), (40, 12)] {
        for screen in
            std::iter::once(Screen::List).chain(DetailTab::ALL.map(|tab| Screen::Detail {
                pr_id: PrId(42),
                tab,
            }))
        {
            let mut state = fixture();
            state.screen = screen;
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| render(frame, &mut state)).unwrap();
            writeln!(output, "{width}x{height} {screen:?}").unwrap();
            let buffer = terminal.backend().buffer();
            for y in 0..height {
                let mut row = String::new();
                for x in 0..width {
                    row.push_str(buffer[(x, y)].symbol());
                }
                output.push_str(row.trim_end());
                output.push('\n');
            }
        }
    }
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/tui/ui/testdata/screens.txt"
    );
    if std::env::var_os("SLUSSA_UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(path, &output).unwrap();
    }
    assert_eq!(output, std::fs::read_to_string(path).unwrap());
}

#[test]
fn unsupported_features_are_hidden_from_content_footer_and_help() {
    use crate::domain::capabilities::{Capabilities, Feature};
    let mut state = fixture();
    state.screen = Screen::Detail {
        pr_id: PrId(42),
        tab: DetailTab::Overview,
    };
    state.store.capabilities = Capabilities::default();
    for help_open in [false, true] {
        state.ui.detail.overlay = help_open.then(|| Overlay::Help(HelpDialog::default()));
        let mut terminal = Terminal::new(TestBackend::new(150, 50)).unwrap();
        terminal.draw(|frame| render(frame, &mut state)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        for hidden in [
            "Builds",
            "mergeable",
            "c: comment",
            "a: verdict",
            "v: review",
            "m: merge",
            "x: decline",
            "quick verdict",
            "start/finish review",
            "resolve thread",
            "edit own",
            "delete own",
            "1-5",
        ] {
            assert!(
                !text.contains(hidden),
                "unexpected {hidden} with help={help_open}"
            );
        }
        assert!(text.contains("Overview"));
        assert!(text.contains("Reviewers"));
        if help_open {
            assert!(text.contains("1-4"));
        }
    }
    state.ui.detail.overlay = None;
    state.store.capabilities.features.insert(Feature::Builds);
    let mut terminal = Terminal::new(TestBackend::new(150, 50)).unwrap();
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    assert!(text.contains("Builds"));
    assert!(!text.contains("a: verdict"));
}

#[test]
fn compact_list_keeps_title_and_help_visible() {
    for width in [40, 70, 100, 140] {
        let mut state = fixture();
        let mut terminal = Terminal::new(TestBackend::new(width, 12)).unwrap();
        terminal.draw(|f| render(f, &mut state)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(
            text.contains("Component migration"),
            "title lost at {width}"
        );
        assert!(text.contains("?: help"), "help lost at {width}");
        assert!(!text.contains("donate"));
    }
}

#[test]
fn help_scroll_reaches_last_action_in_small_terminal() {
    let mut state = fixture();
    state.screen = Screen::Detail {
        pr_id: PrId(42),
        tab: DetailTab::Overview,
    };
    state.ui.detail.overlay = Some(Overlay::Help(HelpDialog::default()));
    let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
    terminal.draw(|f| render(f, &mut state)).unwrap();
    for _ in 0..8 {
        let action = key(&state, KeyCode::PageDown);
        state.ui.update(action, &state.store, state.screen);
    }
    terminal.draw(|f| render(f, &mut state)).unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    assert!(text.contains("quit"));
    assert!(text.contains("esc: close"));
    assert_eq!(
        state.screen,
        Screen::Detail {
            pr_id: PrId(42),
            tab: DetailTab::Overview
        }
    );
}

#[test]
fn compact_detail_tabs_always_show_the_active_tab() {
    for tab in DetailTab::ALL {
        let mut state = fixture();
        state.screen = Screen::Detail {
            pr_id: PrId(42),
            tab,
        };
        let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
        terminal.draw(|f| render(f, &mut state)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(
            text.contains(&format!("{}   1-5: tabs", tab.label())),
            "{tab:?}"
        );
    }
}

#[test]
fn notice_replaces_entire_footer_and_normal_hints_return_afterward() {
    for screen in [
        Screen::List,
        Screen::Detail {
            pr_id: PrId(42),
            tab: DetailTab::Overview,
        },
    ] {
        let mut state = fixture();
        state.screen = screen;
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal.draw(|f| render(f, &mut state)).unwrap();
        state.store.notice = Some(crate::tui::app::store::Notice::info(
            "PR #42: link copied".into(),
        ));
        terminal.draw(|f| render(f, &mut state)).unwrap();
        let footer: String = (0..100)
            .map(|x| terminal.backend().buffer()[(x, 29)].symbol())
            .collect();
        assert_eq!(footer.trim(), "PR #42: link copied");
        state.store.notice = None;
        terminal.draw(|f| render(f, &mut state)).unwrap();
        let footer: String = (0..100)
            .map(|x| terminal.backend().buffer()[(x, 29)].symbol())
            .collect();
        assert!(footer.contains("?: help"));
        assert!(!footer.contains("link copied"));
    }
}

#[test]
fn empty_searches_offer_recovery_in_list_files_and_commits() {
    for screen in [
        Screen::List,
        Screen::Detail {
            pr_id: PrId(42),
            tab: DetailTab::Diff,
        },
        Screen::Detail {
            pr_id: PrId(42),
            tab: DetailTab::Commits,
        },
    ] {
        let mut state = fixture();
        state.screen = screen;
        state.ui.list.search.query = "absent".into();
        state.ui.detail.diff.tree_search.query = "absent".into();
        state.ui.detail.commits.search.query = "absent".into();
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        terminal.draw(|f| render(f, &mut state)).unwrap();
        let text = rendered_text(&terminal);
        assert!(text.contains("No matching"));
        assert!(text.contains("Esc"));
        assert!(
            key_to_action(
                &state,
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)
            )
            .is_none()
        );
    }
}

#[test]
fn builds_scroll_to_last_check_in_a_short_terminal() {
    use crate::domain::ci::{Build, BuildState};
    let mut state = fixture();
    state.screen = Screen::Detail {
        pr_id: PrId(42),
        tab: DetailTab::Builds,
    };
    state.store.cache.details.get_mut(&PrId(42)).unwrap().builds = LoadState::Loaded(
        (0..30)
            .map(|i| Build {
                name: format!("check-{i:02}"),
                state: BuildState::Successful,
                duration_ms: None,
            })
            .collect(),
    );
    let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
    terminal.draw(|f| render(f, &mut state)).unwrap();
    for _ in 0..40 {
        local_key(&mut state, KeyCode::Char('j'));
    }
    terminal.draw(|f| render(f, &mut state)).unwrap();
    assert!(rendered_text(&terminal).contains("check-29"));
}

#[test]
fn commit_list_returns_to_the_same_viewport_after_opening_a_commit() {
    let mut state = fixture();
    state.screen = Screen::Detail {
        pr_id: PrId(42),
        tab: DetailTab::Commits,
    };
    state
        .store
        .cache
        .details
        .get_mut(&PrId(42))
        .unwrap()
        .commits = LoadState::Loaded(
        (0..40)
            .map(|n| Commit {
                oid: CommitOid(format!("{n:07x}")),
                headline: format!("Commit number {n}"),
                message: format!("Commit number {n}"),
                account: AccountKind::Person,
                authorship: Authorship::Human,
                author_name: "alice".into(),
                authored_at: chrono::Utc::now(),
                additions: 1,
                deletions: 0,
            })
            .collect(),
    );
    let mut terminal = Terminal::new(TestBackend::new(60, 16)).unwrap();
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    for _ in 0..25 {
        local_key(&mut state, KeyCode::Char('j'));
        terminal.draw(|frame| render(frame, &mut state)).unwrap();
    }
    for _ in 0..3 {
        local_key(&mut state, KeyCode::Char('k'));
        terminal.draw(|frame| render(frame, &mut state)).unwrap();
    }
    let before = rendered_text(&terminal);
    local_key(&mut state, KeyCode::Enter);
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    assert!(state.ui.detail.commits.open_commit().is_some());
    local_key(&mut state, KeyCode::Esc);
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    assert_eq!(rendered_text(&terminal), before);
}

#[test]
fn wide_description_can_pan_to_hidden_content_and_resets_when_resized() {
    let mut state = fixture();
    state.screen = Screen::Detail {
        pr_id: PrId(42),
        tab: DetailTab::Description,
    };
    if let Some(data) = state.store.cache.details.get_mut(&PrId(42)) {
        data.info = LoadState::Loaded(PrInfo {
            description: Some(format!("```\n{}END_OF_CODE\n```", "x".repeat(100))),
            labels: vec![],
            issues: vec![],
        });
    }
    let mut terminal = Terminal::new(TestBackend::new(40, 16)).unwrap();
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    assert!(rendered_text(&terminal).contains("H/L: pan"));
    assert!(!rendered_text(&terminal).contains("END_OF_CODE"));
    for _ in 0..20 {
        local_key(&mut state, KeyCode::Char('L'));
    }
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    assert!(rendered_text(&terminal).contains("END_OF_CODE"));
    assert_eq!(
        state.screen,
        Screen::Detail {
            pr_id: PrId(42),
            tab: DetailTab::Description
        }
    );
    let mut wide = Terminal::new(TestBackend::new(160, 16)).unwrap();
    wide.draw(|frame| render(frame, &mut state)).unwrap();
    assert_eq!(state.ui.detail.description.horizontal, 0);
    assert!(!rendered_text(&wide).contains("H/L: pan"));
}

#[test]
fn framed_panel_titles_start_under_the_tab_labels() {
    // The column each row's text starts in, after the outer frame.
    fn first_columns(state: &mut AppState, width: u16, height: u16) -> Vec<(String, usize)> {
        let text = draw(state, width, height);
        let chars: Vec<char> = text.chars().collect();
        chars
            .chunks(width as usize)
            .map(|row| {
                let start = row
                    .iter()
                    .skip(1)
                    .position(|c| *c != ' ')
                    .map_or(0, |at| at + 1);
                (row.iter().skip(start).collect::<String>(), start)
            })
            .collect()
    }
    for (width, height) in [(100, 30), (40, 12)] {
        let mut diff = pr_on(DetailTab::Diff, Activity::default());
        let mut commit = pr_on(DetailTab::Commits, Activity::default());
        local_key(&mut commit, KeyCode::Enter);
        let oid = commit.ui.detail.commits.open_commit().unwrap().clone();
        let data = commit.store.cache.details.get_mut(&PrId(42)).unwrap();
        let commit_diff = LoadState::Loaded(data.diff.loaded().unwrap().clone());
        data.commit_diffs.insert(oid, commit_diff);
        for state in [&mut diff, &mut commit] {
            let rows = first_columns(state, width, height);
            let tabs = rows
                .iter()
                .find(
                    |(row, _)| // A narrow tab bar names only the open tab, by its number.
                    row.starts_with("Description")
                        || row.chars().next().is_some_and(|c| c.is_ascii_digit()),
                )
                .map(|(_, at)| *at);
            let title = rows
                .iter()
                .find(|(row, _)| row.chars().skip(1).collect::<String>().starts_with("Files"))
                .map(|(_, at)| *at + 1);
            assert!(tabs.is_some(), "{width}x{height}: no tab bar");
            assert_eq!(tabs, title, "{width}x{height}");
        }
    }
}

#[test]
fn the_sidebar_names_the_issues_a_pr_closes_and_only_then() {
    let mut state = fixture();
    state.screen = Screen::Detail {
        pr_id: PrId(42),
        tab: DetailTab::Overview,
    };
    let draw = |state: &mut AppState| {
        let mut terminal = Terminal::new(TestBackend::new(150, 50)).unwrap();
        terminal.draw(|frame| render(frame, state)).unwrap();
        rendered_text(&terminal)
    };
    assert!(!draw(&mut state).contains("Closes"));

    if let Some(data) = state.store.cache.details.get_mut(&PrId(42)) {
        data.info = LoadState::Loaded(PrInfo {
            description: None,
            labels: vec![],
            issues: vec![LinkedIssue {
                number: 12,
                title: "Crash on start".into(),
                url: Some("https://example.com/team/project/issues/12".into()),
            }],
        });
    }
    let text = draw(&mut state);
    assert!(
        text.contains("Closes") && text.contains("#12 Crash on start"),
        "{text}"
    );
}

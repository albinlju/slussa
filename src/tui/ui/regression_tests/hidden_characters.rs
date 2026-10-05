//! What a diff line holds that a terminal would not draw is shown, not dropped.

use super::support::*;

/// The Diff tab with `content` as the one added line of the file.
fn diff_of(content: &str) -> AppState {
    let mut state = pr_on(DetailTab::Diff, Activity::default());
    if let Some(data) = state.store.cache.details.get_mut(&PrId(42))
        && let LoadState::Loaded(diff) = &mut data.diff
        && let Some(hunk) = diff
            .files
            .first_mut()
            .and_then(|file| file.hunks.first_mut())
    {
        hunk.lines = vec![DiffLine::Added(content.into())];
    }
    state
}

#[test]
fn a_tab_keeps_the_indentation_and_a_hidden_character_is_named() {
    let mut state = diff_of("\tif admin\u{202E} {\u{200B}");
    let text = draw(&mut state, 140, 30);

    assert!(
        text.contains("+    if admin"),
        "a tab is spaces, not nothing: {text}"
    );
    assert!(text.contains("‹U+202E›"), "{text}");
    assert!(text.contains("‹U+200B›"), "{text}");
}

#[test]
fn ordinary_code_is_drawn_as_it_is() {
    let mut state = diff_of("    let s = \"åäö\"; // fine");
    let text = draw(&mut state, 140, 30);
    assert!(text.contains("let s = \"åäö\"; // fine"), "{text}");
    assert!(!text.contains("‹U+"), "{text}");
}

#[test]
fn a_hidden_character_is_drawn_in_the_warning_colour() {
    use ratatui::{Terminal, backend::TestBackend, style::Color};
    let mut state = diff_of("a\u{202E}b");
    let mut terminal = Terminal::new(TestBackend::new(140, 30)).unwrap();
    terminal.draw(|frame| render(frame, &mut state)).unwrap();
    let buffer = terminal.backend().buffer();
    let warning = theme::current().warning;
    let mut marker_colours = Vec::new();
    for y in 0..30 {
        for x in 0..140 {
            if buffer[(x, y)].symbol() == "‹" {
                marker_colours.push(buffer[(x, y)].fg);
            }
        }
    }
    assert_eq!(
        marker_colours,
        [warning],
        "one marker, in the warning colour"
    );
    assert_ne!(warning, Color::Reset);
}

#[test]
fn the_code_a_comment_quotes_shows_them_too() {
    let mut state = diff_of("\treturn admin\u{202E};");
    state.screen = Screen::Detail {
        pr_id: PrId(42),
        tab: DetailTab::Overview,
    };
    if let Some(data) = state.store.cache.details.get_mut(&PrId(42)) {
        data.activity = LoadState::Loaded(Activity {
            threads: vec![thread(1, false, vec![comment(1, "Is this right?")])],
            ..Activity::default()
        });
    }
    let text = draw(&mut state, 140, 40);

    assert!(text.contains("Is this right?"), "{text}");
    assert!(text.contains("‹U+202E›"), "the quoted line: {text}");
    assert!(
        text.contains("    return admin"),
        "and its indentation: {text}"
    );
}

pub mod pr_detail;
pub mod pr_list;
pub mod theme;

use ratatui::{Frame, crossterm::event::KeyCode};

use crate::{
    app::state::{AppState, Screen},
    domain::{
        comment::{Comment, ReviewThread},
        commit::Commit,
        diff::Diff,
        pr::PullRequest,
    },
    tui::pr_detail::DetailTab,
};

#[derive(Debug)]
pub enum Action {
    Quit,
    Back,
    NextPr,
    PrevPr,
    NextTab,
    PrevTab,
    SelectTab(DetailTab),
    OpenPr(u64),
    PrsLoaded(Result<Vec<PullRequest>, String>),
    CommitsLoaded(u64, Result<Vec<Commit>, String>),
    DiffLoaded(u64, Result<Diff, String>),
    CommentsLoaded(u64, Result<Vec<Comment>, String>),
    ReviewThreadsLoaded(u64, Result<Vec<ReviewThread>, String>),
    DiffCursorDown,
    DiffCursorUp,
    DiffToggleAtCursor,
    DiffCollapseAtCursor,
    DiffExpandAtCursor,
    DescriptionScrollDown,
    DescriptionScrollUp,
    OverviewScrollDown,
    OverviewScrollUp,
    OpenFilterPicker,
    CloseFilterPicker,
    FilterPickerNext,
    FilterPickerPrev,
    ApplyFilter,
}

pub fn render(frame: &mut Frame, state: &mut AppState) {
    match state.screen {
        Screen::List => pr_list::render(frame, state, frame.area()),
        Screen::Detail { pr_id, tab } => {
            pr_detail::render(frame, state, pr_id, tab, frame.area())
        }
    }
}

pub fn key_to_action(state: &AppState, key: KeyCode) -> Option<Action> {
    match state.screen {
        Screen::List => pr_list::key_to_action(state, key),
        Screen::Detail { .. } => pr_detail::key_to_action(state, key),
    }
}

const SPINNER_FRAMES: &[&str] = &[
    "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏",
];

pub fn spinner_frame() -> &'static str {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let idx = (now / 100) as usize % SPINNER_FRAMES.len();
    SPINNER_FRAMES[idx]
}

/// Footer with left-side key hints and a right-side `donate / ?` block.
/// Used by both pr_list and pr_detail so the bar stays consistent.
pub fn render_footer(frame: &mut Frame, area: ratatui::layout::Rect, hints: &str) {
    use ratatui::{
        style::Style,
        text::{Line, Span},
        widgets::Paragraph,
    };

    let theme = crate::tui::theme::current();
    let muted = Style::default().fg(theme.muted);

    let left = format!("  {}", hints);
    // Nerd Font glyphs: \u{f004} heart, \u{f059} question-circle.
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    let right_spans = vec![
        Span::styled("\u{f004}", Style::default().fg(theme.orange)),
        Span::styled(" donate", muted),
        Span::raw("    "),
        Span::styled("\u{f059}", muted),
        Span::styled(" help", muted),
        Span::raw("    "),
        Span::styled(version, muted),
        Span::raw("  "),
    ];

    let left_w: usize = left.chars().count();
    let right_w: usize = right_spans.iter().map(|s| s.content.chars().count()).sum();
    let gap = (area.width as usize)
        .saturating_sub(left_w + right_w)
        .max(1);

    let mut spans = vec![Span::styled(left, muted), Span::raw(" ".repeat(gap))];
    spans.extend(right_spans);
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

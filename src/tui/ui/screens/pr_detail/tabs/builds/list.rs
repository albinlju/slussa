use super::super::super::build_status::{OverallState, build_stats, progress_bar, state_color};
use crate::{
    domain::ci::{Build, BuildState},
    tui::ui::{component::scroll_to_item, format, icons, theme, widgets},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

/// The builds, one row each, with the cursor on one of them.
pub(super) fn render(
    frame: &mut Frame<'_>,
    builds: &[Build],
    cursor: usize,
    scroll: &mut u16,
    viewport: &mut u16,
    area: Rect,
) {
    let width = area.width.saturating_sub(1) as usize;
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(builds.len() + HEADER_ROWS);
    lines.push(status_summary(builds, width));
    lines.push(Line::default());

    let name_col = builds
        .iter()
        .map(|b| Span::raw(b.name.as_str()).width())
        .max()
        .unwrap_or(0)
        .min(width.saturating_sub(20).max(10));

    let highlight = Style::default().bg(theme::current().highlight_bg);
    for (at, build) in builds.iter().enumerate() {
        let row = build_row(build, name_col, width);
        lines.push(if at == cursor {
            row.style(highlight)
        } else {
            row
        });
    }

    // Keep the cursor's row on screen: the rows above it are the summary.
    *scroll = scroll_to_item(
        *scroll,
        cursor + HEADER_ROWS,
        1,
        lines.len(),
        usize::from(area.height),
    );
    widgets::scrolled_paragraph(frame, lines, scroll, viewport, area);
}

/// The summary and the blank line under it.
const HEADER_ROWS: usize = 2;

fn status_summary(builds: &[Build], width: usize) -> Line<'static> {
    let theme = theme::current();
    let stats = build_stats(builds);
    let overall = OverallState::of(&stats);
    let (icon, label) = overall.glyph();

    let mut spans = vec![
        Span::styled(
            icon,
            Style::default()
                .fg(overall.color())
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            label,
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("   {}/{} passing   ", stats.passing, stats.total),
            Style::default().fg(theme.muted),
        ),
    ];
    if width >= 60 {
        spans.extend(progress_bar(builds));
    }
    if width == 0 {
        Line::default()
    } else {
        Line::from(widgets::truncate_to_width(spans, width))
    }
}

fn build_row(build: &Build, name_col: usize, width: usize) -> Line<'static> {
    let theme = theme::current();
    let (icon, label) = state_glyph(build.state);
    let color = state_color(build.state);

    let name = format::truncate_ellipsis(&build.name, name_col);
    let name_pad = name_col.saturating_sub(Span::raw(name.as_str()).width());

    let left = vec![
        Span::styled(icon, Style::default().fg(color)),
        Span::raw("  "),
        Span::styled(
            name,
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(name_pad)),
    ];
    let mut right = vec![Span::styled(label, Style::default().fg(color))];
    if width >= 40 {
        right.push(Span::styled(
            format!("  {}", format_duration(build.duration_ms)),
            Style::default().fg(theme.muted),
        ));
    }
    widgets::fitted_row(left, right, width)
}

pub(super) const fn state_glyph(state: BuildState) -> (&'static str, &'static str) {
    match state {
        BuildState::Successful => (icons::CHECK_CIRCLE, "passing"),
        BuildState::Failed => (icons::TIMES_CIRCLE, "failed"),
        BuildState::InProgress => (icons::CIRCLE, "running"),
        BuildState::Cancelled => (icons::BAN, "cancelled"),
        BuildState::Unknown => (icons::QUESTION_CIRCLE, "unknown"),
    }
}

fn format_duration(ms: Option<u64>) -> String {
    match ms {
        Some(ms) => {
            let secs = ms / 1000;
            format!("{}m{:02}s", secs / 60, secs % 60)
        }
        None => "—".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_rows_preserve_status_before_duration_and_fit_the_view() {
        for state in [
            BuildState::Successful,
            BuildState::Failed,
            BuildState::InProgress,
            BuildState::Cancelled,
            BuildState::Unknown,
        ] {
            let build = Build {
                name: "Integration 非常に長い test suite".into(),
                state,
                duration_ms: Some(65000),
                log: None,
            };
            for width in [0, 1, 20, 40, 80] {
                let line = build_row(&build, 30, width);
                assert!(line.width() <= width);
                if width >= 20 {
                    assert!(line.to_string().contains(state_glyph(state).1));
                }
                if width >= 40 {
                    assert!(line.to_string().ends_with("1m05s"));
                }
                assert!(status_summary(std::slice::from_ref(&build), width).width() <= width);
            }
        }
    }
}

//! Builds tab — the CI build statuses reported against the PR's source
//! commit. A summary band (running/passed/failed + an `N/M passing` count and
//! a per-build progress bar) sits above an aligned list of build rows.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    app::state::{LoadState, PrData},
    domain::ci::{Build, BuildState},
    tui::{theme, widgets},
};

pub fn render(frame: &mut Frame, pr_data: Option<&PrData>, area: Rect) {
    let theme = theme::current();
    match pr_data.map(|d| &d.builds) {
        None | Some(LoadState::NotRequested) | Some(LoadState::Loading) => {
            let p = Paragraph::new(format!("{}  Loading builds...", widgets::spinner_frame()))
                .style(Style::default().fg(theme.warning));
            frame.render_widget(p, area);
        }
        Some(LoadState::Failed(msg)) => {
            let p = Paragraph::new(format!("Couldn't load builds: {msg}"))
                .style(Style::default().fg(theme.error));
            frame.render_widget(p, area);
        }
        Some(LoadState::Loaded(builds)) if builds.is_empty() => {
            let p = Paragraph::new("(no builds reported for this commit)")
                .style(Style::default().fg(theme.muted));
            frame.render_widget(p, area);
        }
        Some(LoadState::Loaded(builds)) => render_builds(frame, builds, area),
    }
}

fn render_builds(frame: &mut Frame, builds: &[Build], area: Rect) {
    let width = area.width as usize;
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(builds.len() + 2);
    lines.push(summary_line(builds));
    lines.push(Line::default());

    // Align the state labels into a column: reserve room for the longest
    // "<icon> <name>" prefix, capped so a single long name can't crowd out
    // the rest of the row.
    let name_col = builds
        .iter()
        .map(|b| b.name.chars().count())
        .max()
        .unwrap_or(0)
        .min(width.saturating_sub(20).max(10));

    for build in builds {
        lines.push(build_row(build, name_col, width));
    }

    frame.render_widget(Paragraph::new(lines), area);
}

/// `◐ Checks running   3/5 passing   ▰▰▰▰▱`
fn summary_line(builds: &[Build]) -> Line<'static> {
    let theme = theme::current();
    let total = builds.len();
    let passing = builds
        .iter()
        .filter(|b| b.state == BuildState::Successful)
        .count();
    let any_running = builds.iter().any(|b| b.state == BuildState::InProgress);
    let any_failed = builds
        .iter()
        .any(|b| matches!(b.state, BuildState::Failed | BuildState::Cancelled));

    let (icon, label, accent) = if any_running {
        (widgets::spinner_frame().to_string(), "Checks running", theme.warning)
    } else if any_failed {
        ("\u{f057}".to_string(), "Some checks failed", theme.error)
    } else if passing == total {
        ("\u{f058}".to_string(), "All checks passed", theme.success)
    } else {
        ("\u{f059}".to_string(), "Checks complete", theme.muted)
    };

    let mut spans = vec![
        Span::styled(icon, Style::default().fg(accent).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(label, Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!("   {passing}/{total} passing   "),
            Style::default().fg(theme.muted),
        ),
    ];
    spans.extend(progress_bar(builds));
    Line::from(spans)
}

/// One filled block per build, colored by its state. Shared with the Overview
/// sidebar's Builds section.
pub(super) fn progress_bar(builds: &[Build]) -> Vec<Span<'static>> {
    builds
        .iter()
        .map(|b| Span::styled("\u{25b0}", Style::default().fg(state_color(b.state))))
        .collect()
}

fn build_row(build: &Build, name_col: usize, width: usize) -> Line<'static> {
    let theme = theme::current();
    let (icon, label) = state_glyph(build.state);
    let color = state_color(build.state);

    let name = truncate(&build.name, name_col);
    let name_pad = name_col.saturating_sub(name.chars().count());

    let left = vec![
        Span::styled(icon, Style::default().fg(color)),
        Span::raw("  "),
        Span::styled(name, Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
        Span::raw(" ".repeat(name_pad + 2)),
        Span::styled(label, Style::default().fg(color)),
    ];
    let right = vec![Span::styled(
        format_duration(build.duration_ms),
        Style::default().fg(theme.muted),
    )];
    Line::from(widgets::justify_between(left, right, width))
}

fn state_color(state: BuildState) -> Color {
    let theme = theme::current();
    match state {
        BuildState::Successful => theme.success,
        BuildState::Failed => theme.error,
        BuildState::InProgress => theme.warning,
        BuildState::Cancelled | BuildState::Unknown => theme.muted,
    }
}

/// (status glyph, human label) for a build state.
fn state_glyph(state: BuildState) -> (&'static str, &'static str) {
    match state {
        BuildState::Successful => ("\u{f058}", "passing"), //  check-circle
        BuildState::Failed => ("\u{f057}", "failed"),      //  times-circle
        BuildState::InProgress => ("\u{f111}", "running"), //  circle
        BuildState::Cancelled => ("\u{f05e}", "cancelled"), //  ban
        BuildState::Unknown => ("\u{f059}", "unknown"),    //  question-circle
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

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

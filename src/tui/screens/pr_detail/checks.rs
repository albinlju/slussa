use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    app::state::PrData,
    domain::ci::{Build, BuildState},
    tui::{format, theme, widgets},
};

pub fn render(frame: &mut Frame, pr_data: Option<&PrData>, area: Rect) {
    let Some(builds) =
        widgets::loaded_or_placeholder(frame, pr_data.map(|d| &d.builds), "builds", area)
    else {
        return;
    };
    if builds.is_empty() {
        frame.render_widget(widgets::empty_state("(no builds reported for this commit)"), area);
        return;
    }
    render_builds(frame, builds, area);
}

fn render_builds(frame: &mut Frame, builds: &[Build], area: Rect) {
    let width = area.width as usize;
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(builds.len() + 2);
    lines.push(summary_line(builds));
    lines.push(Line::default());

    let name_col = builds
        .iter()
        .map(|b| Span::raw(b.name.as_str()).width())
        .max()
        .unwrap_or(0)
        .min(width.saturating_sub(20).max(10));

    for build in builds {
        lines.push(build_row(build, name_col, width));
    }

    frame.render_widget(Paragraph::new(lines), area);
}

pub(super) struct BuildStats {
    pub passing: usize,
    pub total: usize,
    pub any_running: bool,
    pub any_failed: bool,
}

impl BuildStats {
    pub fn accent(&self) -> Color {
        let theme = theme::current();
        if self.any_running {
            theme.warning
        } else if self.any_failed {
            theme.error
        } else if self.passing == self.total {
            theme.success
        } else {
            theme.muted
        }
    }
}

pub(super) fn build_stats(builds: &[Build]) -> BuildStats {
    BuildStats {
        passing: builds
            .iter()
            .filter(|b| b.state == BuildState::Successful)
            .count(),
        total: builds.len(),
        any_running: builds.iter().any(|b| b.state == BuildState::InProgress),
        any_failed: builds
            .iter()
            .any(|b| matches!(b.state, BuildState::Failed | BuildState::Cancelled)),
    }
}

fn summary_line(builds: &[Build]) -> Line<'static> {
    let theme = theme::current();
    let stats = build_stats(builds);
    let (passing, total, accent) = (stats.passing, stats.total, stats.accent());

    let (icon, label) = if stats.any_running {
        (widgets::spinner_frame().to_string(), "Checks running")
    } else if stats.any_failed {
        ("\u{f057}".to_string(), "Some checks failed")
    } else if passing == total {
        ("\u{f058}".to_string(), "All checks passed")
    } else {
        ("\u{f059}".to_string(), "Checks complete")
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

    let name = format::truncate_ellipsis(&build.name, name_col);
    let name_pad = name_col.saturating_sub(Span::raw(name.as_str()).width());

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

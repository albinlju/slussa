use super::super::build_status::{OverallState, build_stats, progress_bar, state_color};
use crate::{
    app::store::PrData,
    domain::ci::{Build, BuildState},
    tui::{format, icons, theme, widgets},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

fn render(frame: &mut Frame, pr_data: Option<&PrData>, ui: &mut Builds, area: Rect) {
    let Some(builds) =
        widgets::loaded_or_placeholder(frame, pr_data.map(|d| &d.builds), "builds", area)
    else {
        return;
    };
    if builds.is_empty() {
        frame.render_widget(
            widgets::empty_state("(no builds reported for this commit)"),
            area,
        );
        return;
    }
    render_builds(frame, builds, ui, area);
}

fn render_builds(frame: &mut Frame, builds: &[Build], ui: &mut Builds, area: Rect) {
    let width = area.width as usize;
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(builds.len() + 2);
    lines.push(status_summary(builds));
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

    widgets::scrolled_paragraph(frame, lines, &mut ui.scroll, &mut ui.viewport, area);
}

fn status_summary(builds: &[Build]) -> Line<'static> {
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
    spans.extend(progress_bar(builds));
    Line::from(spans)
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
        Span::raw(" ".repeat(name_pad + 2)),
        Span::styled(label, Style::default().fg(color)),
    ];
    let right = vec![Span::styled(
        format_duration(build.duration_ms),
        Style::default().fg(theme.muted),
    )];
    Line::from(widgets::justify_between(left, right, width))
}

fn state_glyph(state: BuildState) -> (&'static str, &'static str) {
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

#[derive(Debug, Default)]
pub struct Builds {
    scroll: u16,
    viewport: u16,
}
impl crate::tui::component::Component for Builds {
    type Context<'a> = Option<&'a PrData>;
    type Message = crate::app::action::DetailAction;
    fn handle_key(
        &self,
        key: ratatui::crossterm::event::KeyEvent,
        _: &Self::Context<'_>,
    ) -> Option<crate::app::action::Action> {
        use ratatui::crossterm::event::KeyCode;
        let delta = match key.code {
            KeyCode::Char('j') | KeyCode::Down => 1,
            KeyCode::Char('k') | KeyCode::Up => -1,
            KeyCode::PageDown => crate::tui::screens::half_page(self.viewport),
            KeyCode::PageUp => -crate::tui::screens::half_page(self.viewport),
            _ => return None,
        };
        Some(crate::app::action::Action::Detail(
            crate::app::action::DetailAction::BuildsScroll(delta),
        ))
    }
    fn update(
        &mut self,
        action: Self::Message,
        _: &Self::Context<'_>,
    ) -> Option<crate::app::action::Action> {
        if let crate::app::action::DetailAction::BuildsScroll(delta) = action {
            self.scroll = crate::tui::component::scroll(self.scroll, delta);
        }
        None
    }
    fn render(&mut self, frame: &mut Frame, area: Rect, data: &Self::Context<'_>) {
        render(frame, *data, self, area);
    }
}

use super::super::build_status::{OverallState, build_stats, progress_bar, state_color};
use crate::{
    domain::ci::{Build, BuildState},
    tui::{
        app::store::PrData,
        ui::{format, icons, theme, widgets},
    },
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

fn render(frame: &mut Frame<'_>, pr_data: Option<&PrData>, ui: &mut Builds, area: Rect) {
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

fn render_builds(frame: &mut Frame<'_>, builds: &[Build], ui: &mut Builds, area: Rect) {
    let width = area.width.saturating_sub(1) as usize;
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(builds.len() + 2);
    lines.push(status_summary(builds, width));
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

const fn state_glyph(state: BuildState) -> (&'static str, &'static str) {
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
impl crate::tui::ui::component::Component for Builds {
    type Input<'a> = ();
    type View<'a> = Option<&'a PrData>;
    /// How far to scroll.
    type Message = i16;
    fn handle_key(
        &self,
        key: crossterm::event::KeyEvent,
        (): &(),
    ) -> Option<crate::tui::ui::action::Action> {
        use ratatui::crossterm::event::KeyCode;
        let delta = match key.code {
            KeyCode::Char('j') | KeyCode::Down => 1,
            KeyCode::Char('k') | KeyCode::Up => -1,
            KeyCode::PageDown => crate::tui::ui::screens::half_page(self.viewport),
            KeyCode::PageUp => -crate::tui::ui::screens::half_page(self.viewport),
            _ => return None,
        };
        Some(crate::tui::ui::action::Action::Detail(
            crate::tui::ui::action::DetailAction::BuildsScroll(delta),
        ))
    }
    fn update(&mut self, delta: Self::Message, (): &()) -> Option<crate::tui::app::effect::Effect> {
        self.scroll = crate::tui::ui::component::scroll(self.scroll, delta);
        None
    }
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, data: &Self::View<'_>) {
        render(frame, *data, self, area);
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

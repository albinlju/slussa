//! One build's log, read from where it failed.

use super::{OpenLog, list::state_glyph};
use crate::{
    domain::{
        build_log::{BuildLog, LogKind},
        ci::Build,
    },
    tui::{
        app::store::LoadState,
        ui::{
            component::saturating_u16, layout, screens::pr_detail::build_status::state_color,
            theme, widgets,
        },
    },
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

/// How many lines of what came before the first error stay above it.
pub(super) const CONTEXT: usize = 3;

/// The log of `build`, or why it is not there yet.
pub(super) fn render(
    frame: &mut Frame<'_>,
    open: &mut OpenLog,
    build: Option<&Build>,
    state: Option<&LoadState<BuildLog>>,
    area: Rect,
) {
    let [header, body] = layout::split(
        area,
        Direction::Vertical,
        [Constraint::Length(2), Constraint::Min(0)],
    );
    // The header's text stands where the panels' titles do, one column in.
    let header = Rect {
        x: header.x.saturating_add(1),
        width: header.width.saturating_sub(1),
        ..header
    };
    let Some(log) = widgets::loaded_or_placeholder(frame, state, "the build log", body) else {
        frame.render_widget(Paragraph::new(title(build, None)), header);
        return;
    };
    frame.render_widget(Paragraph::new(title(build, Some(log))), header);
    if log.lines.is_empty() {
        frame.render_widget(widgets::empty_state("(the log is empty)"), body);
        return;
    }
    let height = usize::from(body.height);
    open.viewport = body.height;
    let last_top = log.lines.len().saturating_sub(height);
    let top = open.scroll.map_or_else(
        || land(open, log, last_top),
        |scroll| usize::from(scroll).min(last_top),
    );
    open.scroll = Some(saturating_u16(top));

    let lines: Vec<Line<'static>> = log
        .lines
        .iter()
        .skip(top)
        .take(height)
        .map(|line| styled(&line.text, line.kind))
        .collect();
    let content = Rect {
        width: body.width.saturating_sub(1),
        ..body
    };
    frame.render_widget(Paragraph::new(lines), content);
    let bar = widgets::scrollbar(saturating_u16(top), saturating_u16(last_top), body.height);
    if !bar.is_empty() {
        frame.render_widget(Paragraph::new(bar), layout::scrollbar_area(body));
    }
}

/// Where a log opens: on its first error with a little before it, and with no
/// error at the end, where what stopped it is.
fn land(open: &mut OpenLog, log: &BuildLog, last_top: usize) -> usize {
    open.error = log.errors.first().copied();
    open.error
        .map_or(last_top, |at| at.saturating_sub(CONTEXT).min(last_top))
}

/// The build's name and what the log holds.
fn title(build: Option<&Build>, log: Option<&BuildLog>) -> Line<'static> {
    let theme = theme::current();
    let mut spans = Vec::new();
    if let Some(build) = build {
        let (icon, _) = state_glyph(build.state);
        spans.push(Span::styled(
            icon,
            Style::default().fg(state_color(build.state)),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            build.name.clone(),
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ));
    }
    if let Some(log) = log {
        let errors = match log.errors.len() {
            0 => "no errors marked".to_owned(),
            1 => "1 error".to_owned(),
            many => format!("{many} errors"),
        };
        spans.push(Span::styled(
            format!("   {errors}"),
            Style::default().fg(theme.muted),
        ));
        if log.omitted > 0 {
            spans.push(Span::styled(
                format!("   the first {} lines are left out", log.omitted),
                Style::default().fg(theme.muted),
            ));
        }
    }
    Line::from(spans)
}

fn styled(text: &str, kind: LogKind) -> Line<'static> {
    let theme = theme::current();
    let style = match kind {
        LogKind::Plain => Style::default().fg(theme.fg),
        LogKind::Group => Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        LogKind::Warning => Style::default().fg(theme.warning),
        LogKind::Error => Style::default().fg(theme.error),
    };
    Line::styled(text.to_owned(), style)
}

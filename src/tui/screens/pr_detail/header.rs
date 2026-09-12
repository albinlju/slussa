use crate::{
    app::store::LoadState,
    domain::pr::{Mergeability, PrStatus, PullRequest},
    tui::{icons, theme, widgets},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Padding, Paragraph},
};

/// Header badge for a PR's mergeability — glyph + label, colour carrying the
/// meaning. `None` while the fetch is unresolved so nothing flickers in.
fn mergeability_badge(state: &LoadState<Mergeability>) -> Option<Span<'static>> {
    let theme = theme::current();
    let (glyph, label, color) = match state {
        LoadState::Loading => (icons::ADJUST, "checking…", theme.muted),
        LoadState::Loaded(Mergeability::Mergeable) => {
            (icons::CHECK_CIRCLE, "mergeable", theme.success)
        }
        LoadState::Loaded(Mergeability::Conflicts) => {
            (icons::TIMES_CIRCLE, "conflicts", theme.warning)
        }
        LoadState::Loaded(Mergeability::Unknown) => {
            (icons::QUESTION_CIRCLE, "mergeability unknown", theme.muted)
        }
        LoadState::NotRequested | LoadState::Failed(_) => return None,
    };
    Some(Span::styled(
        format!("{glyph} {label}"),
        Style::default().fg(color),
    ))
}

pub(super) fn render(
    frame: &mut Frame,
    pr: &PullRequest,
    mergeability: Option<&LoadState<Mergeability>>,
    area: Rect,
) {
    let theme = theme::current();
    let status_color = theme.status_color(&pr.status);

    let title_line = Line::from(vec![
        Span::styled(format!("#{} ", pr.id), Style::default().fg(theme.muted)),
        Span::styled(
            pr.title.clone(),
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ),
    ]);

    let mut left_spans: Vec<Span<'static>> = vec![
        // Padded background badge — matches the reaction pills and needs no
        // Nerd Font (no powerline caps).
        Span::styled(
            format!(" {} ", pr.status.label()),
            Style::default()
                .fg(theme.bg)
                .bg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" @{}", pr.author.username),
            Style::default().fg(theme.info),
        ),
    ];
    if area.height > 2 {
        left_spans.extend([
            Span::styled("  wants to merge  ", Style::default().fg(theme.muted)),
            Span::styled(pr.source_branch.clone(), Style::default().fg(theme.orange)),
            Span::raw(" → "),
            Span::styled(pr.target_branch.clone(), Style::default().fg(theme.info)),
        ]);
    }
    // Mergeability is only meaningful while the PR is still open.
    let open = !matches!(pr.status, PrStatus::Merged | PrStatus::Declined);
    if open && let Some(badge) = mergeability.and_then(mergeability_badge) {
        left_spans.push(Span::raw("    "));
        left_spans.push(badge);
    }
    let meta_line = Line::from(left_spans);

    let padding = if area.width < 70 { 1 } else { 2 };
    let width = area.width.saturating_sub(padding * 2) as usize;
    let mut lines = vec![title_line];
    if area.height > 2 {
        lines.push(Line::default());
    }
    lines.push(meta_line);
    let lines: Vec<_> = lines
        .into_iter()
        .map(|line| Line::from(widgets::truncate_to_width(line.spans, width)))
        .collect();
    let paragraph =
        Paragraph::new(lines).block(Block::default().padding(Padding::horizontal(padding)));
    frame.render_widget(paragraph, area);
}

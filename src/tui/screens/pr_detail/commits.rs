use chrono::{DateTime, Utc};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
};

use crate::{
    app::state::{LoadState, PrData},
    domain::commit::Commit,
    tui::{theme, widgets},
};

pub fn render(frame: &mut Frame, pr_data: Option<&PrData>, area: Rect) {
    let theme = theme::current();
    let commits_state = pr_data.map(|d| &d.commits);

    match commits_state {
        None | Some(LoadState::NotRequested) | Some(LoadState::Loading) => {
            let paragraph = Paragraph::new(format!("{}  Loading commits...", widgets::spinner_frame()))
                .style(Style::default().fg(theme.warning));
            frame.render_widget(paragraph, area);
        }
        Some(LoadState::Failed(msg)) => {
            let paragraph = Paragraph::new(format!("Couldn't load commits: {msg}"))
                .style(Style::default().fg(theme.error));
            frame.render_widget(paragraph, area);
        }
        Some(LoadState::Loaded(commits)) if commits.is_empty() => {
            let paragraph =
                Paragraph::new("(no commits)").style(Style::default().fg(theme.muted));
            frame.render_widget(paragraph, area);
        }
        Some(LoadState::Loaded(commits)) => {
            let last_idx = commits.len() - 1;
            let now = Utc::now();
            let width = area.width as usize;
            let divider_style = Style::default().fg(theme.divider);
            let items: Vec<ListItem> = commits
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let is_last = i == last_idx;
                    let mut lines = vec![build_commit_line(c, is_last, now, width)];
                    if !is_last {
                        lines.push(Line::styled("─".repeat(width), divider_style));
                    }
                    ListItem::new(lines)
                })
                .collect();

            let list = List::new(items);
            frame.render_widget(list, area);
        }
    }
}

fn build_commit_line(
    c: &Commit,
    is_last: bool,
    now: DateTime<Utc>,
    width: usize,
) -> Line<'static> {
    let theme = theme::current();
    // ├─ for all but the last (oldest) commit, └─ to close the branch off
    // at the bottom.
    let graph = if is_last { "└─ " } else { "├─ " };
    let short_oid: String = c.oid.chars().take(7).collect();
    let age = widgets::relative_age(c.authored_at, now);

    // Right segment: "author  +N -M  · age". Built upfront so we can measure
    // it with the spans themselves instead of summing hand-counted literals.
    let right_spans: Vec<Span<'static>> = vec![
        Span::styled(c.author_name.clone(), Style::default().fg(theme.info)),
        Span::raw("  "),
        Span::styled(format!("+{}", c.additions), Style::default().fg(theme.diff_added)),
        Span::raw(" "),
        Span::styled(format!("-{}", c.deletions), Style::default().fg(theme.diff_removed)),
        Span::styled("  · ", Style::default().fg(theme.muted)),
        Span::styled(age, Style::default().fg(theme.muted)),
    ];
    let right_visible: usize = right_spans.iter().map(|s| s.width()).sum();

    let left_fixed = graph.chars().count() + short_oid.chars().count() + 2; // 2 spaces after oid
    let headline_max = width
        .saturating_sub(left_fixed + right_visible + 2) // 2-col min gap before right block
        .max(10);
    let headline: String = if c.headline.chars().count() > headline_max {
        let mut s: String = c.headline.chars().take(headline_max.saturating_sub(1)).collect();
        s.push('…');
        s
    } else {
        c.headline.clone()
    };

    let used = left_fixed + headline.chars().count() + right_visible;
    let pad = width.saturating_sub(used).max(2);

    let mut spans: Vec<Span<'static>> = Vec::with_capacity(4 + right_spans.len());
    spans.push(Span::styled(graph, Style::default().fg(theme.muted)));
    spans.push(Span::styled(
        format!("{short_oid}  "),
        Style::default().fg(theme.accent),
    ));
    spans.push(Span::raw(headline));
    spans.push(Span::raw(" ".repeat(pad)));
    spans.extend(right_spans);
    Line::from(spans)
}

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
};

use crate::{
    app::state::{LoadState, PrData},
    tui::{spinner_frame, theme},
};

pub fn render(frame: &mut Frame, pr_data: Option<&PrData>, area: Rect) {
    let theme = theme::current();
    let commits_state = pr_data.map(|d| &d.commits);

    match commits_state {
        None | Some(LoadState::NotRequested) | Some(LoadState::Loading) => {
            let paragraph = Paragraph::new(format!("{}  Loading commits...", spinner_frame()))
                .style(Style::default().fg(theme.warning));
            frame.render_widget(paragraph, area);
        }
        Some(LoadState::Loaded(commits)) if commits.is_empty() => {
            let paragraph =
                Paragraph::new("(no commits)").style(Style::default().fg(theme.muted));
            frame.render_widget(paragraph, area);
        }
        Some(LoadState::Loaded(commits)) => {
            let items: Vec<ListItem> = commits
                .iter()
                .map(|c| {
                    let short_oid: String = c.oid.chars().take(7).collect();
                    let line = Line::from(vec![
                        Span::styled(
                            format!("{}  ", short_oid),
                            Style::default().fg(theme.accent),
                        ),
                        Span::raw(c.headline.clone()),
                        Span::raw("  "),
                        Span::styled(
                            format!("— {}", c.author_name),
                            Style::default().fg(theme.info),
                        ),
                    ]);
                    ListItem::new(line)
                })
                .collect();

            let list = List::new(items);
            frame.render_widget(list, area);
        }
    }
}

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
};

use crate::{
    app::state::{AppState, LoadState},
    domain::pr::PullRequest,
    tui::spinner_frame,
};

pub fn render(frame: &mut Frame, pr: &PullRequest, state: &AppState, area: Rect) {
    let commits_state = state.cache.details.get(&pr.id).map(|d| &d.commits);

    match commits_state {
        None | Some(LoadState::NotRequested) | Some(LoadState::Loading) => {
            let paragraph = Paragraph::new(format!("{}  Loading commits...", spinner_frame()))
                .style(Style::default().fg(Color::Yellow));
            frame.render_widget(paragraph, area);
        }
        Some(LoadState::Loaded(commits)) if commits.is_empty() => {
            let paragraph =
                Paragraph::new("(no commits)").style(Style::default().fg(Color::DarkGray));
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
                            Style::default().fg(Color::Yellow),
                        ),
                        Span::raw(c.headline.clone()),
                        Span::raw("  "),
                        Span::styled(
                            format!("— {}", c.author_name),
                            Style::default().fg(Color::Cyan),
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

use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::domain::pr::PullRequest;

pub fn render(frame: &mut Frame, pr: &PullRequest, area: Rect) {
    let body = pr
        .description
        .clone()
        .unwrap_or_else(|| "(ingen beskrivning)".to_string());

    let paragraph = Paragraph::new(body)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(paragraph, area);
}

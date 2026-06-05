use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
};

use crate::domain::pr::PullRequest;

pub fn render(frame: &mut Frame, _pr: &PullRequest, area: Rect) {
    let paragraph = Paragraph::new("Conversation — TODO").style(Style::default().fg(Color::DarkGray));
    frame.render_widget(paragraph, area);
}

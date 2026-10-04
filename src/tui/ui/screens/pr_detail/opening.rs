//! A PR the reader named by its number, while it is being read: the frame of the
//! PR screen with a line saying so, so that slussa starts on the PR and does not
//! show the list first and flip. `Esc` goes back and `q` quits.

use crate::{
    domain::pr::PrId,
    tui::ui::{layout, theme, widgets},
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction},
    style::Style,
    text::Line,
    widgets::{Block, Borders, Paragraph},
};

pub fn render(frame: &mut Frame<'_>, pr_id: PrId) {
    let theme = theme::current();
    let [main_area, footer_area] = layout::split(
        frame.area(),
        Direction::Vertical,
        [Constraint::Min(0), Constraint::Length(1)],
    );
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border));
    let inner = outer.inner(main_area);
    frame.render_widget(outer, main_area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::default(),
            widgets::loading(&format!("Loading PR #{pr_id}…")),
        ]),
        inner,
    );
    let hints = widgets::hints_on("esc: back  q: quit");
    frame.render_widget(
        Paragraph::new(widgets::footer(footer_area.width, &hints, false)),
        footer_area,
    );
}

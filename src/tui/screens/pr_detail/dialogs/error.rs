use crate::tui::theme;
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph, Wrap},
};

pub(in crate::tui::screens::pr_detail) fn render(frame: &mut Frame, message: &str, area: Rect) {
    let theme = theme::current();
    let popup_w = 60.min(area.width.saturating_sub(4)).max(20);
    let text_w = popup_w.saturating_sub(4).max(1);
    let wrapped = (message.chars().count() as u16).div_ceil(text_w);
    let popup_h = (wrapped + 4).min(area.height);
    let popup = Rect {
        x: area.x + area.width.saturating_sub(popup_w) / 2,
        y: area.y + area.height.saturating_sub(popup_h) / 2,
        width: popup_w,
        height: popup_h,
    };

    let lines = vec![
        Line::from(Span::styled(
            message.to_string(),
            Style::default().fg(theme.fg),
        )),
        Line::default(),
        Line::from(Span::styled(
            "any key to dismiss",
            Style::default().fg(theme.muted),
        )),
    ];

    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Error ")
        .border_style(Style::default().fg(theme.error))
        .padding(Padding::horizontal(1));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

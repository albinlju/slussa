use crate::{
    app::action::{Action, DetailAction},
    tui::{component::Component, theme, widgets},
};
use ratatui::{Frame, layout::Rect, style::Style, text::Line, widgets::Paragraph};

#[derive(Debug, Default)]
pub struct ErrorDialog {
    pub scroll: u16,
    pub max_scroll: u16,
}
impl ErrorDialog {
    pub fn render(&mut self, frame: &mut Frame<'_>, message: &str, area: Rect) {
        let width = area.width.min(64);
        let lines = widgets::wrap_text(message, width.saturating_sub(4).max(1) as usize);
        let body = widgets::dialog::frame(
            frame,
            area,
            "Error",
            (width, lines.len() as u16),
            &[("j/k", "scroll"), ("Esc / Enter", "close")],
        );
        self.max_scroll = lines.len().saturating_sub(body.height as usize) as u16;
        self.scroll = self.scroll.min(self.max_scroll);
        frame.render_widget(
            Paragraph::new(lines.into_iter().map(Line::raw).collect::<Vec<_>>())
                .style(Style::default().fg(theme::current().fg))
                .scroll((self.scroll, 0)),
            body,
        );
    }
}

impl Component for ErrorDialog {
    type Context<'a> = &'a str;
    type Message = DetailAction;
    fn handle_key(&self, key: crossterm::event::KeyEvent, _: &Self::Context<'_>) -> Option<Action> {
        use ratatui::crossterm::event::KeyCode;
        let action = match key.code {
            KeyCode::Esc | KeyCode::Enter => DetailAction::DismissError,
            KeyCode::Char('j') | KeyCode::Down => DetailAction::ErrorScroll(1),
            KeyCode::Char('k') | KeyCode::Up => DetailAction::ErrorScroll(-1),
            KeyCode::PageDown => DetailAction::ErrorScroll(5),
            KeyCode::PageUp => DetailAction::ErrorScroll(-5),
            _ => return None,
        };
        Some(Action::Detail(action))
    }
    fn update(&mut self, action: DetailAction, _: &Self::Context<'_>) -> Option<Action> {
        if let DetailAction::ErrorScroll(delta) = action {
            self.scroll = crate::tui::component::scroll(self.scroll, delta).min(self.max_scroll);
            None
        } else {
            Some(Action::Detail(action))
        }
    }
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, message: &Self::Context<'_>) {
        self.render(frame, message, area);
    }
}

use crate::{
    app::action::{Action, Effect, ErrorAction},
    tui::{
        component::{Component, saturating_u16},
        theme, widgets,
    },
};
use ratatui::{Frame, layout::Rect, style::Style, text::Line, widgets::Paragraph};

#[derive(Debug, Default)]
pub struct ErrorDialog {
    pub scroll: u16,
    pub max_scroll: u16,
}
impl Component for ErrorDialog {
    type Input<'a> = ();
    /// The message to show.
    type View<'a> = &'a str;
    type Message = ErrorAction;
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, message: &&str) {
        let width = area.width.min(64);
        let lines = widgets::wrap_text(message, width.saturating_sub(4).max(1) as usize);
        let body = widgets::dialog::frame(
            frame,
            area,
            "Error",
            (width, saturating_u16(lines.len())),
            &[("j/k", "scroll"), ("Esc / Enter", "close")],
        );
        self.max_scroll = saturating_u16(lines.len().saturating_sub(body.height as usize));
        self.scroll = self.scroll.min(self.max_scroll);
        frame.render_widget(
            Paragraph::new(lines.into_iter().map(Line::raw).collect::<Vec<_>>())
                .style(Style::default().fg(theme::current().fg))
                .scroll((self.scroll, 0)),
            body,
        );
    }
    fn handle_key(&self, key: crossterm::event::KeyEvent, (): &()) -> Option<Action> {
        use ratatui::crossterm::event::KeyCode;
        let action = match key.code {
            KeyCode::Esc | KeyCode::Enter => ErrorAction::Dismiss,
            KeyCode::Char('j') | KeyCode::Down => ErrorAction::Scroll(1),
            KeyCode::Char('k') | KeyCode::Up => ErrorAction::Scroll(-1),
            KeyCode::PageDown => ErrorAction::Scroll(5),
            KeyCode::PageUp => ErrorAction::Scroll(-5),
            _ => return None,
        };
        Some(action.into())
    }
    fn update(&mut self, action: ErrorAction, (): &()) -> Option<Effect> {
        match action {
            ErrorAction::Scroll(delta) => {
                self.scroll =
                    crate::tui::component::scroll(self.scroll, delta).min(self.max_scroll);
            }
            // Dismissing is the screen's: the error itself is in the store.
            ErrorAction::Dismiss => {}
        }
        None
    }
}

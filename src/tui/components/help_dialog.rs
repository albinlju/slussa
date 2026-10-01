use crate::{
    app::action::{Action, Effect},
    tui::{
        component::{Component, saturating_u16, scroll},
        theme, widgets,
    },
};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

/// Help owns its scroll position; the containing screen owns opening/closing it.
#[derive(Debug, Default)]
pub struct HelpDialog {
    scroll: u16,
    viewport: u16,
    max_scroll: u16,
}
impl Component for HelpDialog {
    type Context<'a> = &'a [(&'static str, &'static str)];
    type Message = i16;
    fn handle_key(&self, key: KeyEvent, _: &Self::Context<'_>) -> Option<Action> {
        let page = self.viewport.max(1) as i16;
        let delta = match key.code {
            KeyCode::Down | KeyCode::Char('j') => 1,
            KeyCode::Up | KeyCode::Char('k') => -1,
            KeyCode::PageDown => page,
            KeyCode::PageUp => -page,
            _ => return None,
        };
        Some(Action::HelpScroll(delta))
    }
    fn update(&mut self, delta: i16, _: &Self::Context<'_>) -> Option<Effect> {
        self.scroll = scroll(self.scroll, delta).min(self.max_scroll);
        None
    }
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, entries: &Self::Context<'_>) {
        let width = area.width.min(48);
        let height = area
            .height
            .min(saturating_u16(entries.len()).saturating_add(4));
        let popup = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        let theme = theme::current();
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Help ")
            .border_style(Style::default().fg(theme.accent));
        let inner = block.inner(popup);
        frame.render_widget(Clear, popup);
        frame.render_widget(block, popup);
        let body = Rect {
            height: inner.height.saturating_sub(1),
            ..inner
        };
        self.viewport = body.height;
        self.max_scroll = saturating_u16(entries.len().saturating_sub(body.height as usize));
        self.scroll = self.scroll.min(self.max_scroll);
        let lines: Vec<_> = entries
            .iter()
            .map(|(key, desc)| {
                Line::from(vec![
                    Span::styled(format!(" {key:<10}"), Style::default().fg(theme.accent)),
                    Span::styled(desc.to_string(), Style::default().fg(theme.fg)),
                ])
            })
            .collect();
        frame.render_widget(Paragraph::new(lines).scroll((self.scroll, 0)), body);
        if inner.height > 0 {
            let hint = if self.max_scroll > 0 {
                "j/k: scroll  esc: close"
            } else {
                "esc: close"
            };
            let footer = Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1);
            frame.render_widget(
                Paragraph::new(Line::from(widgets::truncate_to_width(
                    vec![Span::styled(hint, Style::default().fg(theme.muted))],
                    inner.width as usize,
                ))),
                footer,
            );
        }
    }
}

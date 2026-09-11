use crate::{
    app::action::{Action, DetailAction},
    tui::{
        component::{Component, step_index},
        theme,
    },
};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph},
};

fn render(
    frame: &mut Frame,
    strategies: &[crate::domain::pr::MergeStrategy],
    cursor: usize,
    area: Rect,
) {
    let theme = theme::current();
    let selected = Style::default()
        .bg(theme.highlight_bg)
        .add_modifier(Modifier::BOLD);
    let normal = Style::default().fg(theme.muted);

    let mut lines = vec![
        Line::from(Span::styled("Merge this PR", Style::default().fg(theme.fg))),
        Line::default(),
    ];
    for (i, strategy) in strategies.iter().enumerate() {
        let marker = if i == cursor { "▶ " } else { "  " };
        let style = if i == cursor { selected } else { normal };
        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(theme.accent)),
            Span::styled(format!(" {} ", strategy.label()), style),
        ]));
    }

    let content_w = lines.iter().map(Line::width).max().unwrap_or(0) as u16;
    let popup_w = (content_w + 4).min(area.width);
    let popup_h = (lines.len() as u16 + 2).min(area.height);
    let popup = Rect {
        x: area.x + area.width.saturating_sub(popup_w) / 2,
        y: area.y + area.height.saturating_sub(popup_h) / 2,
        width: popup_w,
        height: popup_h,
    };

    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Merge ")
        .border_style(Style::default().fg(theme.accent))
        .padding(Padding::horizontal(1));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    frame.render_widget(Paragraph::new(lines), inner);
}

fn key_to_action(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Left | KeyCode::Up | KeyCode::Char('h' | 'k') => {
            Some(Action::Detail(DetailAction::MergeMove(-1)))
        }
        KeyCode::Right | KeyCode::Down | KeyCode::Char('j' | 'l') => {
            Some(Action::Detail(DetailAction::MergeMove(1)))
        }
        KeyCode::Enter => Some(Action::Detail(DetailAction::MergeSelect)),
        KeyCode::Esc => Some(Action::Detail(DetailAction::CloseMergePicker)),
        _ => None,
    }
}

#[derive(Debug, Default)]
pub struct MergeDialog {
    cursor: usize,
}
impl MergeDialog {
    pub fn selected(
        &self,
        strategies: &[crate::domain::pr::MergeStrategy],
    ) -> Option<crate::domain::pr::MergeStrategy> {
        strategies.get(self.cursor).copied()
    }
}

impl Component for MergeDialog {
    type Context<'a> = &'a [crate::domain::pr::MergeStrategy];
    type Message = DetailAction;
    fn handle_key(&self, key: KeyEvent, _: &Self::Context<'_>) -> Option<Action> {
        key_to_action(key.code)
    }
    fn update(&mut self, action: DetailAction, ctx: &Self::Context<'_>) -> Option<Action> {
        match action {
            DetailAction::MergeMove(delta) => {
                self.cursor = step_index(self.cursor, delta, ctx.len());
                None
            }
            other => Some(Action::Detail(other)),
        }
    }
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Self::Context<'_>) {
        render(frame, ctx, self.cursor, area);
    }
}

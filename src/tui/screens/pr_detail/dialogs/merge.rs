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

    let selected_line = 2 + cursor;
    crate::tui::widgets::dialog::choices(frame, area, "Merge", lines, selected_line);
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

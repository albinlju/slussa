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
    frame: &mut Frame<'_>,
    strategies: &[crate::domain::pr::MergeStrategy],
    dialog: &MergeDialog,
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
    if !dialog.pr_label.is_empty() {
        lines.insert(1, Line::styled(dialog.pr_label.clone(), normal));
        lines.insert(
            2,
            Line::from(vec![
                Span::styled("Into: ", normal),
                Span::styled(
                    dialog.target_branch.clone(),
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
        );
        lines.insert(
            3,
            Line::from(vec![
                Span::styled("From: ", normal),
                Span::styled(
                    dialog.source_branch.clone(),
                    Style::default().fg(theme.orange),
                ),
            ]),
        );
    }
    for (i, strategy) in strategies.iter().enumerate() {
        let marker = if i == dialog.cursor { "▶ " } else { "  " };
        let style = if i == dialog.cursor { selected } else { normal };
        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(theme.accent)),
            Span::styled(format!(" {} ", strategy.label()), style),
        ]));
    }

    let selected_line = lines.len() - strategies.len() + dialog.cursor;
    crate::tui::widgets::dialog::choices_with_hints(
        frame,
        area,
        "Merge",
        lines,
        selected_line,
        &[("j/k", "move"), ("Enter", "merge"), ("Esc", "cancel")],
    );
}

const fn key_to_action(code: KeyCode) -> Option<Action> {
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
    pub pr_label: String,
    pub target_branch: String,
    pub source_branch: String,
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
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, ctx: &Self::Context<'_>) {
        render(frame, ctx, self, area);
    }
}

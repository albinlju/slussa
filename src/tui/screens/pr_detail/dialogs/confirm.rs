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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmKind {
    DeleteComment { id: u64, review: bool },
    Decline,
    DiscardReview,
}

impl ConfirmKind {
    pub fn prompt(self) -> &'static str {
        match self {
            Self::DeleteComment { .. } => "Delete this comment?",
            Self::Decline => "Decline this PR?",
            Self::DiscardReview => "Discard this review draft?",
        }
    }
}

const CONFIRM_OPTIONS: [&str; 2] = ["Yes", "No"];

fn render(frame: &mut Frame, kind: ConfirmKind, cursor: usize, area: Rect) {
    let theme = theme::current();
    let selected = Style::default()
        .bg(theme.highlight_bg)
        .add_modifier(Modifier::BOLD);
    let normal = Style::default().fg(theme.muted);

    let mut lines = vec![
        Line::from(Span::styled(kind.prompt(), Style::default().fg(theme.fg))),
        Line::default(),
    ];
    let options = if kind == ConfirmKind::DiscardReview {
        ["Discard review", "Keep reviewing"]
    } else {
        CONFIRM_OPTIONS
    };
    for (i, label) in options.iter().enumerate() {
        let marker = if i == cursor { "▶ " } else { "  " };
        let style = if i == cursor { selected } else { normal };
        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(theme.accent)),
            Span::styled(format!(" {label} "), style),
        ]));
    }

    let selected_line = 2 + cursor;
    crate::tui::widgets::dialog::choices(frame, area, "Confirm", lines, selected_line);
}

fn key_to_action(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Left | KeyCode::Up | KeyCode::Char('h' | 'k') => {
            Some(Action::Detail(DetailAction::ConfirmMove(-1)))
        }
        KeyCode::Right | KeyCode::Down | KeyCode::Char('j' | 'l') => {
            Some(Action::Detail(DetailAction::ConfirmMove(1)))
        }
        KeyCode::Enter => Some(Action::Detail(DetailAction::SubmitConfirm)),
        KeyCode::Esc => Some(Action::Detail(DetailAction::CloseConfirm)),
        _ => None,
    }
}

#[derive(Debug)]
pub struct ConfirmDialog {
    kind: ConfirmKind,
    cursor: usize,
}
impl ConfirmDialog {
    pub fn new(kind: ConfirmKind) -> Self {
        Self {
            kind,
            cursor: usize::from(kind == ConfirmKind::DiscardReview),
        }
    }
    pub fn accepted(&self) -> Option<ConfirmKind> {
        (self.cursor == 0).then_some(self.kind)
    }
    #[cfg(test)]
    pub fn kind(&self) -> ConfirmKind {
        self.kind
    }
}

impl Component for ConfirmDialog {
    type Context<'a> = ();
    type Message = DetailAction;
    fn handle_key(&self, key: KeyEvent, (): &Self::Context<'_>) -> Option<Action> {
        key_to_action(key.code)
    }
    fn update(&mut self, action: DetailAction, (): &()) -> Option<Action> {
        match action {
            DetailAction::ConfirmMove(delta) => {
                self.cursor = step_index(self.cursor, delta, 2);
                None
            }
            other => Some(Action::Detail(other)),
        }
    }
    fn render(&mut self, frame: &mut Frame, area: Rect, (): &()) {
        render(frame, self.kind, self.cursor, area);
    }
}

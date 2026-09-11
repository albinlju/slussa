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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmKind {
    DeleteComment { id: u64, review: bool },
    Decline,
}

impl ConfirmKind {
    pub fn prompt(self) -> &'static str {
        match self {
            Self::DeleteComment { .. } => "Delete this comment?",
            Self::Decline => "Decline this PR?",
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
    for (i, label) in CONFIRM_OPTIONS.iter().enumerate() {
        let marker = if i == cursor { "▶ " } else { "  " };
        let style = if i == cursor { selected } else { normal };
        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(theme.accent)),
            Span::styled(format!(" {label} "), style),
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
        .title(" Confirm ")
        .border_style(Style::default().fg(theme.accent))
        .padding(Padding::horizontal(1));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    frame.render_widget(Paragraph::new(lines), inner);
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
        Self { kind, cursor: 0 }
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

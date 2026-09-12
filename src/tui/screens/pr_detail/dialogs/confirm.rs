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
            Self::Decline => "Close / decline this PR?",
            Self::DiscardReview => "Discard this review draft?",
        }
    }
}

const CONFIRM_OPTIONS: [&str; 2] = ["Yes", "No"];

fn render(
    frame: &mut Frame,
    kind: ConfirmKind,
    cursor: usize,
    context: &str,
    target_branch: Option<&str>,
    area: Rect,
) {
    let theme = theme::current();
    let selected = Style::default()
        .bg(theme.highlight_bg)
        .add_modifier(Modifier::BOLD);
    let normal = Style::default().fg(theme.muted);

    let mut lines = vec![
        Line::from(Span::styled(kind.prompt(), Style::default().fg(theme.fg))),
        Line::default(),
    ];
    if !context.is_empty() {
        for (index, line) in context.lines().take(3).enumerate() {
            lines.insert(
                1 + index,
                Line::styled(line.to_owned(), Style::default().fg(theme.muted)),
            );
        }
    }
    if let Some(branch) = target_branch {
        let index = lines.len() - 1;
        lines.insert(
            index,
            Line::from(vec![
                Span::styled("Target: ", normal),
                Span::styled(
                    branch.to_owned(),
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
        );
        lines.insert(index + 1, Line::styled("Closes without merging", normal));
    }
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

    let selected_line = lines.len() - 2 + cursor;
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
    context: String,
    target_branch: Option<String>,
}
impl ConfirmDialog {
    pub fn new(kind: ConfirmKind) -> Self {
        Self {
            kind,
            cursor: usize::from(matches!(
                kind,
                ConfirmKind::DiscardReview | ConfirmKind::Decline
            )),
            context: String::new(),
            target_branch: None,
        }
    }
    pub fn with_context(mut self, context: String) -> Self {
        self.context = context;
        self
    }
    pub fn set_pr_context(&mut self, context: String, target_branch: &str) {
        if self.kind == ConfirmKind::Decline {
            self.context = context;
            self.target_branch = Some(target_branch.to_owned());
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
        render(
            frame,
            self.kind,
            self.cursor,
            &self.context,
            self.target_branch.as_deref(),
            area,
        );
    }
}

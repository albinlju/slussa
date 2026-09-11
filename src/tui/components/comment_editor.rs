use crate::{
    app::{
        action::{Action, DetailAction},
        reviews::CommentTarget,
    },
    tui::{component::Component, theme},
};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

#[derive(Debug, Default)]
pub struct CommentEditor {
    pub draft: Option<CommentDraft>,
}
impl Component for CommentEditor {
    type Context<'a> = ();
    type Message = DetailAction;
    fn handle_key(&self, key: KeyEvent, (): &()) -> Option<Action> {
        self.draft.as_ref()?;
        let action = match key.code {
            KeyCode::Char(c) => DetailAction::CommentType(c),
            KeyCode::Backspace => DetailAction::CommentBackspace,
            KeyCode::Enter => DetailAction::CommentSubmit,
            KeyCode::Esc => DetailAction::CommentCancel,
            _ => return None,
        };
        Some(Action::Detail(action))
    }
    fn update(&mut self, action: DetailAction, (): &()) -> Option<Action> {
        match action {
            DetailAction::CommentType(c) => {
                if let Some(draft) = &mut self.draft {
                    draft.text.push(c);
                }
            }
            DetailAction::CommentBackspace => {
                if let Some(draft) = &mut self.draft {
                    draft.text.pop();
                }
            }
            DetailAction::CommentCancel => self.draft = None,
            other => return Some(Action::Detail(other)),
        }
        None
    }
    fn render(&mut self, frame: &mut Frame, area: Rect, (): &()) {
        if let Some(draft) = &self.draft {
            frame.render_widget(Paragraph::new(comment_prompt(draft)), area);
        }
    }
}

pub(crate) fn comment_prompt(draft: &CommentDraft) -> Line<'static> {
    let theme = theme::current();
    let label = match &draft.target {
        CommentTarget::Line(a) => format!("  comment {}:{} ▏ ", a.path, a.line),
        CommentTarget::Pr => "  comment ▏ ".to_owned(),
        CommentTarget::Reply(_) => "  reply ▏ ".to_owned(),
        CommentTarget::Edit { .. } => "  edit ▏ ".to_owned(),
        CommentTarget::Review { verdict } => format!("  {} ▏ ", verdict.label().to_lowercase()),
    };
    Line::from(vec![
        Span::styled(label, Style::default().fg(theme.muted)),
        Span::styled(draft.text.clone(), Style::default().fg(theme.fg)),
        Span::styled("█", Style::default().fg(theme.accent)),
    ])
}

#[derive(Debug, Clone)]
pub struct CommentDraft {
    pub target: CommentTarget,
    pub text: String,
}

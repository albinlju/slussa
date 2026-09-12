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

/// Cap a single-line string to `max` columns, adding an ellipsis when cut, to
/// keep the review popup from stretching to a long comment's width.
fn truncate_cols(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let head: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{head}…")
}

fn render(frame: &mut Frame, ctx: &ReviewContext<'_>, cursor: usize, area: Rect) {
    let theme = theme::current();
    let selected = Style::default()
        .bg(theme.highlight_bg)
        .add_modifier(Modifier::BOLD);
    let normal = Style::default().fg(theme.muted);

    let mut lines = vec![Line::from(Span::styled(
        "Submit review",
        Style::default().fg(theme.fg),
    ))];

    // Show what the review will carry, so finishing isn't a blind submit.
    if let Some(review) = ctx.pending
        && !review.comments.is_empty()
    {
        let n = review.comments.len();
        let noun = if n == 1 { "comment" } else { "comments" };
        lines.push(Line::default());
        lines.push(Line::from(Span::styled(
            format!("Including {n} {noun}:"),
            Style::default().fg(theme.muted),
        )));
        let preview = if area.height >= 20 { 2 } else { 0 };
        for pc in review.comments.iter().take(preview) {
            let first = pc.text.lines().next().unwrap_or("");
            let summary = format!("  {}:{}  {first}", pc.anchor.path, pc.anchor.line);
            lines.push(Line::from(Span::styled(
                truncate_cols(&summary, 50),
                Style::default().fg(theme.muted),
            )));
        }
    }

    lines.push(Line::default());
    for (i, (verdict, disabled)) in ctx.options.iter().enumerate() {
        let marker = if i == cursor { "▶ " } else { "  " };
        // A disabled verdict (e.g. approving your own PR) stays listed but dimmed,
        // with the reason inline, rather than being hidden.
        let label = match disabled {
            Some(reason) => format!(" {} ({reason}) ", verdict.label()),
            None => format!(" {} ", verdict.label()),
        };
        let style = if i == cursor && disabled.is_none() {
            selected
        } else {
            normal
        };
        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(theme.accent)),
            Span::styled(label, style),
        ]));
    }

    let selected_line = lines.len().saturating_sub(ctx.options.len()) + cursor;
    crate::tui::widgets::dialog::choices(frame, area, "Review", lines, selected_line);
}

fn key_to_action(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Left | KeyCode::Up | KeyCode::Char('h' | 'k') => {
            Some(Action::Detail(DetailAction::ReviewMove(-1)))
        }
        KeyCode::Right | KeyCode::Down | KeyCode::Char('j' | 'l') => {
            Some(Action::Detail(DetailAction::ReviewMove(1)))
        }
        KeyCode::Enter => Some(Action::Detail(DetailAction::ReviewSelect)),
        KeyCode::Esc => Some(Action::Detail(DetailAction::CloseReviewPicker)),
        _ => None,
    }
}

pub struct ReviewContext<'a> {
    pub options: Vec<(crate::domain::review::ReviewVerdict, Option<&'static str>)>,
    pub pending: Option<&'a crate::app::reviews::PendingReview>,
}
#[derive(Debug, Default)]
pub struct ReviewDialog {
    cursor: usize,
}
impl ReviewDialog {
    pub fn new(ctx: &ReviewContext<'_>) -> Self {
        Self {
            cursor: ctx
                .options
                .iter()
                .position(|(_, reason)| reason.is_none())
                .unwrap_or(0),
        }
    }
    pub fn selected(
        &self,
        ctx: &ReviewContext<'_>,
    ) -> Option<crate::domain::review::ReviewVerdict> {
        ctx.options
            .get(self.cursor)
            .filter(|(_, reason)| reason.is_none())
            .map(|(verdict, _)| *verdict)
    }
    #[cfg(test)]
    pub fn cursor(&self) -> usize {
        self.cursor
    }
}

impl Component for ReviewDialog {
    type Context<'a> = ReviewContext<'a>;
    type Message = DetailAction;
    fn handle_key(&self, key: KeyEvent, _: &Self::Context<'_>) -> Option<Action> {
        key_to_action(key.code)
    }
    fn update(&mut self, action: DetailAction, ctx: &ReviewContext<'_>) -> Option<Action> {
        match action {
            DetailAction::ReviewMove(delta) => {
                self.cursor = step_index(self.cursor, delta, ctx.options.len());
                None
            }
            other => Some(Action::Detail(other)),
        }
    }
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &ReviewContext<'_>) {
        render(frame, ctx, self.cursor, area);
    }
}

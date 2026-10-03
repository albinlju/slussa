use crate::tui::{
    app::effect::Effect,
    ui::{
        action::{Action, ReviewAction},
        component::{Component, saturating_u16, step_index},
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

fn render(frame: &mut Frame<'_>, ctx: &ReviewContext<'_>, dialog: &mut ReviewDialog, area: Rect) {
    let cursor = dialog.cursor;
    if dialog.mode == ReviewMode::Comments {
        let body = crate::tui::ui::widgets::dialog::frame(
            frame,
            area,
            "Review draft · not published",
            (76, area.height.saturating_sub(6)),
            &[("j/k", "scroll"), ("Tab", "back"), ("Esc", "close")],
        );
        let mut lines = Vec::new();
        if let Some(review) = ctx.pending {
            for (index, comment) in review.comments.iter().enumerate() {
                if index > 0 {
                    lines.push(Line::default());
                }
                let heading = format!(
                    "{}. {}:{}",
                    index + 1,
                    comment.anchor.path,
                    comment.anchor.line
                );
                for line in crate::tui::ui::widgets::wrap_text(&heading, body.width as usize) {
                    lines.push(Line::styled(
                        line,
                        Style::default().fg(theme::current().link),
                    ));
                }
                for line in crate::tui::ui::widgets::wrap_text(&comment.text, body.width as usize) {
                    lines.push(Line::styled(line, Style::default().fg(theme::current().fg)));
                }
            }
        }
        dialog.scroll = dialog.scroll.min(saturating_u16(
            lines.len().saturating_sub(body.height as usize),
        ));
        frame.render_widget(
            ratatui::widgets::Paragraph::new(lines).scroll((dialog.scroll, 0)),
            body,
        );
        return;
    }
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
    let action = dialog.selected(ctx).map_or("unavailable", |verdict| {
        if verdict.needs_body() {
            "continue"
        } else {
            "submit review"
        }
    });
    let mut hints = vec![("j/k", "move"), ("Enter", action), ("Esc", "cancel")];
    if ctx
        .pending
        .is_some_and(|review| !review.comments.is_empty())
    {
        hints.insert(0, ("Tab", "inspect comments"));
    }
    if area.width < 44 {
        hints = vec![
            (
                "Enter",
                if action == "submit review" {
                    "submit"
                } else {
                    action
                },
            ),
            ("Esc", "cancel"),
        ];
        if ctx
            .pending
            .is_some_and(|review| !review.comments.is_empty())
        {
            hints.insert(0, ("Tab", "comments"));
        }
    }
    crate::tui::ui::widgets::dialog::choices_with_hints(
        frame,
        area,
        "Review",
        lines,
        selected_line,
        &hints,
    );
}

const fn key_to_action(code: KeyCode) -> Option<ReviewAction> {
    match code {
        KeyCode::Left | KeyCode::Up | KeyCode::Char('h' | 'k') => Some(ReviewAction::Move(-1)),
        KeyCode::Right | KeyCode::Down | KeyCode::Char('j' | 'l') => Some(ReviewAction::Move(1)),
        KeyCode::Enter => Some(ReviewAction::Select),
        KeyCode::Esc => Some(ReviewAction::Close),
        _ => None,
    }
}

pub struct ReviewContext<'a> {
    pub options: Vec<(crate::domain::review::ReviewVerdict, Option<&'static str>)>,
    pub pending: Option<&'a crate::domain::review::PendingReview>,
}
/// Which side of the dialog is showing. Tab switches.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum ReviewMode {
    /// The verdicts to choose from.
    #[default]
    Verdicts,
    /// The queued comments, to read before submitting.
    Comments,
}

#[derive(Debug, Default)]
pub struct ReviewDialog {
    cursor: usize,
    mode: ReviewMode,
    /// How far the comments are scrolled; kept while the dialog is open.
    scroll: u16,
}
impl ReviewDialog {
    pub fn new(ctx: &ReviewContext<'_>) -> Self {
        Self {
            cursor: ctx
                .options
                .iter()
                .position(|(_, reason)| reason.is_none())
                .unwrap_or(0),
            ..Self::default()
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
    pub const fn cursor(&self) -> usize {
        self.cursor
    }
}

impl Component for ReviewDialog {
    type Input<'a> = ReviewContext<'a>;
    type View<'a> = ReviewContext<'a>;
    type Message = ReviewAction;
    fn handle_key(&self, key: KeyEvent, ctx: &ReviewContext<'_>) -> Option<Action> {
        if key.code == KeyCode::Tab && ctx.pending.is_some_and(|r| !r.comments.is_empty()) {
            return Some(ReviewAction::Preview.into());
        }
        let action = key_to_action(key.code)?;
        match (self.mode, action) {
            // Nothing is submitted from the list of comments.
            (ReviewMode::Comments, ReviewAction::Select) => None,
            (ReviewMode::Verdicts | ReviewMode::Comments, action) => Some(action.into()),
        }
    }
    fn update(&mut self, action: ReviewAction, ctx: &ReviewContext<'_>) -> Option<Effect> {
        match action {
            ReviewAction::Preview => {
                self.mode = match self.mode {
                    ReviewMode::Verdicts => ReviewMode::Comments,
                    ReviewMode::Comments => ReviewMode::Verdicts,
                };
            }
            ReviewAction::Move(delta) => match self.mode {
                ReviewMode::Verdicts => {
                    self.cursor = step_index(self.cursor, delta, ctx.options.len());
                }
                ReviewMode::Comments => {
                    self.scroll = crate::tui::ui::component::scroll(self.scroll, delta);
                }
            },
            // Closing and choosing are the screen's: it holds the dialog.
            ReviewAction::Select | ReviewAction::Close => {}
        }
        None
    }
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, ctx: &ReviewContext<'_>) {
        render(frame, ctx, self, area);
    }
}

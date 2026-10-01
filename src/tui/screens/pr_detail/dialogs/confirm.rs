use crate::{
    app::action::{Action, ConfirmAction, Effect},
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
    DeleteComment {
        id: u64,
        review: bool,
    },
    Decline,
    /// Reopen a PR that was closed without merging.
    Reopen,
    DiscardReview,
}

impl ConfirmKind {
    pub const fn prompt(self) -> &'static str {
        match self {
            Self::DeleteComment { .. } => "Delete this comment?",
            Self::Decline => "Close / decline this PR?",
            Self::Reopen => "Reopen this PR?",
            Self::DiscardReview => "Discard this review draft?",
        }
    }
}

const CONFIRM_OPTIONS: [&str; 2] = ["Yes", "No"];

fn render(
    frame: &mut Frame<'_>,
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

const fn key_to_action(code: KeyCode) -> Option<ConfirmAction> {
    match code {
        KeyCode::Left | KeyCode::Up | KeyCode::Char('h' | 'k') => Some(ConfirmAction::Move(-1)),
        KeyCode::Right | KeyCode::Down | KeyCode::Char('j' | 'l') => Some(ConfirmAction::Move(1)),
        KeyCode::Enter => Some(ConfirmAction::Accept),
        KeyCode::Esc => Some(ConfirmAction::Close),
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
        match self.kind {
            ConfirmKind::Decline => {
                self.context = context;
                self.target_branch = Some(target_branch.to_owned());
            }
            // Reopening states no target: nothing is lost or merged by it.
            ConfirmKind::Reopen => self.context = context,
            ConfirmKind::DeleteComment { .. } | ConfirmKind::DiscardReview => {}
        }
    }
    pub fn accepted(&self) -> Option<ConfirmKind> {
        (self.cursor == 0).then_some(self.kind)
    }
    #[cfg(test)]
    pub const fn kind(&self) -> ConfirmKind {
        self.kind
    }
}

impl Component for ConfirmDialog {
    type Input<'a> = ();
    type View<'a> = ();
    type Message = ConfirmAction;
    fn handle_key(&self, key: KeyEvent, (): &()) -> Option<Action> {
        key_to_action(key.code).map(Action::from)
    }
    fn update(&mut self, action: ConfirmAction, (): &()) -> Option<Effect> {
        match action {
            ConfirmAction::Move(delta) => self.cursor = step_index(self.cursor, delta, 2),
            // Closing and accepting are the screen's: it holds the dialog.
            ConfirmAction::Accept | ConfirmAction::Close => {}
        }
        None
    }
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, (): &()) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reopening_defaults_to_yes_and_closing_defaults_to_no() {
        assert_eq!(
            ConfirmDialog::new(ConfirmKind::Reopen).accepted(),
            Some(ConfirmKind::Reopen),
            "reopening is routine and easy to undo"
        );
        assert_eq!(ConfirmDialog::new(ConfirmKind::Decline).accepted(), None);
        assert_eq!(
            ConfirmDialog::new(ConfirmKind::DiscardReview).accepted(),
            None
        );
    }

    fn drawn(dialog: &mut ConfirmDialog) -> String {
        use ratatui::{Terminal, backend::TestBackend};
        let mut terminal = Terminal::new(TestBackend::new(70, 20)).unwrap();
        terminal
            .draw(|frame| dialog.render(frame, frame.area(), &()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..20)
            .map(|y| (0..70).map(|x| buffer[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_reopen_dialog_names_the_pr_and_does_not_talk_about_closing() {
        let mut dialog = ConfirmDialog::new(ConfirmKind::Reopen);
        dialog.set_pr_context("PR #7 · Fix it".into(), "main");
        let text = drawn(&mut dialog);

        assert!(text.contains("Reopen this PR?"), "{text}");
        assert!(text.contains("PR #7 · Fix it"), "{text}");
        assert!(text.contains("Yes") && text.contains("No"), "{text}");
        assert!(!text.contains("Closes without merging"), "{text}");
        assert!(!text.contains("Target:"), "{text}");
    }

    #[test]
    fn reopening_shows_the_pr_but_no_closing_target() {
        let mut dialog = ConfirmDialog::new(ConfirmKind::Reopen);
        dialog.set_pr_context("PR #7 · Fix it".into(), "main");
        assert_eq!(dialog.context, "PR #7 · Fix it");
        assert_eq!(dialog.target_branch, None);

        let mut decline = ConfirmDialog::new(ConfirmKind::Decline);
        decline.set_pr_context("PR #7 · Fix it".into(), "main");
        assert_eq!(decline.target_branch.as_deref(), Some("main"));
        assert_eq!(ConfirmKind::Reopen.prompt(), "Reopen this PR?");
    }
}

use crate::{
    tui::app::action::{Action, ConfirmAction, Effect},
    tui::ui::{component::Component, screens::pr_detail::dialogs::PrSummary, theme},
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
    DeleteComment(crate::domain::comment::CommentKey),
    Decline,
    /// Reopen a PR that was closed without merging.
    Reopen,
    DiscardReview,
}

impl ConfirmKind {
    pub const fn prompt(self) -> &'static str {
        match self {
            Self::DeleteComment(_) => "Delete this comment?",
            Self::Decline => "Close / decline this PR?",
            Self::Reopen => "Reopen this PR?",
            Self::DiscardReview => "Discard this review draft?",
        }
    }
}

/// The two answers. The default differs by what is asked: see
/// `ConfirmKind::default_choice`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    Yes,
    No,
}

impl ConfirmKind {
    /// What Enter does if nothing is moved. Yes only where the action is
    /// routine and easy to undo; a new kind has to say which it is.
    const fn default_choice(self) -> Choice {
        match self {
            Self::Reopen | Self::DeleteComment(_) => Choice::Yes,
            Self::Decline | Self::DiscardReview => Choice::No,
        }
    }

    const fn labels(self) -> [&'static str; 2] {
        match self {
            Self::DiscardReview => ["Discard review", "Keep reviewing"],
            Self::DeleteComment(_) | Self::Decline | Self::Reopen => ["Yes", "No"],
        }
    }
}

fn render(frame: &mut Frame<'_>, dialog: &ConfirmDialog, pr: &PrSummary<'_>, area: Rect) {
    let theme = theme::current();
    let selected = Style::default()
        .bg(theme.highlight_bg)
        .add_modifier(Modifier::BOLD);
    let normal = Style::default().fg(theme.muted);
    let kind = dialog.kind;
    // What the question is about: the PR, or the comment to delete.
    let (context, target_branch) = match kind {
        ConfirmKind::Decline => (Some(pr.label.as_str()), Some(pr.target_branch)),
        // Reopening states no target: nothing is lost or merged by it.
        ConfirmKind::Reopen => (Some(pr.label.as_str()), None),
        ConfirmKind::DeleteComment(_) => (dialog.preview.as_deref(), None),
        ConfirmKind::DiscardReview => (None, None),
    };

    let mut lines = vec![
        Line::from(Span::styled(kind.prompt(), Style::default().fg(theme.fg))),
        Line::default(),
    ];
    for (index, line) in context.into_iter().flat_map(str::lines).take(3).enumerate() {
        lines.insert(
            1 + index,
            Line::styled(line.to_owned(), Style::default().fg(theme.muted)),
        );
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
    let cursor = match dialog.choice {
        Choice::Yes => 0,
        Choice::No => 1,
    };
    for (i, label) in kind.labels().iter().enumerate() {
        let marker = if i == cursor { "▶ " } else { "  " };
        let style = if i == cursor { selected } else { normal };
        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(theme.accent)),
            Span::styled(format!(" {label} "), style),
        ]));
    }

    let selected_line = lines.len() - 2 + cursor;
    crate::tui::ui::widgets::dialog::choices(frame, area, "Confirm", lines, selected_line);
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
    choice: Choice,
    /// The comment a delete is asked about.
    preview: Option<String>,
}
impl ConfirmDialog {
    pub const fn new(kind: ConfirmKind) -> Self {
        Self {
            kind,
            choice: kind.default_choice(),
            preview: None,
        }
    }
    pub fn with_preview(mut self, preview: String) -> Self {
        self.preview = Some(preview);
        self
    }
    pub fn accepted(&self) -> Option<ConfirmKind> {
        (self.choice == Choice::Yes).then_some(self.kind)
    }
    #[cfg(test)]
    pub const fn kind(&self) -> ConfirmKind {
        self.kind
    }
}

impl Component for ConfirmDialog {
    type Input<'a> = ();
    /// The PR the question is asked on.
    type View<'a> = PrSummary<'a>;
    type Message = ConfirmAction;
    fn handle_key(&self, key: KeyEvent, (): &()) -> Option<Action> {
        key_to_action(key.code).map(Action::from)
    }
    fn update(&mut self, action: ConfirmAction, (): &()) -> Option<Effect> {
        match action {
            ConfirmAction::Move(delta) => {
                self.choice = match delta.signum() {
                    -1 => Choice::Yes,
                    1 => Choice::No,
                    _ => self.choice,
                };
            }
            // Closing and accepting are the screen's: it holds the dialog.
            ConfirmAction::Accept | ConfirmAction::Close => {}
        }
        None
    }
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, pr: &PrSummary<'_>) {
        render(frame, self, pr, area);
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
            .draw(|frame| {
                let pr = PrSummary {
                    label: "PR #7 · Fix it".into(),
                    target_branch: "main",
                    source_branch: "feature",
                };
                dialog.render(frame, frame.area(), &pr);
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..20)
            .map(|y| (0..70).map(|x| buffer[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_reopen_dialog_names_the_pr_and_does_not_talk_about_closing() {
        let text = drawn(&mut ConfirmDialog::new(ConfirmKind::Reopen));

        assert!(text.contains("Reopen this PR?"), "{text}");
        assert!(text.contains("PR #7 · Fix it"), "{text}");
        assert!(text.contains("Yes") && text.contains("No"), "{text}");
        assert!(!text.contains("Closes without merging"), "{text}");
        assert!(!text.contains("Target:"), "{text}");
    }

    #[test]
    fn declining_names_the_target_and_a_delete_shows_the_comment_not_the_pr() {
        let text = drawn(&mut ConfirmDialog::new(ConfirmKind::Decline));
        assert!(text.contains("PR #7 · Fix it"), "{text}");
        assert!(text.contains("Target: main"), "{text}");
        assert!(text.contains("Closes without merging"), "{text}");

        let kind = ConfirmKind::DeleteComment(crate::domain::comment::CommentKey {
            id: crate::domain::comment::CommentId(3),
            kind: crate::domain::comment::CommentKind::Conversation,
        });
        let text = drawn(&mut ConfirmDialog::new(kind).with_preview("@ann: typo".into()));
        assert!(text.contains("@ann: typo"), "{text}");
        assert!(!text.contains("PR #7"), "{text}");
        assert_eq!(ConfirmKind::Reopen.prompt(), "Reopen this PR?");
    }

    #[test]
    fn moving_picks_an_answer_and_only_yes_accepts() {
        let mut dialog = ConfirmDialog::new(ConfirmKind::Decline);
        assert_eq!(dialog.accepted(), None);
        dialog.update(ConfirmAction::Move(-1), &());
        assert_eq!(dialog.accepted(), Some(ConfirmKind::Decline));
        dialog.update(ConfirmAction::Move(-1), &());
        assert_eq!(dialog.accepted(), Some(ConfirmKind::Decline));
        dialog.update(ConfirmAction::Move(5), &());
        assert_eq!(dialog.accepted(), None);
    }
}

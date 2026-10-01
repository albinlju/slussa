use crate::{
    app::action::{Action, Effect, MergeAction},
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
    if !dialog.blockers.is_empty() {
        lines.push(Line::default());
        lines.push(Line::styled(
            "Blocked by:",
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ));
        for reason in &dialog.blockers {
            lines.push(Line::styled(
                format!("  • {reason}"),
                Style::default().fg(theme.warning),
            ));
        }
        lines.push(Line::styled(
            "You can still try; the server decides.",
            normal,
        ));
        lines.push(Line::default());
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

const fn key_to_action(code: KeyCode) -> Option<MergeAction> {
    match code {
        KeyCode::Left | KeyCode::Up | KeyCode::Char('h' | 'k') => Some(MergeAction::Move(-1)),
        KeyCode::Right | KeyCode::Down | KeyCode::Char('j' | 'l') => Some(MergeAction::Move(1)),
        KeyCode::Enter => Some(MergeAction::Select),
        KeyCode::Esc => Some(MergeAction::Close),
        _ => None,
    }
}

#[derive(Debug, Default)]
pub struct MergeDialog {
    cursor: usize,
    pub pr_label: String,
    pub target_branch: String,
    pub source_branch: String,
    /// Why the provider says this PR cannot be merged yet; empty when nothing
    /// is known to stand in the way.
    pub blockers: Vec<String>,
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
    type Message = MergeAction;
    fn handle_key(&self, key: KeyEvent, _: &Self::Context<'_>) -> Option<Action> {
        key_to_action(key.code).map(Action::from)
    }
    fn update(&mut self, action: MergeAction, ctx: &Self::Context<'_>) -> Option<Effect> {
        match action {
            MergeAction::Move(delta) => self.cursor = step_index(self.cursor, delta, ctx.len()),
            // Closing and merging are the screen's: it holds the dialog.
            MergeAction::Select | MergeAction::Close => {}
        }
        None
    }
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, ctx: &Self::Context<'_>) {
        render(frame, ctx, self, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::pr::MergeStrategy;
    use ratatui::{Terminal, backend::TestBackend};

    fn drawn(dialog: &mut MergeDialog) -> String {
        let strategies = [MergeStrategy::Merge, MergeStrategy::Squash];
        let mut terminal = Terminal::new(TestBackend::new(90, 28)).unwrap();
        terminal
            .draw(|frame| dialog.render(frame, frame.area(), &strategies.as_slice()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..28)
            .map(|y| (0..90).map(|x| buffer[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn blockers_are_listed_above_the_choices_and_the_choices_stay_usable() {
        let mut dialog = MergeDialog {
            blockers: vec![
                "An approving review is required.".into(),
                "Required checks have not passed.".into(),
            ],
            ..MergeDialog::default()
        };
        let text = drawn(&mut dialog);

        assert!(text.contains("Blocked by:"), "{text}");
        assert!(
            text.contains("• An approving review is required."),
            "{text}"
        );
        assert!(
            text.contains("• Required checks have not passed."),
            "{text}"
        );
        assert!(text.contains("the server decides"), "{text}");
        assert!(text.contains("Merge commit"), "{text}");
        assert!(text.contains("Squash and merge"), "{text}");
        assert!(text.contains("Enter"), "the footer stays visible: {text}");
    }

    #[test]
    fn a_clean_merge_shows_no_blocker_section() {
        let text = drawn(&mut MergeDialog::default());
        assert!(!text.contains("Blocked by"), "{text}");
        assert!(text.contains("Merge commit"), "{text}");
    }
}

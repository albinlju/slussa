use crate::{
    tui::app::action::{Action, Effect, MergeAction},
    tui::ui::{
        component::{Component, step_index},
        screens::pr_detail::dialogs::PrSummary,
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

/// What the merge dialog shows besides its own selection.
pub struct MergeView<'a> {
    pub strategies: &'a [crate::domain::pr::MergeStrategy],
    pub pr: PrSummary<'a>,
    /// Why the provider says this PR cannot be merged yet; empty when nothing
    /// is known to stand in the way.
    pub blockers: &'a [String],
}

fn render(frame: &mut Frame<'_>, view: &MergeView<'_>, dialog: &MergeDialog, area: Rect) {
    let strategies = view.strategies;
    let theme = theme::current();
    let selected = Style::default()
        .bg(theme.highlight_bg)
        .add_modifier(Modifier::BOLD);
    let normal = Style::default().fg(theme.muted);

    let mut lines = vec![
        Line::from(Span::styled("Merge this PR", Style::default().fg(theme.fg))),
        Line::styled(view.pr.label.clone(), normal),
        Line::from(vec![
            Span::styled("Into: ", normal),
            Span::styled(
                view.pr.target_branch.to_owned(),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("From: ", normal),
            Span::styled(
                view.pr.source_branch.to_owned(),
                Style::default().fg(theme.orange),
            ),
        ]),
        Line::default(),
    ];
    if !view.blockers.is_empty() {
        lines.push(Line::default());
        lines.push(Line::styled(
            "Blocked by:",
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ));
        for reason in view.blockers {
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
    crate::tui::ui::widgets::dialog::choices_with_hints(
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
    /// The strategies on offer.
    type Input<'a> = &'a [crate::domain::pr::MergeStrategy];
    type View<'a> = MergeView<'a>;
    type Message = MergeAction;
    fn handle_key(&self, key: KeyEvent, _: &Self::Input<'_>) -> Option<Action> {
        key_to_action(key.code).map(Action::from)
    }
    fn update(&mut self, action: MergeAction, ctx: &Self::Input<'_>) -> Option<Effect> {
        match action {
            MergeAction::Move(delta) => self.cursor = step_index(self.cursor, delta, ctx.len()),
            // Closing and merging are the screen's: it holds the dialog.
            MergeAction::Select | MergeAction::Close => {}
        }
        None
    }
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, view: &MergeView<'_>) {
        render(frame, view, self, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::pr::MergeStrategy;
    use ratatui::{Terminal, backend::TestBackend};

    fn drawn(blockers: &[String]) -> String {
        let view = MergeView {
            strategies: &[MergeStrategy::Merge, MergeStrategy::Squash],
            pr: PrSummary {
                label: "PR #7 · Fix it".into(),
                target_branch: "main",
                source_branch: "feature",
            },
            blockers,
        };
        let mut terminal = Terminal::new(TestBackend::new(90, 28)).unwrap();
        terminal
            .draw(|frame| MergeDialog::default().render(frame, frame.area(), &view))
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..28)
            .map(|y| (0..90).map(|x| buffer[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn blockers_are_listed_above_the_choices_and_the_choices_stay_usable() {
        let text = drawn(&[
            "An approving review is required.".into(),
            "Required checks have not passed.".into(),
        ]);

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
        let text = drawn(&[]);
        assert!(!text.contains("Blocked by"), "{text}");
        assert!(text.contains("PR #7 · Fix it"), "{text}");
        assert!(text.contains("Into: main"), "{text}");
        assert!(text.contains("From: feature"), "{text}");
        assert!(text.contains("Merge commit"), "{text}");
    }
}

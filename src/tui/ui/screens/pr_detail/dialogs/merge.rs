use crate::{
    domain::{
        capabilities::{Capabilities, Feature},
        pr::{DeletableBranch, MergeStrategy, Mergeability},
    },
    tui::{
        app::effect::Effect,
        ui::{
            action::{Action, MergeAction},
            component::{Component, step_index},
            screens::pr_detail::dialogs::PrSummary,
            theme,
        },
    },
};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

/// What the PR allows besides merging now: merging by itself once it is ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoMergeOffer {
    Unavailable,
    Available,
    On(MergeStrategy),
}

impl AutoMergeOffer {
    pub fn of(caps: &Capabilities, mergeability: Option<&Mergeability>) -> Self {
        if !caps.supports(Feature::AutoMerge) {
            return Self::Unavailable;
        }
        match mergeability {
            Some(Mergeability::AutoMerge { strategy, .. }) => Self::On(*strategy),
            Some(Mergeability::Blocked(_)) => Self::Available,
            // Not known to wait on anything, so there may be nothing to wait for.
            Some(Mergeability::Mergeable | Mergeability::Conflicts(_) | Mergeability::Unknown)
            | None => Self::Unavailable,
        }
    }
}

/// What the merge dialog shows besides its own selection.
pub struct MergeView<'a> {
    pub auto: AutoMergeOffer,
    pub strategies: &'a [MergeStrategy],
    pub pr: PrSummary<'a>,
    /// Why the provider says this PR cannot be merged yet; empty when nothing
    /// is known to stand in the way.
    pub blockers: &'a [String],
    /// The branch the merge may delete, when the provider and the PR allow it.
    pub branch: Option<&'a DeletableBranch>,
}

/// Whether the box says the branch goes: asked for, and there is one to delete.
const fn delete_offered(view: &MergeView<'_>, dialog: &MergeDialog) -> bool {
    dialog.delete_branch && view.branch.is_some()
}

fn render(frame: &mut Frame<'_>, view: &MergeView<'_>, dialog: &MergeDialog, area: Rect) {
    let strategies = view.strategies;
    let theme = theme::current();
    let selected = Style::default()
        .bg(theme.highlight_bg)
        .add_modifier(Modifier::BOLD);
    let normal = Style::default().fg(theme.muted);

    let when_ready = dialog.when_ready && view.auto == AutoMergeOffer::Available;
    let title = if when_ready {
        "Merge this PR when it is ready"
    } else {
        "Merge this PR"
    };
    let mut lines = vec![
        Line::from(Span::styled(title, Style::default().fg(theme.fg))),
        Line::styled(view.pr.label.clone(), normal),
        Line::default(),
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
    if let AutoMergeOffer::On(strategy) = view.auto {
        lines.push(Line::styled(
            format!("Merges by itself when ready: {}", strategy.label()),
            Style::default().fg(theme.info),
        ));
    }
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
    let deleting = delete_offered(view, dialog);
    if let Some(branch) = view.branch.filter(|_| !when_ready) {
        lines.push(Line::from(vec![
            Span::styled(if deleting { "[x] " } else { "[ ] " }, normal),
            Span::styled(format!("Delete {} after the merge", branch.name()), normal),
        ]));
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
    let mut hints = vec![
        ("j/k", "move"),
        (
            "Enter",
            if when_ready {
                "merge when ready"
            } else {
                "merge"
            },
        ),
    ];
    match view.auto {
        AutoMergeOffer::Unavailable => {}
        AutoMergeOffer::Available => {
            hints.push((
                "a",
                if when_ready {
                    "merge now"
                } else {
                    "when ready"
                },
            ));
        }
        AutoMergeOffer::On(_) => hints.push(("a", "turn off auto-merge")),
    }
    if view.branch.is_some() && !when_ready {
        hints.push((
            "d",
            if deleting {
                "keep branch"
            } else {
                "delete branch"
            },
        ));
    }
    hints.push(("Esc", "cancel"));
    crate::tui::ui::widgets::dialog::choices_with_room(
        frame,
        area,
        "Merge",
        lines,
        selected_line,
        &hints,
        crate::tui::ui::widgets::dialog::Room::Roomy,
    );
}

const fn key_to_action(code: KeyCode) -> Option<MergeAction> {
    match code {
        KeyCode::Left | KeyCode::Up | KeyCode::Char('h' | 'k') => Some(MergeAction::Move(-1)),
        KeyCode::Right | KeyCode::Down | KeyCode::Char('j' | 'l') => Some(MergeAction::Move(1)),
        KeyCode::Char('a') => Some(MergeAction::Auto),
        KeyCode::Char('d') => Some(MergeAction::DeleteBranch),
        KeyCode::Enter => Some(MergeAction::Select),
        KeyCode::Esc => Some(MergeAction::Close),
        _ => None,
    }
}

#[derive(Debug, Default)]
pub struct MergeDialog {
    cursor: usize,
    /// Enter merges by itself once the PR is ready, not now.
    pub when_ready: bool,
    /// Enter deletes the source branch after merging it.
    pub delete_branch: bool,
}
impl MergeDialog {
    pub fn selected(&self, strategies: &[MergeStrategy]) -> Option<MergeStrategy> {
        strategies.get(self.cursor).copied()
    }
}

impl Component for MergeDialog {
    /// The strategies on offer.
    type Input<'a> = &'a [MergeStrategy];
    type View<'a> = MergeView<'a>;
    type Message = MergeAction;
    fn handle_key(&self, key: KeyEvent, _: &Self::Input<'_>) -> Option<Action> {
        key_to_action(key.code).map(Action::from)
    }
    fn update(&mut self, action: MergeAction, ctx: &Self::Input<'_>) -> Option<Effect> {
        match action {
            MergeAction::Move(delta) => self.cursor = step_index(self.cursor, delta, ctx.len()),
            // Closing and merging are the screen's: it holds the dialog.
            MergeAction::Select
            | MergeAction::Close
            | MergeAction::Auto
            | MergeAction::DeleteBranch => {}
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
    use ratatui::{Terminal, backend::TestBackend};

    fn drawn(blockers: &[String]) -> String {
        drawn_with(blockers, AutoMergeOffer::Unavailable, false)
    }

    fn drawn_with(blockers: &[String], auto: AutoMergeOffer, when_ready: bool) -> String {
        drawn_full(blockers, auto, when_ready, None, false)
    }

    fn drawn_full(
        blockers: &[String],
        auto: AutoMergeOffer,
        when_ready: bool,
        branch: Option<&DeletableBranch>,
        delete_branch: bool,
    ) -> String {
        let view = MergeView {
            auto,
            strategies: &[MergeStrategy::Merge, MergeStrategy::Squash],
            pr: PrSummary {
                label: "PR #7 · Fix it".into(),
                target_branch: "main",
                source_branch: "feature",
            },
            blockers,
            branch,
        };
        let mut terminal = Terminal::new(TestBackend::new(90, 28)).unwrap();
        terminal
            .draw(|frame| {
                MergeDialog {
                    when_ready,
                    delete_branch,
                    ..MergeDialog::default()
                }
                .render(frame, frame.area(), &view);
            })
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

    #[test]
    fn when_ready_is_offered_only_where_the_pr_waits_and_the_provider_can() {
        let caps = |on: bool| Capabilities {
            features: on.then_some(Feature::AutoMerge).into_iter().collect(),
            ..Capabilities::default()
        };
        let waiting = Mergeability::Blocked(vec!["checks".into()]);
        assert_eq!(
            AutoMergeOffer::of(&caps(true), Some(&waiting)),
            AutoMergeOffer::Available
        );
        assert_eq!(
            AutoMergeOffer::of(&caps(false), Some(&waiting)),
            AutoMergeOffer::Unavailable
        );
        for settled in [
            Mergeability::Mergeable,
            Mergeability::Conflicts(vec![]),
            Mergeability::Unknown,
        ] {
            assert_eq!(
                AutoMergeOffer::of(&caps(true), Some(&settled)),
                AutoMergeOffer::Unavailable
            );
        }
        let on = Mergeability::AutoMerge {
            strategy: MergeStrategy::Squash,
            waiting: vec![],
        };
        assert_eq!(
            AutoMergeOffer::of(&caps(true), Some(&on)),
            AutoMergeOffer::On(MergeStrategy::Squash)
        );
    }

    #[test]
    fn the_dialog_says_what_a_is_for_in_each_state() {
        let blocked = ["Required checks have not passed.".to_owned()];
        let available = drawn_with(&blocked, AutoMergeOffer::Available, false);
        assert!(available.contains("a when ready"), "{available}");

        let armed = drawn_with(&blocked, AutoMergeOffer::Available, true);
        assert!(armed.contains("Merge this PR when it is ready"), "{armed}");
        assert!(armed.contains("Enter merge when ready"), "{armed}");

        let on = drawn_with(&blocked, AutoMergeOffer::On(MergeStrategy::Squash), false);
        assert!(
            on.contains("Merges by itself when ready: Squash and merge"),
            "{on}"
        );
        assert!(on.contains("a turn off auto-merge"), "{on}");

        assert!(
            !drawn(&blocked).contains("a: "),
            "no a where it does nothing"
        );
    }

    fn own_branch() -> DeletableBranch {
        use crate::domain::pr::{PullRequest, SourceRepo};
        DeletableBranch::of(&PullRequest {
            source_branch: "feature".into(),
            source_repo: SourceRepo::Same,
            target_branch: "main".into(),
            ..PullRequest::for_test(7, chrono::Utc::now())
        })
        .expect("a branch of this repository")
    }

    #[test]
    fn the_dialog_offers_to_delete_the_branch_only_where_there_is_one_to_delete() {
        let branch = own_branch();
        let off = drawn_full(
            &[],
            AutoMergeOffer::Unavailable,
            false,
            Some(&branch),
            false,
        );
        assert!(off.contains("[ ] Delete feature after the merge"), "{off}");
        assert!(off.contains("d delete branch"), "{off}");

        let on = drawn_full(&[], AutoMergeOffer::Unavailable, false, Some(&branch), true);
        assert!(on.contains("[x] Delete feature after the merge"), "{on}");
        assert!(on.contains("d keep branch"), "{on}");

        let none = drawn_full(&[], AutoMergeOffer::Unavailable, false, None, true);
        assert!(!none.contains("Delete feature"), "{none}");
        assert!(!none.contains("d delete branch"), "{none}");
        assert!(!none.contains("d keep branch"), "{none}");
    }

    #[test]
    fn merging_when_ready_leaves_the_branch_to_the_repository() {
        let branch = own_branch();
        let armed = drawn_full(
            &["Checks.".to_owned()],
            AutoMergeOffer::Available,
            true,
            Some(&branch),
            true,
        );
        assert!(!armed.contains("Delete feature"), "{armed}");
        assert!(!armed.contains("d keep branch"), "{armed}");
    }

    #[test]
    fn the_dialog_is_at_least_sixty_wide_and_sets_the_pr_apart_from_its_branches() {
        let text = drawn(&[]);
        let top = text
            .lines()
            .find(|line| line.contains("╭ Merge"))
            .expect("the dialog's top edge");
        let edge: String = top.chars().skip_while(|c| *c != '╭').collect();
        assert!(
            edge.chars().count() >= 60,
            "{} wide: {text}",
            edge.chars().count()
        );

        let lines: Vec<&str> = text.lines().collect();
        let label = lines
            .iter()
            .position(|line| line.contains("PR #7 · Fix it"))
            .expect("the PR label");
        assert!(
            lines[label + 1]
                .trim_matches(|c: char| c == '│' || c.is_whitespace())
                .is_empty(),
            "an empty line follows the label: {text}"
        );
        assert!(lines[label + 2].contains("Into: main"), "{text}");
    }
}

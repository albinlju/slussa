//! Choosing which of the issues a PR closes to open in the browser.

use crate::{
    domain::pr::LinkedIssue,
    tui::{
        app::effect::Effect,
        ui::{
            action::{Action, IssueAction},
            component::{Component, step_index},
            theme,
            widgets::dialog::{Room, choices_with_room},
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

/// What the dialog is drawn from: the PR it is about, and the issues it can open.
pub struct IssuesView<'a> {
    pub pr_label: String,
    /// The repository the PR is in as its address says, to tell the issues of
    /// another one.
    pub pr_url: Option<&'a str>,
    pub issues: &'a [(&'a LinkedIssue, &'a str)],
}

/// `owner/repo` of a GitHub address, which is what `https://host/owner/repo/…`
/// says first.
fn repo_of(url: &str) -> Option<&str> {
    let rest = url.split_once("://")?.1;
    let path = rest.split_once('/')?.1;
    let mut parts = path.splitn(3, '/');
    let (owner, repo) = (parts.next()?, parts.next()?);
    let end = owner.len() + 1 + repo.len();
    path.get(..end)
        .filter(|_| !owner.is_empty() && !repo.is_empty())
}

#[derive(Debug, Default)]
pub struct IssueDialog {
    cursor: usize,
}

impl IssueDialog {
    /// The issue under the cursor, among those on offer.
    pub fn selected<'a, 'b>(
        &self,
        issues: &'b [(&'a LinkedIssue, &'a str)],
    ) -> Option<&'b (&'a LinkedIssue, &'a str)> {
        issues.get(self.cursor)
    }
}

const fn key_to_action(code: KeyCode) -> Option<IssueAction> {
    match code {
        KeyCode::Left | KeyCode::Up | KeyCode::Char('h' | 'k') => Some(IssueAction::Move(-1)),
        KeyCode::Right | KeyCode::Down | KeyCode::Char('j' | 'l') => Some(IssueAction::Move(1)),
        KeyCode::Enter => Some(IssueAction::Select),
        KeyCode::Esc => Some(IssueAction::Close),
        _ => None,
    }
}

impl Component for IssueDialog {
    /// How many issues are on offer.
    type Input<'a> = usize;
    type View<'a> = IssuesView<'a>;
    type Message = IssueAction;

    fn handle_key(&self, key: KeyEvent, _: &Self::Input<'_>) -> Option<Action> {
        key_to_action(key.code).map(Action::from)
    }

    fn update(&mut self, action: IssueAction, count: &Self::Input<'_>) -> Option<Effect> {
        match action {
            IssueAction::Move(delta) => self.cursor = step_index(self.cursor, delta, *count),
            // Closing and opening are the screen's: it holds the dialog.
            IssueAction::Select | IssueAction::Close => {}
        }
        None
    }

    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, view: &IssuesView<'_>) {
        let theme = theme::current();
        let selected = Style::default()
            .bg(theme.highlight_bg)
            .add_modifier(Modifier::BOLD);
        let normal = Style::default().fg(theme.muted);
        let here = view.pr_url.and_then(repo_of);
        let mut lines = vec![Line::styled(view.pr_label.clone(), normal), Line::default()];
        for (i, (issue, url)) in view.issues.iter().enumerate() {
            let marker = if i == self.cursor { "▶ " } else { "  " };
            let style = if i == self.cursor { selected } else { normal };
            let elsewhere = repo_of(url).filter(|repo| Some(*repo) != here);
            let place = elsewhere.map_or_else(String::new, |repo| format!("({repo}) "));
            let text = format!(" #{}  {} {place}", issue.number, issue.title);
            lines.push(Line::from(vec![
                Span::styled(marker, Style::default().fg(theme.accent)),
                Span::styled(text, style),
            ]));
        }
        let selected_line = 2 + self.cursor;
        choices_with_room(
            frame,
            area,
            "Open issue",
            lines,
            selected_line,
            &[
                ("j/k", "move"),
                ("Enter", "open in browser"),
                ("Esc", "cancel"),
            ],
            Room::Roomy,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn issue(number: u64, title: &str, url: &str) -> LinkedIssue {
        LinkedIssue {
            number,
            title: title.into(),
            url: Some(url.into()),
        }
    }

    fn drawn(cursor: usize) -> String {
        let a = issue(
            12,
            "Crash on start",
            "https://example.com/team/project/issues/12",
        );
        let b = issue(
            7,
            "Wrong sort order",
            "https://example.com/other/repo/issues/7",
        );
        let issues = [
            (&a, a.url.as_deref().unwrap_or("")),
            (&b, b.url.as_deref().unwrap_or("")),
        ];
        let view = IssuesView {
            pr_label: "PR #42 closes:".into(),
            pr_url: Some("https://example.com/team/project/pull/42"),
            issues: &issues,
        };
        let mut terminal = Terminal::new(TestBackend::new(90, 20)).unwrap();
        terminal
            .draw(|frame| IssueDialog { cursor }.render(frame, frame.area(), &view))
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..20)
            .map(|y| (0..90).map(|x| buffer[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_repository_of_an_address_is_its_first_two_parts() {
        assert_eq!(
            repo_of("https://github.com/team/project/issues/12"),
            Some("team/project")
        );
        assert_eq!(
            repo_of("https://github.com/team/project"),
            Some("team/project")
        );
        assert_eq!(repo_of("https://github.com/team"), None);
        assert_eq!(repo_of("not an address"), None);
    }

    #[test]
    fn the_dialog_lists_the_issues_and_names_the_repository_of_one_elsewhere() {
        let text = drawn(0);
        assert!(text.contains("PR #42 closes:"), "{text}");
        assert!(text.contains("▶  #12  Crash on start"), "{text}");
        assert!(
            !text.contains("(team/project)"),
            "same repository: not named: {text}"
        );
        assert!(text.contains("#7  Wrong sort order (other/repo)"), "{text}");
        assert!(text.contains("Enter open in browser"), "{text}");
    }

    #[test]
    fn the_cursor_marks_the_issue_that_enter_opens() {
        let text = drawn(1);
        assert!(text.contains("▶  #7  Wrong sort order"), "{text}");
        let mut dialog = IssueDialog::default();
        dialog.update(IssueAction::Move(5), &2);
        assert_eq!(dialog.cursor, 1, "it stops at the last issue");
        dialog.update(IssueAction::Move(-5), &2);
        assert_eq!(dialog.cursor, 0);
    }
}

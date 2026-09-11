use crate::{
    app::action::{Action, SearchAction},
    domain::{commit::Commit, pr::PullRequest},
    tui::{component::Component, widgets},
};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::Rect,
    widgets::Paragraph,
};

#[derive(Debug, Default)]
pub struct SearchInput {
    pub open: bool,
    pub query: String,
}

impl SearchInput {
    pub fn matches(&self, haystack: &str) -> bool {
        self.query.is_empty() || haystack.to_lowercase().contains(&self.query.to_lowercase())
    }

    pub fn matches_pr(&self, pr: &PullRequest) -> bool {
        self.matches(&pr.title)
            || self.matches(&pr.author.username)
            || self.matches(&format!("#{}", pr.id))
    }

    fn matches_commit(&self, c: &Commit) -> bool {
        self.matches(&c.oid) || self.matches(&c.headline)
    }

    pub fn filter_commits<'a>(&self, commits: &'a [Commit]) -> Vec<&'a Commit> {
        commits.iter().filter(|c| self.matches_commit(c)).collect()
    }
}

pub struct SearchContext {
    pub highlight: bool,
    pub matches: usize,
}
impl Component for SearchInput {
    type Context<'a> = SearchContext;
    type Message = SearchAction;
    fn handle_key(&self, key: KeyEvent, ctx: &SearchContext) -> Option<Action> {
        if !self.open {
            return (key.code == KeyCode::Char('/')).then_some(Action::Search(SearchAction::Open));
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return None;
        }
        match key.code {
            KeyCode::Char(c) => Some(Action::Search(SearchAction::Type(c))),
            KeyCode::Backspace => Some(Action::Search(SearchAction::Backspace)),
            KeyCode::Esc => Some(Action::Search(SearchAction::Cancel)),
            KeyCode::Enter if ctx.highlight => Some(Action::Search(SearchAction::Confirm)),
            _ => None,
        }
    }
    fn update(&mut self, action: SearchAction, _: &SearchContext) -> Option<Action> {
        match action {
            SearchAction::Open => self.open = true,
            SearchAction::Confirm => self.open = false,
            SearchAction::Type(c) => self.query.push(c),
            SearchAction::Backspace => {
                self.query.pop();
            }
            SearchAction::Cancel => {
                self.open = false;
                self.query.clear();
            }
        }
        None
    }
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &SearchContext) {
        frame.render_widget(
            Paragraph::new(widgets::search_prompt(&self.query, ctx.matches, area.width)),
            area,
        );
    }
}

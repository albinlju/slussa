use crate::{
    app::action::{Action, DetailAction},
    domain::pr::PullRequest,
    tui::{
        component::{Component, scroll},
        widgets::{self, markdown},
    },
};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::Rect,
};

pub fn render(frame: &mut Frame, pr: &PullRequest, ui: &mut Description, area: Rect) {
    let lines = markdown::render(description_body(pr), area.width.saturating_sub(1));
    widgets::scrolled_paragraph(frame, lines, &mut ui.scroll, &mut ui.viewport, area);
}

fn description_body(pr: &PullRequest) -> &str {
    pr.description
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("(no description)")
}

#[derive(Debug, Default)]
pub struct Description {
    pub scroll: u16,
    pub viewport: u16,
}

impl Component for Description {
    type Context<'a> = &'a PullRequest;
    type Message = DetailAction;
    fn handle_key(&self, key: KeyEvent, _: &Self::Context<'_>) -> Option<Action> {
        let delta = match key.code {
            KeyCode::Char('j') | KeyCode::Down => 1,
            KeyCode::Char('k') | KeyCode::Up => -1,
            KeyCode::PageDown => crate::tui::screens::half_page(self.viewport),
            KeyCode::PageUp => -crate::tui::screens::half_page(self.viewport),
            _ => return None,
        };
        Some(Action::Detail(DetailAction::DescriptionScroll(delta)))
    }
    fn update(&mut self, action: DetailAction, _: &Self::Context<'_>) -> Option<Action> {
        if let DetailAction::DescriptionScroll(delta) = action {
            self.scroll = scroll(self.scroll, delta);
        }
        None
    }
    fn render(&mut self, frame: &mut Frame, area: Rect, pr: &&PullRequest) {
        render(frame, pr, self, area);
    }
}

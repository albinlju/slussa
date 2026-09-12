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
    let content = Rect {
        width: area.width.saturating_sub(1),
        ..area
    };
    ui.max_horizontal = lines
        .iter()
        .map(ratatui::text::Line::width)
        .max()
        .unwrap_or(0)
        .saturating_sub(content.width as usize)
        .min(u16::MAX as usize) as u16;
    ui.horizontal = ui.horizontal.min(ui.max_horizontal);
    let max_scroll = lines
        .len()
        .saturating_sub(area.height as usize)
        .min(u16::MAX as usize) as u16;
    ui.scroll = ui.scroll.min(max_scroll);
    ui.viewport = area.height;
    frame.render_widget(
        ratatui::widgets::Paragraph::new(lines).scroll((ui.scroll, ui.horizontal)),
        content,
    );
    if max_scroll > 0 {
        frame.render_widget(
            ratatui::widgets::Paragraph::new(widgets::scrollbar(
                ui.scroll,
                max_scroll,
                area.height,
            )),
            crate::tui::layout::scrollbar_area(area),
        );
    }
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
    pub horizontal: u16,
    pub max_horizontal: u16,
}

impl Component for Description {
    type Context<'a> = &'a PullRequest;
    type Message = DetailAction;
    fn handle_key(&self, key: KeyEvent, _: &Self::Context<'_>) -> Option<Action> {
        if matches!(key.code, KeyCode::Char('H' | 'L')) && self.max_horizontal > 0 {
            return Some(Action::Detail(DetailAction::DescriptionHorizontal(
                if key.code == KeyCode::Char('H') {
                    -8
                } else {
                    8
                },
            )));
        }
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
        match action {
            DetailAction::DescriptionScroll(delta) => self.scroll = scroll(self.scroll, delta),
            DetailAction::DescriptionHorizontal(delta) => {
                self.horizontal = scroll(self.horizontal, delta).min(self.max_horizontal);
            }
            _ => {}
        }
        None
    }
    fn render(&mut self, frame: &mut Frame, area: Rect, pr: &&PullRequest) {
        render(frame, pr, self, area);
    }
}

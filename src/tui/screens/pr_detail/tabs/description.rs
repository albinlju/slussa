use crate::{
    app::{
        action::{Action, DescriptionAction, Effect},
        store::LoadState,
    },
    domain::pr::{PrInfo, PullRequest},
    tui::{
        component::{Component, saturating_u16, scroll},
        widgets::{self, markdown},
    },
};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::Rect,
};

/// `info` holds the description for a provider whose list leaves it out. Until
/// it arrives there is nothing to show but that it is loading.
fn render(
    frame: &mut Frame<'_>,
    pr: &PullRequest,
    info: Option<&LoadState<PrInfo>>,
    ui: &mut Description,
    area: Rect,
) {
    let body = match info {
        Some(LoadState::Loaded(info)) => description_body(info.description.as_deref()),
        Some(LoadState::Loading) if pr.description.is_none() => {
            frame.render_widget(
                ratatui::widgets::Paragraph::new(widgets::loading("Loading description...")),
                area,
            );
            return;
        }
        Some(LoadState::Failed(error)) if pr.description.is_none() => {
            frame.render_widget(
                ratatui::widgets::Paragraph::new(format!(
                    "Couldn't load the description. F: retry\n{}",
                    error.user_message()
                ))
                .style(ratatui::style::Style::default().fg(crate::tui::theme::current().error)),
                area,
            );
            return;
        }
        _ => description_body(pr.description.as_deref()),
    };
    let lines = markdown::render(body, area.width.saturating_sub(1));
    let content = Rect {
        width: area.width.saturating_sub(1),
        ..area
    };
    ui.max_horizontal = saturating_u16(
        lines
            .iter()
            .map(ratatui::text::Line::width)
            .max()
            .unwrap_or(0)
            .saturating_sub(content.width as usize),
    );
    ui.horizontal = ui.horizontal.min(ui.max_horizontal);
    let max_scroll = saturating_u16(lines.len().saturating_sub(area.height as usize));
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

fn description_body(description: Option<&str>) -> &str {
    description
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

/// The PR whose description to show, and what was read for it when the list
/// left the description out.
pub struct DescriptionView<'a> {
    pub pr: &'a PullRequest,
    pub info: Option<&'a LoadState<PrInfo>>,
}

impl Component for Description {
    type Input<'a> = ();
    type View<'a> = DescriptionView<'a>;
    type Message = DescriptionAction;
    fn handle_key(&self, key: KeyEvent, (): &()) -> Option<Action> {
        if matches!(key.code, KeyCode::Char('H' | 'L')) && self.max_horizontal > 0 {
            let delta = if key.code == KeyCode::Char('H') {
                -8
            } else {
                8
            };
            return Some(DescriptionAction::Horizontal(delta).into());
        }
        let delta = match key.code {
            KeyCode::Char('j') | KeyCode::Down => 1,
            KeyCode::Char('k') | KeyCode::Up => -1,
            KeyCode::PageDown => crate::tui::screens::half_page(self.viewport),
            KeyCode::PageUp => -crate::tui::screens::half_page(self.viewport),
            _ => return None,
        };
        Some(DescriptionAction::Scroll(delta).into())
    }
    fn update(&mut self, action: DescriptionAction, (): &()) -> Option<Effect> {
        match action {
            DescriptionAction::Scroll(delta) => self.scroll = scroll(self.scroll, delta),
            DescriptionAction::Horizontal(delta) => {
                self.horizontal = scroll(self.horizontal, delta).min(self.max_horizontal);
            }
        }
        None
    }
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, view: &DescriptionView<'_>) {
        render(frame, view.pr, view.info, self, area);
    }
}

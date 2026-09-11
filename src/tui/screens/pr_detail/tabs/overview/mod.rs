mod sidebar;
pub mod timeline;
use crate::{
    app::{
        action::{Action, DetailAction},
        store::PrData,
    },
    domain::pr::PullRequest,
    tui::{component::Component, layout},
};
use ratatui::{
    Frame,
    crossterm::event::KeyEvent,
    layout::{Constraint, Direction, Rect},
};
use timeline::{Timeline, TimelineContext};

const SIDEBAR_WIDTH: u16 = 30;
const SIDEBAR_BREAKPOINT: u16 = 64;

#[derive(Debug, Default)]
pub struct Overview {
    pub timeline: Timeline,
}
impl Component for Overview {
    type Context<'a> = (&'a PullRequest, Option<&'a PrData>);
    type Message = DetailAction;
    fn handle_key(&self, key: KeyEvent, ctx: &Self::Context<'_>) -> Option<Action> {
        self.timeline.handle_key(
            key,
            &TimelineContext {
                data: ctx.1,
                author: &ctx.0.author.username,
                scrollbar: Rect::default(),
            },
        )
    }
    fn update(&mut self, action: DetailAction, ctx: &Self::Context<'_>) -> Option<Action> {
        self.timeline.update(
            action,
            &TimelineContext {
                data: ctx.1,
                author: &ctx.0.author.username,
                scrollbar: Rect::default(),
            },
        )
    }
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Self::Context<'_>) {
        render(frame, ctx.0, ctx.1, self, area);
    }
}

pub fn render(
    frame: &mut Frame,
    pr: &PullRequest,
    pr_data: Option<&PrData>,
    ui: &mut Overview,
    area: Rect,
) {
    let (body_area, sidebar_area, scrollbar_area) = if area.width >= SIDEBAR_BREAKPOINT {
        let [body, sidebar, scrollbar] = layout::split(
            area,
            Direction::Horizontal,
            [
                Constraint::Min(0),
                Constraint::Length(SIDEBAR_WIDTH),
                Constraint::Length(1),
            ],
        );
        (body, Some(sidebar), scrollbar)
    } else {
        let [body, scrollbar] = layout::split(
            area,
            Direction::Horizontal,
            [Constraint::Min(0), Constraint::Length(1)],
        );
        (body, None, scrollbar)
    };

    if let Some(sidebar) = sidebar_area {
        frame.render_widget(sidebar::Sidebar { pr, data: pr_data }, sidebar);
    }
    ui.timeline.render(
        frame,
        body_area,
        &TimelineContext {
            data: pr_data,
            author: &pr.author.username,
            scrollbar: scrollbar_area,
        },
    );
}

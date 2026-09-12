mod sidebar;
pub mod timeline;
use crate::{
    app::{
        action::{Action, DetailAction},
        store::PrData,
    },
    domain::{
        capabilities::{Capabilities, Feature},
        pr::PullRequest,
    },
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

pub struct OverviewContext<'a> {
    pub pr: &'a PullRequest,
    pub data: Option<&'a PrData>,
    pub capabilities: &'a Capabilities,
}

#[derive(Debug, Default)]
pub struct Overview {
    pub timeline: Timeline,
}
impl Overview {
    pub fn render_with_scrollbar(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        ctx: &OverviewContext<'_>,
        scrollbar: Rect,
    ) {
        render(frame, ctx, self, area, scrollbar);
    }
}
impl Component for Overview {
    type Context<'a> = OverviewContext<'a>;
    type Message = DetailAction;
    fn handle_key(&self, key: KeyEvent, ctx: &Self::Context<'_>) -> Option<Action> {
        self.timeline.handle_key(
            key,
            &TimelineContext {
                data: ctx.data,
                author: &ctx.pr.author.username,
                scrollbar: Rect::default(),
            },
        )
    }
    fn update(&mut self, action: DetailAction, ctx: &Self::Context<'_>) -> Option<Action> {
        self.timeline.update(
            action,
            &TimelineContext {
                data: ctx.data,
                author: &ctx.pr.author.username,
                scrollbar: Rect::default(),
            },
        )
    }
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Self::Context<'_>) {
        self.render_with_scrollbar(frame, area, ctx, layout::scrollbar_area(area));
    }
}

fn render(
    frame: &mut Frame,
    ctx: &OverviewContext<'_>,
    ui: &mut Overview,
    area: Rect,
    scrollbar_area: Rect,
) {
    let pr = ctx.pr;
    let pr_data = ctx.data;
    let (body_area, sidebar_area) = if area.width >= SIDEBAR_BREAKPOINT {
        let [body, sidebar, _gutter] = layout::split(
            area,
            Direction::Horizontal,
            [
                Constraint::Min(0),
                Constraint::Length(SIDEBAR_WIDTH),
                Constraint::Length(1),
            ],
        );
        (body, Some(sidebar))
    } else {
        let [body, _gutter] = layout::split(
            area,
            Direction::Horizontal,
            [Constraint::Min(0), Constraint::Length(1)],
        );
        (body, None)
    };

    if let Some(sidebar) = sidebar_area {
        frame.render_widget(
            sidebar::Sidebar {
                pr,
                data: pr_data,
                show_builds: ctx.capabilities.supports(Feature::Builds),
            },
            sidebar,
        );
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

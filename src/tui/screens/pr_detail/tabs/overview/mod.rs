mod blocks;
mod hidden;
mod sidebar;
pub mod timeline;
use crate::tui::widgets::comment_meta::Roles;
use crate::{
    app::{
        action::{Action, Effect, TimelineAction},
        store::PrData,
    },
    domain::{
        authorship::{AiMarkers, AuthorFilter},
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

/// Whether `f` is offered: when the PR has an agent's comment to filter on, or
/// a filter is on already, which must not become impossible to turn off.
pub fn offers_filter(markers: &AiMarkers, data: Option<&PrData>, current: AuthorFilter) -> bool {
    current != AuthorFilter::All
        || data
            .and_then(|d| d.activity.loaded())
            .is_some_and(|activity| markers.any_in(activity))
}

pub struct OverviewContext<'a> {
    pub pr: &'a PullRequest,
    pub data: Option<&'a PrData>,
    pub capabilities: &'a Capabilities,
    pub markers: &'a AiMarkers,
    /// Where the timeline's scrollbar goes: the screen's edge, not the tab's.
    pub scrollbar: Rect,
}

#[derive(Debug, Default)]
pub struct Overview {
    pub timeline: Timeline,
}

impl Component for Overview {
    type Input<'a> = ();
    type View<'a> = OverviewContext<'a>;
    type Message = TimelineAction;
    fn handle_key(&self, key: KeyEvent, (): &()) -> Option<Action> {
        self.timeline.handle_key(key, &())
    }
    fn update(&mut self, action: TimelineAction, (): &()) -> Option<Effect> {
        self.timeline.update(action, &())
    }
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, ctx: &OverviewContext<'_>) {
        render(frame, ctx, self, area, ctx.scrollbar);
    }
}

fn render(
    frame: &mut Frame<'_>,
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
            roles: Roles {
                pr_author: &pr.author.username,
                markers: ctx.markers,
            },
            scrollbar: scrollbar_area,
        },
    );
}

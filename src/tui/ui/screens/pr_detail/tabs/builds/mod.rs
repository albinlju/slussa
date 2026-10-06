//! The Builds tab: the builds of the head, with a cursor on one of them, and
//! the log of the one the reader opened, in the same place.

mod list;
mod log;

use crate::{
    domain::{
        build_log::BuildLog,
        ci::{Build, JobId},
        pr::PrId,
    },
    tui::{
        app::{effect::Effect, store::PrData},
        ui::{
            action::{Action, BuildsAction},
            component::{Component, saturating_u16, scroll, step_index},
            screens::half_page,
            widgets,
        },
    },
};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::Rect,
};

/// What the tab shows: the builds, or the log of one of them.
#[derive(Debug, Default)]
enum BuildsView {
    #[default]
    List,
    Log(OpenLog),
}

/// The log of one build that is open. Its place is known once it has been read:
/// until then there is nothing to place it in.
#[derive(Debug)]
pub(super) struct OpenLog {
    job: JobId,
    /// The first line on screen; `None` until the log is read and placed.
    scroll: Option<u16>,
    /// The error the reader is at, for `n` and `N`.
    error: Option<usize>,
    viewport: u16,
}

#[derive(Debug, Default)]
pub struct Builds {
    cursor: usize,
    scroll: u16,
    viewport: u16,
    view: BuildsView,
}

impl Builds {
    /// Whether a log is open rather than the list.
    pub const fn log_open(&self) -> bool {
        matches!(self.view, BuildsView::Log(_))
    }

    /// Whether the build the cursor is on has a log `enter` opens.
    pub fn log_under_cursor(&self, input: &BuildsInput<'_>) -> bool {
        input.job_at(self.cursor).is_some()
    }

    /// Back to the list, as when the tab is left.
    pub const fn close_log(&mut self) {
        self.view = BuildsView::List;
    }
}

/// The builds to move between, and the log that is open.
pub struct BuildsInput<'a> {
    pub pr_id: PrId,
    data: Option<&'a PrData>,
}

impl<'a> BuildsInput<'a> {
    pub const fn new(pr_id: PrId, data: Option<&'a PrData>) -> Self {
        Self { pr_id, data }
    }

    pub fn builds(&self) -> &'a [Build] {
        self.data
            .and_then(|data| data.builds.loaded())
            .map_or(&[], Vec::as_slice)
    }

    fn log(&self, job: JobId) -> Option<&'a BuildLog> {
        self.data?.build_logs.get(&job)?.loaded()
    }

    /// The job of the build the cursor is on, when it has a log to read.
    fn job_at(&self, cursor: usize) -> Option<JobId> {
        self.builds().get(cursor)?.log
    }
}

impl Component for Builds {
    type Input<'a> = BuildsInput<'a>;
    type View<'a> = BuildsInput<'a>;
    type Message = BuildsAction;

    fn handle_key(&self, key: KeyEvent, input: &BuildsInput<'_>) -> Option<Action> {
        let action = match &self.view {
            BuildsView::List => match key.code {
                KeyCode::Char('j') | KeyCode::Down => BuildsAction::Move(1),
                KeyCode::Char('k') | KeyCode::Up => BuildsAction::Move(-1),
                KeyCode::PageDown => BuildsAction::Move(half_page(self.viewport)),
                KeyCode::PageUp => BuildsAction::Move(-half_page(self.viewport)),
                KeyCode::Enter if input.job_at(self.cursor).is_some() => BuildsAction::Open,
                _ => return None,
            },
            BuildsView::Log(open) => match key.code {
                KeyCode::Char('j') | KeyCode::Down => BuildsAction::Scroll(1),
                KeyCode::Char('k') | KeyCode::Up => BuildsAction::Scroll(-1),
                KeyCode::PageDown => BuildsAction::Scroll(half_page(open.viewport)),
                KeyCode::PageUp => BuildsAction::Scroll(-half_page(open.viewport)),
                KeyCode::Char('n') => BuildsAction::NextError(1),
                KeyCode::Char('N') => BuildsAction::NextError(-1),
                _ => return None,
            },
        };
        Some(Action::from(action))
    }

    fn update(&mut self, action: BuildsAction, input: &BuildsInput<'_>) -> Option<Effect> {
        match (&mut self.view, action) {
            (BuildsView::List, BuildsAction::Move(delta)) => {
                self.cursor = step_index(self.cursor, delta, input.builds().len());
            }
            (BuildsView::List, BuildsAction::Open) => {
                let job = input.job_at(self.cursor)?;
                self.view = BuildsView::Log(OpenLog {
                    job,
                    scroll: None,
                    error: None,
                    viewport: 0,
                });
                return Some(Effect::LoadBuildLog {
                    pr_id: input.pr_id,
                    job,
                });
            }
            (BuildsView::Log(open), BuildsAction::Scroll(delta)) => {
                open.scroll = open.scroll.map(|at| scroll(at, delta));
            }
            (BuildsView::Log(open), BuildsAction::NextError(step)) => {
                let log = input.log(open.job)?;
                let at = if step >= 0 {
                    log.next_error(open.error.map_or(0, |at| at.saturating_add(1)))
                } else {
                    log.previous_error(open.error.unwrap_or(usize::MAX))
                }?;
                open.error = Some(at);
                open.scroll = Some(saturating_u16(at.saturating_sub(log::CONTEXT)));
            }
            (BuildsView::Log(_), BuildsAction::Close) => self.view = BuildsView::List,
            // A key for the other view: the list has no log to scroll.
            (
                BuildsView::List,
                BuildsAction::Scroll(_) | BuildsAction::NextError(_) | BuildsAction::Close,
            )
            | (BuildsView::Log(_), BuildsAction::Move(_) | BuildsAction::Open) => {}
        }
        None
    }

    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, input: &BuildsInput<'_>) {
        match &mut self.view {
            BuildsView::List => {
                let state = input.data.map(|data| &data.builds);
                let Some(builds) = widgets::loaded_or_placeholder(frame, state, "builds", area)
                else {
                    return;
                };
                if builds.is_empty() {
                    frame.render_widget(
                        widgets::empty_state("(no builds reported for this commit)"),
                        area,
                    );
                    return;
                }
                self.cursor = self.cursor.min(builds.len().saturating_sub(1));
                list::render(
                    frame,
                    builds,
                    self.cursor,
                    &mut self.scroll,
                    &mut self.viewport,
                    area,
                );
            }
            BuildsView::Log(open) => {
                let build = input.builds().iter().find(|b| b.log == Some(open.job));
                let state = input.data.and_then(|data| data.build_logs.get(&open.job));
                log::render(frame, open, build, state, area);
            }
        }
    }
}

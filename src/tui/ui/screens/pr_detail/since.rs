//! What is new since the reader looked: the diff from the head they had open to
//! the head now, in the Diff tab in place of the whole diff. It has a viewer of
//! its own, so it never moves the PR diff's cursor or search, and no comment is
//! made on it: its lines are not the PR's.

use crate::{
    domain::{commit::CommitOid, diff::DiffRange},
    tui::{
        app::store::{LoadState, PrData},
        ui::{
            component::Component,
            components::diff_viewer::{DiffContext, DiffViewer, PaneNav},
            layout, theme,
            widgets::{self, comment::meta::Reading},
        },
    },
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Rect},
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph},
};

#[derive(Debug)]
pub struct SinceView {
    range: DiffRange,
    viewer: DiffViewer,
}

impl SinceView {
    pub fn new(range: DiffRange) -> Self {
        Self {
            range,
            viewer: DiffViewer::default(),
        }
    }

    pub const fn range(&self) -> &DiffRange {
        &self.range
    }

    pub const fn viewer(&self) -> &DiffViewer {
        &self.viewer
    }

    pub const fn viewer_mut(&mut self) -> &mut DiffViewer {
        &mut self.viewer
    }

    /// The head the reader has on screen once this is open and read.
    pub fn head_read(&self, data: Option<&PrData>) -> Option<&CommitOid> {
        data?.range_diffs.get(&self.range)?.loaded()?;
        Some(&self.range.head)
    }
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    since: &mut SinceView,
    data: Option<&PrData>,
    reading: Reading<'_>,
    area: Rect,
) {
    let [banner, body] = layout::split(
        area,
        Direction::Vertical,
        [Constraint::Length(2), Constraint::Min(0)],
    );
    // The banner's text stands where the panels' titles do, one column in.
    let banner = Rect {
        x: banner.x.saturating_add(1),
        width: banner.width.saturating_sub(1),
        ..banner
    };
    render_banner(frame, &since.range, banner);
    let state = data.and_then(|data| data.range_diffs.get(&since.range));
    if let Some(LoadState::Loaded(diff)) = state
        && diff.files.is_empty()
    {
        // What the pane drew last is not there to act on any more.
        since.viewer.pane = PaneNav::default();
        frame.render_widget(
            widgets::empty_state(
                "Nothing new in the files of this PR. If the branch was reset to an \
                 older commit, that is what it looks like too. w: the whole diff.",
            ),
            body,
        );
        return;
    }
    since.viewer.render(
        frame,
        body,
        &DiffContext {
            diff: state,
            threads: &[],
            pending: &[],
            reading,
        },
    );
}

fn render_banner(frame: &mut Frame<'_>, range: &DiffRange, area: Rect) {
    let theme = theme::current();
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(theme.divider));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let left = vec![
        Span::styled(
            "↻ new since you read it  ",
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{} → {}", range.base.short(), range.head.short()),
            Style::default().fg(theme.muted),
        ),
    ];
    let hint = if inner.width < 60 {
        "esc: whole"
    } else {
        "w / esc: the whole diff"
    };
    let right = vec![Span::styled(hint, Style::default().fg(theme.muted))];
    let line = widgets::fitted_row(left, right, usize::from(inner.width));
    frame.render_widget(Paragraph::new(line), inner);
}

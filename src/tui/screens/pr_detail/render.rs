use super::{dialogs, footer, header};
use crate::{
    app::{
        navigation::Screen,
        reviews::{PendingComment, PendingReview},
        store::{LoadState, PrData},
    },
    domain::{comment::CommentThread, pr::PullRequest},
    tui::{
        component::Component,
        components::diff_viewer::DiffContext,
        layout,
        screens::pr_detail::{
            DetailContext, DetailView, PrDetailScreen,
            tabs::{DetailTab, builds, commits},
        },
        theme,
    },
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

fn render_tabs_and_content(
    frame: &mut Frame,
    pr: &PullRequest,
    pr_data: Option<&PrData>,
    ui: &mut PrDetailScreen,
    pending: &[PendingComment],
    tab: DetailTab,
    area: Rect,
) {
    let theme = theme::current();
    let [tabs_area, content_area] = layout::split(
        area,
        Direction::Vertical,
        [Constraint::Length(3), Constraint::Min(0)],
    );

    let tabs_block = Block::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(Style::default().fg(theme.divider));
    let tabs_inner = tabs_block.inner(tabs_area);
    frame.render_widget(tabs_block, tabs_area);
    frame.render_widget(Paragraph::new(tab_bar(tab)), tabs_inner);

    render_content(frame, pr, pr_data, ui, pending, tab, content_area);
}

fn tab_bar(tab: DetailTab) -> Line<'static> {
    let theme = theme::current();
    let active = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    let inactive = Style::default().fg(theme.muted);
    let sep = Style::default().fg(theme.muted);

    let mut spans = vec![Span::raw("  ")];
    for (i, t) in DetailTab::ALL.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", sep));
        }
        let style = if i == tab.index() { active } else { inactive };
        spans.push(Span::styled(t.label(), style));
    }
    Line::from(spans)
}

fn render_content(
    frame: &mut Frame,
    pr: &PullRequest,
    pr_data: Option<&PrData>,
    ui: &mut PrDetailScreen,
    pending: &[PendingComment],
    tab: DetailTab,
    area: Rect,
) {
    let inset = match tab {
        DetailTab::Description => area,
        _ => Rect {
            x: area.x + 2,
            y: area.y,
            width: area.width.saturating_sub(4),
            height: area.height,
        },
    };
    match tab {
        DetailTab::Description => ui.description.render(frame, inset, &pr),
        DetailTab::Overview => ui.overview.render(frame, inset, &(pr, pr_data)),
        DetailTab::Diff => {
            let threads = activity_threads(pr_data);
            let diff = pr_data.and_then(|d| d.diff_for(ui.commits.open_commit.as_deref()));
            ui.diff.render(
                frame,
                inset,
                &DiffContext {
                    diff,
                    threads,
                    pending,
                    author: &pr.author.username,
                },
            );
        }
        DetailTab::Commits => {
            ui.commits.render(
                frame,
                inset,
                &commits::CommitContext {
                    pr_id: pr.id,
                    data: pr_data,
                    pending,
                    author: &pr.author.username,
                },
            );
        }
        DetailTab::Builds => builds::render(frame, pr_data, inset),
    }
}

fn pending_comments(pending: Option<&PendingReview>) -> &[PendingComment] {
    pending.map_or(&[], |r| r.comments.as_slice())
}

fn activity_threads(pr_data: Option<&PrData>) -> &[CommentThread] {
    pr_data
        .and_then(|d| match &d.activity {
            LoadState::Loaded(b) => Some(b.threads.as_slice()),
            _ => None,
        })
        .unwrap_or(&[])
}

pub(super) fn render(
    ui: &mut PrDetailScreen,
    frame: &mut Frame,
    area: Rect,
    ctx: &DetailContext<'_>,
) {
    let Screen::Detail { pr_id, tab } = ctx.screen else {
        return;
    };

    let LoadState::Loaded(prs) = &ctx.store.cache.prs else {
        return;
    };
    let Some(pr) = prs.iter().find(|p| p.id == pr_id) else {
        return;
    };

    let theme = theme::current();
    let (main_area, footer_area, help_area) = if ui.help_open {
        let [main, footer, help] = layout::split(
            area,
            Direction::Vertical,
            [
                Constraint::Min(0),
                Constraint::Length(1),
                Constraint::Percentage(30),
            ],
        );
        (main, footer, Some(help))
    } else {
        let [main, footer] = layout::split(
            area,
            Direction::Vertical,
            [Constraint::Min(0), Constraint::Length(1)],
        );
        (main, footer, None)
    };

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border));
    let inner = outer.inner(main_area);
    frame.render_widget(outer, main_area);

    let [header_area, _gap, content_area] = layout::split(
        inner,
        Direction::Vertical,
        [
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(0),
        ],
    );

    let pr_data = ctx.store.cache.details.get(&pr.id);
    header::render(frame, pr, pr_data.map(|d| &d.mergeability), header_area);
    render_tabs_and_content(
        frame,
        pr,
        pr_data,
        ui,
        pending_comments(ctx.store.reviews.get(&pr_id)),
        tab,
        content_area,
    );
    let state = DetailView {
        detail: ui,
        store: ctx.store,
        screen: ctx.screen,
        refreshing: ctx.refreshing,
    };
    footer::render(frame, &state, pr_data, tab, footer_area);

    if let Some(help_area) = help_area {
        dialogs::help::render(frame, help_area);
    }
    let review_ctx = dialogs::review::ReviewContext {
        options: state.review_context().options,
        pending: ctx.store.reviews.get(&pr_id),
    };
    if let Some(dialog) = &mut ui.confirm {
        dialog.render(frame, area, &());
    }
    if let Some(dialog) = &mut ui.review_picker {
        dialog.render(frame, area, &review_ctx);
    }
    if let Some(dialog) = &mut ui.merge_picker {
        dialog.render(frame, area, &ctx.store.merge_strategies.as_slice());
    }
    if let Some(msg) = ctx.store.errors.get(&pr_id) {
        dialogs::error::render(frame, msg, area);
    }
}

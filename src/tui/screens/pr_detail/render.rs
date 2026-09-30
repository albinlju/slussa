use super::{dialogs, footer, header};
use crate::{
    app::{
        navigation::Screen,
        reviews::{PendingComment, PendingReview},
        store::{LoadState, PrData},
    },
    domain::{
        capabilities::{Capabilities, Feature},
        comment::CommentThread,
    },
    tui::{
        component::Component,
        components::diff_viewer::DiffContext,
        layout,
        screens::pr_detail::{
            DetailContext, DetailView, PrDetailScreen,
            tabs::{DetailTab, commits},
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
    frame: &mut Frame<'_>,
    overview: &super::tabs::overview::OverviewContext<'_>,
    ui: &mut PrDetailScreen,
    pending: &[PendingComment],
    tab: DetailTab,
    area: Rect,
) {
    let theme = theme::current();
    let compact = area.height < 16;
    let [tabs_area, content_area] = layout::split(
        area,
        Direction::Vertical,
        [
            Constraint::Length(if compact { 2 } else { 3 }),
            Constraint::Min(0),
        ],
    );

    let tabs_block = Block::default()
        .borders(if compact {
            Borders::BOTTOM
        } else {
            Borders::TOP | Borders::BOTTOM
        })
        .border_style(Style::default().fg(theme.divider));
    let tabs_inner = tabs_block.inner(tabs_area);
    frame.render_widget(tabs_block, tabs_area);
    frame.render_widget(
        Paragraph::new(tab_bar(tab, overview.capabilities, tabs_inner.width)),
        tabs_inner,
    );

    render_content(frame, overview, ui, pending, tab, content_area);
}

fn tab_bar(tab: DetailTab, caps: &Capabilities, width: u16) -> Line<'static> {
    let theme = theme::current();
    let active = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    let inactive = Style::default().fg(theme.muted);
    let sep = Style::default().fg(theme.muted);

    let tabs = DetailTab::available(caps);
    let labels_width =
        2 + tabs.iter().map(|t| t.label().len()).sum::<usize>() + tabs.len().saturating_sub(1) * 3;
    if labels_width > width as usize {
        let number = tabs.iter().position(|t| *t == tab).unwrap_or(0) + 1;
        return Line::from(vec![
            Span::styled(format!("  {number} {}", tab.label()), active),
            Span::styled(format!("   1-{}: tabs", tabs.len()), inactive),
        ]);
    }
    let mut spans = vec![Span::raw("  ")];
    for (i, t) in DetailTab::available(caps).iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", sep));
        }
        let style = if *t == tab { active } else { inactive };
        spans.push(Span::styled(t.label(), style));
    }
    Line::from(spans)
}

fn render_content(
    frame: &mut Frame<'_>,
    overview: &super::tabs::overview::OverviewContext<'_>,
    ui: &mut PrDetailScreen,
    pending: &[PendingComment],
    tab: DetailTab,
    area: Rect,
) {
    let pr = overview.pr;
    let pr_data = overview.data;
    let inset = match tab {
        DetailTab::Description => area,
        _ => Rect {
            x: area.x + if area.width < 70 { 0 } else { 2 },
            y: area.y,
            width: area
                .width
                .saturating_sub(if area.width < 70 { 0 } else { 4 }),
            height: area.height,
        },
    };
    match tab {
        DetailTab::Description => ui.description.render(frame, inset, &pr),
        DetailTab::Overview => ui.overview.render_with_scrollbar(
            frame,
            inset,
            overview,
            Rect::new(area.right(), area.y, 1, area.height),
        ),
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
        DetailTab::Builds => ui.builds.render(frame, inset, &pr_data),
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
    frame: &mut Frame<'_>,
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
    let [main_area, footer_area] = layout::split(
        area,
        Direction::Vertical,
        [Constraint::Min(0), Constraint::Length(1)],
    );

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border));
    let inner = outer.inner(main_area);
    frame.render_widget(outer, main_area);

    let compact = area.width < 70 || area.height < 22;
    let [header_area, _gap, content_area] = layout::split(
        inner,
        Direction::Vertical,
        [
            Constraint::Length(if compact { 2 } else { 3 }),
            Constraint::Length(u16::from(!compact)),
            Constraint::Min(0),
        ],
    );

    let pr_data = ctx.store.cache.details.get(&pr.id);
    header::render(
        frame,
        pr,
        pr_data
            .filter(|_| ctx.store.capabilities.supports(Feature::Mergeability))
            .map(|d| &d.mergeability),
        header_area,
    );
    render_tabs_and_content(
        frame,
        &super::tabs::overview::OverviewContext {
            pr,
            data: pr_data,
            capabilities: &ctx.store.capabilities,
        },
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

    let review_ctx = dialogs::review::ReviewContext {
        options: state.review_context().options,
        pending: ctx.store.reviews.get(&pr_id),
    };
    ui.editor.target_context = ui.editor.draft.as_ref().and_then(|draft| {
        let comment = match &draft.target {
            crate::app::reviews::CommentTarget::Reply(id) => {
                pr_data.and_then(|data| match &data.activity {
                    LoadState::Loaded(activity) => activity
                        .threads
                        .iter()
                        .find(|thread| thread.reply_to == Some(*id))
                        .and_then(|thread| thread.comments.first()),
                    _ => None,
                })
            }
            crate::app::reviews::CommentTarget::Edit { id, review } => {
                state.find_comment(*id, *review)
            }
            _ => None,
        }?;
        Some(format!(
            "@{}: {}",
            comment.author.username,
            comment.content.lines().next().unwrap_or("")
        ))
    });
    if ui.editor.is_open() {
        ui.editor.render_with_review(
            frame,
            area,
            ctx.store.operations.contains_key(&pr_id),
            ctx.store.reviews.contains_key(&pr_id),
        );
    }
    if ui.help_open {
        ui.help.render(
            frame,
            area,
            &dialogs::help::entries(&ctx.store.capabilities, pr.url.is_some()).as_slice(),
        );
    }
    if let Some(dialog) = &mut ui.confirm {
        dialog.set_pr_context(format!("PR #{} · {}", pr.id, pr.title), &pr.target_branch);
        dialog.render(frame, area, &());
    }
    if let Some(dialog) = &mut ui.review_picker {
        dialog.render(frame, area, &review_ctx);
    }
    if let Some(dialog) = &mut ui.merge_picker {
        dialog.pr_label = format!("PR #{} · {}", pr.id, pr.title);
        dialog.target_branch.clone_from(&pr.target_branch);
        dialog.source_branch.clone_from(&pr.source_branch);
        dialog.render(
            frame,
            area,
            &ctx.store.capabilities.merge_strategies.as_slice(),
        );
    }
    if let Some(msg) = ctx.store.errors.get(&pr_id) {
        ui.error.render(frame, msg, area);
    }
}

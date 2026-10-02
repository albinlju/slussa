use super::{dialogs, footer, header};
use crate::tui::ui::widgets::{comment_fold::Folds, comment_meta::Reading};
use crate::{
    domain::{
        capabilities::{Capabilities, Feature},
        comment::CommentThread,
        pr::Mergeability,
    },
    tui::app::{
        reviews::{PendingComment, PendingReview},
        store::PrData,
    },
    tui::ui::{
        component::Component,
        components::{comment_editor::EditorView, diff_viewer::DiffContext},
        layout,
        screens::pr_detail::{
            DetailContext, DetailView, Overlay, PrDetailScreen,
            dialogs::{PrSummary, merge::MergeView},
            tabs::{DetailTab, commits, description::DescriptionView, overview::OverviewContext},
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
    ctx: &DetailContext<'_>,
    ui: &mut PrDetailScreen,
    pending: &[PendingComment],
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
        Paragraph::new(tab_bar(ctx.tab, &ctx.store.capabilities, tabs_inner.width)),
        tabs_inner,
    );

    render_content(frame, ctx, ui, pending, content_area);
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
    ctx: &DetailContext<'_>,
    ui: &mut PrDetailScreen,
    pending: &[PendingComment],
    area: Rect,
) {
    let (pr, pr_data, tab) = (ctx.pr, ctx.data, ctx.tab);
    let inset = match tab {
        DetailTab::Description => area,
        DetailTab::Overview | DetailTab::Diff | DetailTab::Commits | DetailTab::Builds => Rect {
            x: area.x + if area.width < 70 { 0 } else { 2 },
            y: area.y,
            width: area
                .width
                .saturating_sub(if area.width < 70 { 0 } else { 4 }),
            height: area.height,
        },
    };
    match tab {
        DetailTab::Description => ui.description.render(
            frame,
            inset,
            &DescriptionView {
                pr,
                info: pr_data.map(|data| &data.info),
            },
        ),
        DetailTab::Overview => ui.overview.render(
            frame,
            inset,
            &OverviewContext {
                pr,
                data: pr_data,
                capabilities: &ctx.store.capabilities,
                scrollbar: Rect::new(area.right(), area.y, 1, area.height),
            },
        ),
        DetailTab::Diff => {
            let threads = activity_threads(pr_data);
            let diff = pr_data.map(|d| &d.diff);
            ui.diff.render(
                frame,
                inset,
                &DiffContext {
                    diff,
                    threads,
                    pending,
                    reading: Reading {
                        pr_author: &pr.author.username,
                        folds: Folds::Open,
                    },
                },
            );
        }
        DetailTab::Commits => {
            ui.commits.render(
                frame,
                inset,
                &commits::CommitContext {
                    data: pr_data,
                    pending,
                    reading: Reading {
                        pr_author: &pr.author.username,
                        folds: Folds::Open,
                    },
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
        .and_then(|data| data.activity.loaded())
        .map_or(&[], |activity| activity.threads.as_slice())
}

pub(super) fn render(
    ui: &mut PrDetailScreen,
    frame: &mut Frame<'_>,
    area: Rect,
    ctx: &DetailContext<'_>,
) {
    let (pr_id, pr, pr_data) = (ctx.pr_id, ctx.pr, ctx.data);

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
        ctx,
        ui,
        pending_comments(ctx.store.reviews.get(&pr_id)),
        content_area,
    );
    let state = DetailView::new(ui, ctx);
    footer::render(frame, &state, footer_area);

    let review_ctx = dialogs::review::ReviewContext {
        options: state.review_context().options,
        pending: ctx.store.reviews.get(&pr_id),
    };
    if ui.editor.is_open() {
        let view = EditorView {
            sending: ctx.store.operations.contains_key(&pr_id),
            review_active: ctx.store.reviews.contains_key(&pr_id),
            context: ui
                .editor
                .target()
                .and_then(|target| state.comment_under(target))
                .map(|comment| {
                    format!(
                        "@{}: {}",
                        comment.author.username,
                        comment.content.lines().next().unwrap_or("")
                    )
                }),
        };
        ui.editor.render(frame, area, &view);
    }
    match &mut ui.overlay {
        Some(Overlay::Help(help)) => help.render(
            frame,
            area,
            &dialogs::help::entries(
                &ctx.store.capabilities,
                pr.url.is_some(),
                super::tabs::overview::offers_filter(pr_data, ui.overview.timeline.filter),
            )
            .as_slice(),
        ),
        Some(Overlay::Confirm(dialog)) => dialog.render(frame, area, &PrSummary::of(pr)),
        Some(Overlay::Review(dialog)) => dialog.render(frame, area, &review_ctx),
        Some(Overlay::Merge(dialog)) => {
            // What stands in the way of a merge, as far as the provider said.
            let blockers = pr_data
                .and_then(|data| data.mergeability.loaded())
                .map_or(&[][..], Mergeability::blockers);
            dialog.render(
                frame,
                area,
                &MergeView {
                    strategies: &ctx.store.capabilities.merge_strategies,
                    pr: PrSummary::of(pr),
                    blockers,
                },
            );
        }
        None => {}
    }
    if let Some(msg) = ctx.store.errors.get(&pr_id) {
        ui.error.render(frame, area, &msg.as_str());
    }
}

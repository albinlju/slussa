use super::{
    nav::{NavItem, NavKind},
    threads::{ThreadDraw, push_thread},
};
use crate::{
    domain::{
        comment::{CommentId, CommentThread},
        diff::{Diff, DiffLine, FileDiff, LineRef},
        review::{CommentAnchor, PendingComment},
    },
    tui::ui::{
        components::diff_viewer::{DiffViewer, FocusedNav, PaneNav},
        icons, layout, theme,
        widgets::{
            self,
            comment::{fold::Folds, meta::Reading},
            markdown,
        },
    },
};
use chrono::Utc;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use std::collections::{HashMap, HashSet};

pub(super) const DIFF_GUTTER: &str = "  ";
const DIFF_GUTTER_COLS: u16 = 2;

#[expect(clippy::too_many_arguments, reason = "render inputs; see ROADMAP")]
pub(super) fn render(
    frame: &mut Frame<'_>,
    diff: &Diff,
    ui_diff: &mut DiffViewer,
    file_stats: &[(u32, u32)],
    threads: &[CommentThread],
    pending: &[PendingComment],
    focused: bool,
    reading: Reading<'_>,
    area: Rect,
) {
    ui_diff.pane_viewport = area.height.saturating_sub(4);

    let bounded = ui_diff.focused_file.min(diff.files.len().saturating_sub(1));
    let Some(file) = diff.files.get(bounded) else {
        return;
    };
    let (adds, dels) = file_stats.get(bounded).copied().unwrap_or((0, 0));

    let pane_cursor = ui_diff.pane_cursor;
    let current_scroll = ui_diff.pane_scroll;
    let query: &str = if ui_diff.pane_search.open {
        ""
    } else {
        ui_diff.pane_search.query.as_str()
    };

    let theme = theme::current();

    let (header_inner, body_area) = widgets::framed_panel(frame, area, "Code", focused);
    ui_diff.pane_viewport = body_area.height;
    let active = focused.then_some(pane_cursor);
    let DiffBody {
        mut lines,
        nav_items,
        matches,
    } = build_diff_body(
        file,
        diff.revision.as_ref(),
        threads,
        pending,
        body_area.width,
        active,
        query,
        Reading {
            folds: Folds::Long {
                opened: &ui_diff.opened_comments,
            },
            ..reading
        },
        &ui_diff.expanded_threads,
    );
    let cursor = active.and_then(|i| nav_items.get(i));

    if !query.is_empty() {
        let match_style = Style::default().fg(theme.bg).bg(theme.warning);
        for line in &mut lines {
            *line = widgets::highlight_query(std::mem::take(line), query, match_style);
        }
    }

    render_pane_header(frame, &file.path, adds, dels, cursor, header_inner);

    if let Some(m) = cursor
        && matches!(m.kind, NavKind::Line { .. } | NavKind::Fold { .. })
        && let Some(line) = lines.get_mut(m.rendered_row)
    {
        *line = highlight_row(std::mem::take(line), body_area.width as usize);
    }

    let visible = body_area.height as usize;
    let max_scroll = crate::tui::ui::component::saturating_u16(lines.len().saturating_sub(visible));
    // The same rule as in the Overview: move as little as possible, and show an
    // item taller than the viewport from its first row.
    let scroll = match cursor {
        Some(item) => crate::tui::ui::component::scroll_to_item(
            current_scroll,
            item.rendered_row,
            item.row_span,
            lines.len(),
            visible,
        ),
        None => current_scroll.min(max_scroll),
    };

    let paragraph = Paragraph::new(lines).scroll((scroll, 0));
    frame.render_widget(paragraph, body_area);

    if max_scroll > 0 {
        let bar = widgets::scrollbar(scroll, max_scroll, body_area.height);
        frame.render_widget(Paragraph::new(bar), layout::scrollbar_area(body_area));
    }

    let focused = cursor.map(|item| {
        let (line, removed) = item.anchor();
        FocusedNav {
            anchor: CommentAnchor {
                revision: diff.revision.clone(),
                path: file.path.clone(),
                line,
                removed,
            },
            target: item.target(),
        }
    });
    ui_diff.pane_scroll = scroll;
    ui_diff.pane = PaneNav {
        item_count: nav_items.len(),
        matches,
        focused,
    };
}

struct DiffBody {
    lines: Vec<Line<'static>>,
    nav_items: Vec<NavItem>,
    matches: Vec<usize>,
}

#[expect(clippy::too_many_arguments, reason = "render inputs; see ROADMAP")]
fn build_diff_body(
    file: &FileDiff,
    revision: Option<&crate::domain::diff::DiffRevision>,
    threads: &[CommentThread],
    pending: &[PendingComment],
    width: u16,
    active: Option<usize>,
    query: &str,
    reading: Reading<'_>,
    expanded: &HashSet<CommentId>,
) -> DiffBody {
    let theme = theme::current();
    let mut lines: Vec<Line<'_>> = Vec::new();
    let mut nav_items: Vec<NavItem> = Vec::new();
    let mut matches: Vec<usize> = Vec::new();
    let query_lower = query.to_lowercase();

    let (comments_at, comments_at_old) = index_comments(threads, &file.path, revision);
    let (pending_at, pending_at_old) = index_pending(pending, &file.path, revision);
    let now = Utc::now();
    let thread_width = width.saturating_sub(2 * DIFF_GUTTER_COLS);

    for hunk in &file.hunks {
        lines.push(Line::styled(
            format!("{DIFF_GUTTER}@@ -{} +{} @@", hunk.old_start, hunk.new_start),
            Style::default().fg(theme.info),
        ));

        for (diff_line, new_no, old_no) in hunk.numbered_lines() {
            let rendered_row = lines.len();
            lines.push(styled_diff_row(diff_line));

            let removed = matches!(diff_line, DiffLine::Removed(_));
            let line = if removed { old_no } else { new_no };

            let item_idx = nav_items.len();
            nav_items.push(NavItem {
                rendered_row,
                row_span: 1,
                kind: NavKind::Line { line, removed },
            });
            if !query_lower.is_empty() && diff_line.content().to_lowercase().contains(&query_lower)
            {
                matches.push(item_idx);
            }

            let threads_here = if removed {
                comments_at_old.get(&old_no)
            } else {
                comments_at.get(&new_no)
            };
            for thread in threads_here.into_iter().flatten() {
                push_thread(
                    &mut lines,
                    &mut nav_items,
                    thread,
                    (line, removed),
                    &ThreadDraw {
                        width: thread_width,
                        now,
                        active,
                        anchor_text: diff_line.content(),
                        reading,
                        expanded: thread.reply_to.is_none_or(|id| expanded.contains(&id)),
                    },
                );
            }

            let pending_here = if removed {
                pending_at_old.get(&old_no)
            } else {
                pending_at.get(&new_no)
            };
            for &(index, pc) in pending_here.into_iter().flatten() {
                let idx = nav_items.len();
                let start = lines.len();
                let span =
                    push_pending_lines(&mut lines, &pc.text, thread_width, active == Some(idx));
                nav_items.push(NavItem {
                    rendered_row: start,
                    row_span: span,
                    kind: NavKind::Pending {
                        line,
                        removed,
                        index,
                    },
                });
            }
        }
    }

    DiffBody {
        lines,
        nav_items,
        matches,
    }
}

fn styled_diff_row(diff_line: &DiffLine) -> Line<'static> {
    let theme = theme::current();
    let (marker, color) = match diff_line {
        DiffLine::Added(_) => ('+', theme.diff_added),
        DiffLine::Removed(_) => ('-', theme.diff_removed),
        DiffLine::Context(_) => (' ', theme.diff_context),
    };
    let text = Style::default().fg(color);
    let warning = Style::default()
        .fg(theme.warning)
        .add_modifier(Modifier::BOLD);
    let mut spans = vec![Span::styled(format!("{DIFF_GUTTER}{marker}"), text)];
    spans.extend(widgets::spans(
        widgets::reveal(diff_line.content()),
        text,
        warning,
    ));
    Line::from(spans).style(text)
}

type CommentIndex<'a> = HashMap<usize, Vec<&'a CommentThread>>;

fn index_comments<'a>(
    threads: &'a [CommentThread],
    path: &str,
    revision: Option<&crate::domain::diff::DiffRevision>,
) -> (CommentIndex<'a>, CommentIndex<'a>) {
    let mut by_new: CommentIndex<'_> = HashMap::new();
    let mut by_old: CommentIndex<'_> = HashMap::new();
    // Only anchored (code-review) threads land in the diff; general discussion
    // has no path/line and is skipped.
    for thread in threads {
        let Some(anchor) = &thread.anchor else {
            continue;
        };
        if anchor.path != path || !thread.matches_revision(revision) {
            continue;
        }
        match anchor.line {
            Some(LineRef::New(line)) => by_new.entry(line).or_default().push(thread),
            Some(LineRef::Old(line)) => by_old.entry(line).or_default().push(thread),
            None => {}
        }
    }
    (by_new, by_old)
}

type PendingIndex<'a> = HashMap<usize, Vec<(usize, &'a PendingComment)>>;

/// Bucket the queued review comments for `path` by their anchor line, carrying
/// each one's index in the original `pending` slice (so `d` can remove it).
fn index_pending<'a>(
    pending: &'a [PendingComment],
    path: &str,
    revision: Option<&crate::domain::diff::DiffRevision>,
) -> (PendingIndex<'a>, PendingIndex<'a>) {
    let mut by_new: PendingIndex<'_> = HashMap::new();
    let mut by_old: PendingIndex<'_> = HashMap::new();
    for (i, pc) in pending.iter().enumerate() {
        if pc.anchor.path != path || pc.anchor.revision.as_ref() != revision {
            continue;
        }
        let bucket = if pc.anchor.removed {
            &mut by_old
        } else {
            &mut by_new
        };
        bucket.entry(pc.anchor.line).or_default().push((i, pc));
    }
    (by_new, by_old)
}

/// A queued review comment, rendered as a draft block (accent bar + `pending`
/// tag) so it reads as not-yet-posted. Returns the row count for nav spans.
fn push_pending_lines(
    lines: &mut Vec<Line<'static>>,
    text: &str,
    width: u16,
    active: bool,
) -> usize {
    let theme = theme::current();
    let bar = Style::default().fg(theme.accent);
    let body_style = Style::default().fg(theme.fg);
    let row_style = if active {
        Style::default().bg(theme.highlight_bg)
    } else {
        Style::default()
    };
    let text_width = width.saturating_sub(2);

    let mut block: Vec<Line<'static>> = vec![Line::from(vec![
        Span::styled(format!("{DIFF_GUTTER}▌ "), bar),
        Span::styled(
            "pending",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
    ])];
    for body_line in markdown::render_no_margin(text, text_width) {
        let mut spans = vec![Span::styled(format!("{DIFF_GUTTER}▌ "), bar)];
        spans.extend(
            body_line
                .spans
                .into_iter()
                .map(|s| Span::styled(s.content, body_style)),
        );
        block.push(Line::from(spans));
    }

    let count = block.len();
    for line in block {
        lines.push(line.style(row_style));
    }
    count
}

fn render_pane_header(
    frame: &mut Frame<'_>,
    path: &str,
    adds: u32,
    dels: u32,
    cursor: Option<&NavItem>,
    area: Rect,
) {
    let theme = theme::current();
    let inner_w = (area.width as usize).saturating_sub(4);
    let stats_w = format!("+{adds} -{dels}").chars().count();
    let position = cursor.map(|m| {
        let (line, removed) = m.anchor();
        match m.kind {
            NavKind::Thread(_) | NavKind::Fold { .. } => {
                format!("  {} L{line}", icons::COMMENT)
            }
            NavKind::Pending { .. } => format!("  {} L{line} (pending)", icons::COMMENT),
            NavKind::Line { .. } if removed => format!("  L{line} (old)"),
            NavKind::Line { .. } => format!("  L{line}"),
        }
    });
    let position_width = position.as_ref().map_or(0, |p| Span::raw(p).width());
    let displayed_path =
        elide_path_start(path, inner_w.saturating_sub(stats_w + position_width + 2));

    let mut left = vec![
        Span::raw("  "),
        Span::styled(
            displayed_path,
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ),
    ];
    if let Some(label) = position {
        left.push(Span::styled(label, Style::default().fg(theme.muted)));
    }
    let right = vec![
        Span::styled(format!("+{adds}"), Style::default().fg(theme.diff_added)),
        Span::raw(" "),
        Span::styled(format!("-{dels}"), Style::default().fg(theme.diff_removed)),
    ];
    let line = widgets::fitted_row(left, right, area.width.saturating_sub(2) as usize);
    frame.render_widget(Paragraph::new(line), area);
}

fn elide_path_start(path: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if Span::raw(path).width() <= max {
        return path.to_owned();
    }
    let mut used = 1;
    let mut tail = Vec::new();
    for ch in path.chars().rev() {
        let width = Span::raw(ch.to_string()).width();
        if used + width > max {
            break;
        }
        used += width;
        tail.push(ch);
    }
    format!("…{}", tail.into_iter().rev().collect::<String>())
}

fn highlight_row(line: Line<'static>, row_w: usize) -> Line<'static> {
    let bg = theme::current().highlight_bg;
    let visible: usize = line.spans.iter().map(Span::width).sum();
    let mut spans: Vec<Span<'static>> = line
        .spans
        .into_iter()
        .map(|s| Span::styled(s.content, s.style.bg(bg)))
        .collect();
    let pad = row_w.saturating_sub(visible);
    if pad > 0 {
        spans.push(Span::styled(" ".repeat(pad), Style::default().bg(bg)));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn long_diff_paths_leave_room_for_position_and_stats() {
        let path = "src/非常に長いディレクトリ/nested/component.rs";
        let cursor = NavItem {
            rendered_row: 0,
            row_span: 1,
            kind: NavKind::Line {
                line: 1234,
                removed: true,
            },
        };
        for width in [40, 80] {
            let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
            terminal
                .draw(|frame| render_pane_header(frame, path, 123, 45, Some(&cursor), frame.area()))
                .unwrap();
            let text: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect();
            assert!(text.contains("L1234 (old)"));
            assert!(text.contains("+123 -45"));
        }
        for width in 0..60 {
            assert!(Span::raw(elide_path_start(path, width)).width() <= width);
        }
    }
}

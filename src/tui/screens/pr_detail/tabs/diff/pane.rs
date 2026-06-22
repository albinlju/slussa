use std::collections::{HashMap, HashSet};

use chrono::Utc;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    app::state::{CommentAnchor, DiffViewState, ThreadRef},
    domain::{
        comment::CommentThread,
        diff::{Diff, DiffLine, FileDiff},
    },
    tui::{icons, layout, screens::pr_detail::render_inline_thread, theme, widgets},
};

const DIFF_GUTTER: &str = "  ";
const DIFF_GUTTER_COLS: u16 = 2;

#[allow(clippy::too_many_arguments)]
pub(super) fn render(
    frame: &mut Frame,
    diff: &Diff,
    ui_diff: &mut DiffViewState,
    file_stats: &[(u32, u32)],
    threads: &[CommentThread],
    focused: bool,
    author: &str,
    area: Rect,
) {
    ui_diff.pane_viewport = area.height.saturating_sub(4);

    let bounded = ui_diff.focused_file.min(diff.files.len().saturating_sub(1));
    let Some(file) = diff.files.get(bounded) else {
        ui_diff.pane_item_count = 0;
        ui_diff.pane_matches = Vec::new();
        ui_diff.pane_anchor = None;
        ui_diff.pane_reply = None;
        ui_diff.pane_thread = None;
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

    let (header_inner, body_area) = widgets::framed_panel(frame, area);
    let active = focused.then_some(pane_cursor);
    let DiffBody {
        mut lines,
        nav_items,
        matches,
    } = build_diff_body(
        file,
        threads,
        body_area.width,
        active,
        query,
        author,
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
        && matches!(m.kind, NavKind::Line { .. })
        && m.rendered_row < lines.len()
    {
        let row = m.rendered_row;
        lines[row] = highlight_row(std::mem::take(&mut lines[row]), body_area.width as usize);
    }

    let visible = body_area.height as usize;
    let max_scroll = lines.len().saturating_sub(visible) as u16;
    let scroll = scroll_to_cursor(current_scroll, cursor, lines.len(), visible);

    let paragraph = Paragraph::new(lines).scroll((scroll, 0));
    frame.render_widget(paragraph, body_area);

    if max_scroll > 0 {
        let bar = widgets::scrollbar(scroll, max_scroll, body_area.height);
        frame.render_widget(Paragraph::new(bar), layout::scrollbar_area(body_area));
    }

    ui_diff.pane_anchor = cursor.map(|m| {
        let (line, removed) = m.anchor();
        CommentAnchor {
            path: file.path.clone(),
            line,
            removed,
        }
    });
    ui_diff.pane_reply = cursor.and_then(NavItem::reply_to);
    ui_diff.pane_thread = cursor.and_then(NavItem::thread_ref);
    ui_diff.pane_scroll = scroll;
    ui_diff.pane_item_count = nav_items.len();
    ui_diff.pane_matches = matches;
}

enum NavKind {
    Line {
        line: usize,
        removed: bool,
    },
    Thread {
        line: usize,
        removed: bool,
        reply_to: Option<u64>,
        node_id: Option<String>,
        resolved: bool,
    },
}

struct NavItem {
    rendered_row: usize,
    row_span: usize,
    kind: NavKind,
}

impl NavItem {
    fn anchor(&self) -> (usize, bool) {
        match self.kind {
            NavKind::Line { line, removed } | NavKind::Thread { line, removed, .. } => {
                (line, removed)
            }
        }
    }

    fn reply_to(&self) -> Option<u64> {
        match self.kind {
            NavKind::Thread { reply_to, .. } => reply_to,
            NavKind::Line { .. } => None,
        }
    }

    fn thread_ref(&self) -> Option<ThreadRef> {
        match &self.kind {
            NavKind::Thread {
                reply_to,
                node_id,
                resolved,
                ..
            } => Some(ThreadRef {
                node_id: node_id.clone(),
                comment_id: *reply_to,
                resolved: *resolved,
            }),
            NavKind::Line { .. } => None,
        }
    }
}

struct DiffBody {
    lines: Vec<Line<'static>>,
    nav_items: Vec<NavItem>,
    matches: Vec<usize>,
}

#[allow(clippy::too_many_arguments)]
fn build_diff_body(
    file: &FileDiff,
    threads: &[CommentThread],
    width: u16,
    active: Option<usize>,
    query: &str,
    author: &str,
    expanded: &HashSet<u64>,
) -> DiffBody {
    let theme = theme::current();
    let mut lines: Vec<Line> = Vec::new();
    let mut nav_items: Vec<NavItem> = Vec::new();
    let mut matches: Vec<usize> = Vec::new();
    let query_lower = query.to_lowercase();

    let (comments_at, comments_at_old) = index_comments(threads, &file.path);
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
                let idx = nav_items.len();
                let start = lines.len();
                // Resolved threads stay collapsed unless the user expanded this one.
                let is_expanded = thread.reply_to.is_none_or(|id| expanded.contains(&id));
                let span = push_thread_lines(
                    &mut lines,
                    thread,
                    thread_width,
                    now,
                    active == Some(idx),
                    diff_line.content(),
                    author,
                    is_expanded,
                );
                nav_items.push(NavItem {
                    rendered_row: start,
                    row_span: span,
                    kind: NavKind::Thread {
                        line,
                        removed,
                        reply_to: thread.reply_to,
                        node_id: thread.anchor.as_ref().and_then(|a| a.node_id.clone()),
                        resolved: thread.resolved(),
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
    Line::styled(
        format!("{DIFF_GUTTER}{marker}{}", diff_line.content()),
        Style::default().fg(color),
    )
}

type CommentIndex<'a> = HashMap<usize, Vec<&'a CommentThread>>;

fn index_comments<'a>(
    threads: &'a [CommentThread],
    path: &str,
) -> (CommentIndex<'a>, CommentIndex<'a>) {
    let mut by_new: CommentIndex = HashMap::new();
    let mut by_old: CommentIndex = HashMap::new();
    // Only anchored (code-review) threads land in the diff; general discussion
    // has no path/line and is skipped.
    for thread in threads {
        let Some(anchor) = &thread.anchor else { continue };
        if anchor.path != path {
            continue;
        }
        if let Some(line) = anchor.line {
            by_new.entry(line).or_default().push(thread);
        } else if let Some(old) = anchor.old_line {
            by_old.entry(old).or_default().push(thread);
        }
    }
    (by_new, by_old)
}

#[allow(clippy::too_many_arguments)]
fn push_thread_lines(
    lines: &mut Vec<Line<'static>>,
    thread: &CommentThread,
    thread_width: u16,
    now: chrono::DateTime<Utc>,
    active: bool,
    anchor_text: &str,
    author: &str,
    expanded: bool,
) -> usize {
    let rendered = render_inline_thread(
        thread,
        thread_width,
        now,
        active,
        Some(anchor_text),
        author,
        expanded,
    );
    let count = rendered.len();
    for tline in rendered {
        let line_style = tline.style;
        let mut spans: Vec<Span<'static>> = Vec::with_capacity(tline.spans.len() + 1);
        spans.push(Span::raw(DIFF_GUTTER));
        spans.extend(tline.spans);
        lines.push(Line::from(spans).style(line_style));
    }
    count
}

fn render_pane_header(
    frame: &mut Frame,
    path: &str,
    adds: u32,
    dels: u32,
    cursor: Option<&NavItem>,
    area: Rect,
) {
    let theme = theme::current();
    let inner_w = (area.width as usize).saturating_sub(4);
    let stats_w = format!("+{adds} -{dels}").chars().count();
    let displayed_path = elide_path_start(path, inner_w.saturating_sub(stats_w + 2));

    let mut left = vec![
        Span::raw("  "),
        Span::styled(
            displayed_path,
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ),
    ];
    if let Some(m) = cursor {
        let (line, removed) = m.anchor();
        let is_thread = matches!(m.kind, NavKind::Thread { .. });
        let label = if is_thread {
            format!("  {} L{line}", icons::COMMENT)
        } else if removed {
            format!("  L{line} (old)")
        } else {
            format!("  L{line}")
        };
        left.push(Span::styled(label, Style::default().fg(theme.muted)));
    }
    let right = vec![
        Span::styled(format!("+{adds}"), Style::default().fg(theme.diff_added)),
        Span::raw(" "),
        Span::styled(format!("-{dels}"), Style::default().fg(theme.diff_removed)),
    ];
    let line = Line::from(widgets::justify_between(left, right, inner_w + 2));
    frame.render_widget(Paragraph::new(line), area);
}

fn elide_path_start(path: &str, max: usize) -> String {
    let total = path.chars().count();
    if total <= max || max < 2 {
        return path.to_string();
    }
    let keep = max.saturating_sub(1);
    let skip = total.saturating_sub(keep);
    let tail: String = path.chars().skip(skip).collect();
    format!("…{tail}")
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

fn scroll_to_cursor(current: u16, cursor: Option<&NavItem>, total: usize, visible: usize) -> u16 {
    let max_scroll = total.saturating_sub(visible) as u16;
    let mut scroll = current.min(max_scroll) as usize;
    if let Some(m) = cursor {
        let top = m.rendered_row;
        let bottom = m.rendered_row + m.row_span.saturating_sub(1);
        if top < scroll {
            scroll = top;
        } else if visible > 0 && bottom >= scroll + visible {
            scroll = bottom + 1 - visible;
        }
    }
    (scroll as u16).min(max_scroll)
}

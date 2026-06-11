//! The Diff tab's diff pane: file-path/stats header band, then the diff body
//! with inline review-thread boxes anchored to their lines.

use std::collections::HashMap;

use chrono::Utc;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::{
    domain::{
        comment::ReviewThread,
        diff::{Diff, DiffLine, FileDiff},
    },
    tui::{screens::pr_detail::render_inline_thread, theme, widgets},
};

/// 2-col gutter on each side of the diff body; the row's background tint
/// extends across it to the box borders.
const DIFF_GUTTER: &str = "  ";
const DIFF_GUTTER_COLS: u16 = 2;

#[allow(clippy::too_many_arguments)]
pub(super) fn render(
    frame: &mut Frame,
    diff: &Diff,
    focused_file: usize,
    pane_scroll: &mut u16,
    pane_cursor: usize,
    file_stats: &[(u32, u32)],
    threads: &[ReviewThread],
    focused: bool,
    search_query: &str,
    area: Rect,
) -> (usize, Vec<usize>) {
    let bounded = focused_file.min(diff.files.len().saturating_sub(1));
    let Some(file) = diff.files.get(bounded) else {
        return (0, Vec::new());
    };
    let (adds, dels) = file_stats.get(bounded).copied().unwrap_or((0, 0));

    let theme = theme::current();

    let border_color = if focused { theme.accent } else { theme.divider };
    let pane_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));
    let pane_inner = pane_block.inner(area);
    frame.render_widget(pane_block, area);

    let pane_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // header row + bottom divider
            Constraint::Min(0),    // diff body
        ])
        .split(pane_inner);

    let body_area = pane_chunks[1];
    // The cursor is only live while the pane has the keyboard.
    let active = focused.then_some(pane_cursor);
    let (mut lines, meta, matches) =
        file_to_lines(file, threads, body_area.width, active, search_query);
    let cursor = active.and_then(|i| meta.get(i));

    // Search highlight: tint every match across the diff body. Applied before
    // the cursor tint so the cursor row still reads as the cursor.
    if !search_query.is_empty() {
        let match_style = Style::default().fg(theme.bg).bg(theme.warning);
        for line in &mut lines {
            *line = widgets::highlight_query(std::mem::take(line), search_query, match_style);
        }
    }

    render_pane_header(
        frame,
        &file.path,
        adds,
        dels,
        cursor.map(|m| {
            let (line, removed) = m.line_removed();
            (line, removed, matches!(m.kind, NavKind::Thread { .. }))
        }),
        pane_chunks[0],
    );

    // A line cursor tints its row; a thread cursor is already marked by the
    // box's accent border.
    if let Some(m) = cursor
        && matches!(m.kind, NavKind::Line { .. })
        && m.rendered_row < lines.len()
    {
        let row = m.rendered_row;
        lines[row] = highlight_row(std::mem::take(&mut lines[row]), body_area.width as usize);
    }

    let total = lines.len();
    let visible = body_area.height as usize;
    let max_scroll = total.saturating_sub(visible) as u16;

    // Keep the whole cursor item in view; otherwise honour the stored offset.
    let mut scroll = (*pane_scroll).min(max_scroll) as usize;
    if let Some(m) = cursor {
        let top = m.rendered_row;
        let bottom = m.rendered_row + m.row_span.saturating_sub(1);
        if top < scroll {
            scroll = top;
        } else if visible > 0 && bottom >= scroll + visible {
            scroll = bottom + 1 - visible;
        }
    }
    let scroll = (scroll as u16).min(max_scroll);
    *pane_scroll = scroll;

    let paragraph = Paragraph::new(lines).scroll((scroll, 0));
    frame.render_widget(paragraph, body_area);

    if max_scroll > 0 {
        let bar = widgets::scrollbar(scroll, max_scroll, body_area.height);
        frame.render_widget(Paragraph::new(bar), widgets::scrollbar_area(body_area));
    }

    (meta.len(), matches)
}

fn render_pane_header(
    frame: &mut Frame,
    path: &str,
    adds: u32,
    dels: u32,
    cursor: Option<(usize, bool, bool)>,
    area: Rect,
) {
    let theme = theme::current();

    let header_block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(theme.divider));
    let header_inner = header_block.inner(area);
    frame.render_widget(header_block, area);

    // 2-col padding on each side inside the header band.
    let inner_w = (header_inner.width as usize).saturating_sub(4);
    let stats_w = "+".len() + adds.to_string().len() + 1 + "-".len() + dels.to_string().len();
    let displayed_path = truncate_path_left(path, inner_w.saturating_sub(stats_w + 2));

    let mut left = vec![
        Span::raw("  "),
        Span::styled(
            displayed_path,
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ),
    ];
    if let Some((line, removed, is_thread)) = cursor {
        let label = if is_thread {
            format!("  \u{f075} L{line}") //  comment on this line
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
    frame.render_widget(Paragraph::new(line), header_inner);
}

/// Truncate from the left (`…tail`) so the filename stays visible.
fn truncate_path_left(path: &str, max: usize) -> String {
    let total = path.chars().count();
    if total <= max || max < 2 {
        return path.to_string();
    }
    let keep = max.saturating_sub(1);
    let skip = total.saturating_sub(keep);
    let tail: String = path.chars().skip(skip).collect();
    format!("…{tail}")
}

fn styled_diff_line(diff_line: &DiffLine, width: u16) -> Line<'static> {
    let theme = theme::current();
    let row_w = width as usize;
    match diff_line {
        DiffLine::Added(c) => widgets::diff_bg_row(
            DIFF_GUTTER,
            "+",
            c,
            theme.diff_added,
            theme.diff_added_bg,
            theme.fg,
            row_w,
        ),
        DiffLine::Removed(c) => widgets::diff_bg_row(
            DIFF_GUTTER,
            "-",
            c,
            theme.diff_removed,
            theme.diff_removed_bg,
            theme.fg,
            row_w,
        ),
        DiffLine::Context(c) => Line::styled(
            format!("{DIFF_GUTTER} {c}"),
            Style::default().fg(theme.diff_context),
        ),
    }
}

enum NavKind {
    Line { line: usize, removed: bool },
    Thread { line: usize, removed: bool },
}

/// A cursor stop in the pane (diff line or thread box), indexed in render
/// order to match `pane_cursor`. `row_span` is the box height for threads so
/// scrolling can keep the whole box in view.
struct NavItem {
    rendered_row: usize,
    row_span: usize,
    kind: NavKind,
}

impl NavItem {
    fn line_removed(&self) -> (usize, bool) {
        match self.kind {
            NavKind::Line { line, removed } | NavKind::Thread { line, removed } => (line, removed),
        }
    }
}

fn file_to_lines(
    file: &FileDiff,
    threads: &[ReviewThread],
    width: u16,
    active: Option<usize>,
    query: &str,
) -> (Vec<Line<'static>>, Vec<NavItem>, Vec<usize>) {
    let theme = theme::current();
    let mut lines: Vec<Line> = Vec::new();
    let mut meta: Vec<NavItem> = Vec::new();
    // Nav-item indices of diff lines containing the (applied) search query.
    let mut matches: Vec<usize> = Vec::new();
    let query_lower = query.to_lowercase();

    // Threads on added/context lines key off the new-file line number,
    // threads on removed lines off the old-file number.
    let mut comments_at: HashMap<usize, Vec<&ReviewThread>> = HashMap::new();
    let mut comments_at_old: HashMap<usize, Vec<&ReviewThread>> = HashMap::new();
    for thread in threads.iter().filter(|t| t.path == file.path) {
        if let Some(line) = thread.line {
            comments_at.entry(line).or_default().push(thread);
        } else if let Some(old) = thread.old_line {
            comments_at_old.entry(old).or_default().push(thread);
        }
    }

    let now = Utc::now();

    // Thread boxes render narrower than the diff rows so the row bg visibly
    // flows past the box on both sides.
    let thread_width = width.saturating_sub(2 * DIFF_GUTTER_COLS);

    for hunk in &file.hunks {
        let old_start = hunk.old_start;
        let new_start = hunk.new_start;
        lines.push(Line::styled(
            format!("{DIFF_GUTTER}@@ -{old_start} +{new_start} @@"),
            Style::default().fg(theme.info),
        ));

        let mut new_line_num = hunk.new_start;
        let mut old_line_num = hunk.old_start;
        for diff_line in &hunk.lines {
            let rendered_row = lines.len();
            lines.push(styled_diff_line(diff_line, width));
            let (line, removed) = match diff_line {
                DiffLine::Removed(_) => (old_line_num, true),
                _ => (new_line_num, false),
            };
            let item_idx = meta.len();
            meta.push(NavItem {
                rendered_row,
                row_span: 1,
                kind: NavKind::Line { line, removed },
            });
            if !query_lower.is_empty() {
                let text = match diff_line {
                    DiffLine::Added(t) | DiffLine::Removed(t) | DiffLine::Context(t) => t,
                };
                if text.to_lowercase().contains(&query_lower) {
                    matches.push(item_idx);
                }
            }

            let threads_here = match diff_line {
                DiffLine::Removed(_) => comments_at_old.get(&old_line_num),
                _ => comments_at.get(&new_line_num),
            };
            if let Some(threads_here) = threads_here {
                let anchor_text = match diff_line {
                    DiffLine::Added(t) | DiffLine::Removed(t) | DiffLine::Context(t) => t.as_str(),
                };
                for thread in threads_here {
                    let idx = meta.len();
                    let start = lines.len();
                    let span = push_thread_lines(
                        &mut lines,
                        thread,
                        thread_width,
                        now,
                        active == Some(idx),
                        anchor_text,
                    );
                    meta.push(NavItem {
                        rendered_row: start,
                        row_span: span,
                        kind: NavKind::Thread { line, removed },
                    });
                }
            }

            match diff_line {
                DiffLine::Added(_) => new_line_num += 1,
                DiffLine::Removed(_) => old_line_num += 1,
                DiffLine::Context(_) => {
                    new_line_num += 1;
                    old_line_num += 1;
                }
            }
        }
    }

    (lines, meta, matches)
}

/// Re-tint a row with the cursor bg, keeping each span's fg. Pads to `row_w`
/// since context rows aren't otherwise padded and would only tint under
/// their text.
fn highlight_row(line: Line<'static>, row_w: usize) -> Line<'static> {
    let bg = theme::current().highlight_bg;
    let visible: usize = line.spans.iter().map(|s| s.width()).sum();
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

/// Render one review thread into the diff body, gutter-prefixed to align
/// with the diff rows. Returns the box height for the caller's `row_span`.
fn push_thread_lines(
    lines: &mut Vec<Line<'static>>,
    thread: &ReviewThread,
    thread_width: u16,
    now: chrono::DateTime<Utc>,
    active: bool,
    anchor_text: &str,
) -> usize {
    let rendered = render_inline_thread(thread, thread_width, now, active, Some(anchor_text));
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

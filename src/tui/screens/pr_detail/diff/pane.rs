//! Right side of the Diff tab — outer rounded box, the file-path/stats
//! header band, and the diff body below (hunk headers, +/-/context rows,
//! inline review-thread boxes anchored to specific lines).

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

/// 2-col gutter on each side of the diff body — the content (prefix +
/// text) sits inside this gutter, but the background tint extends all the
/// way across the row to the box's borders.
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
    area: Rect,
) {
    let bounded = focused_file.min(diff.files.len().saturating_sub(1));
    let Some(file) = diff.files.get(bounded) else {
        return;
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

    // Diff body fills the full `pane_inner` width so the row-background tint
    // on +/- lines flows to the box's left/right borders. Each line's own
    // padding handles the 2-col left/right gutter for the content itself.
    let body_area = pane_chunks[1];
    let (mut lines, meta) = file_to_lines(file, threads, body_area.width);

    // The cursor is only live when the pane has the keyboard; otherwise the
    // tree owns navigation and we leave the body unmarked.
    let cursor = if focused { meta.get(pane_cursor) } else { None };

    render_pane_header(
        frame,
        &file.path,
        adds,
        dels,
        cursor.map(|m| (m.line, m.removed)),
        pane_chunks[0],
    );

    if let Some(m) = cursor
        && m.rendered_row < lines.len()
    {
        let row = m.rendered_row;
        lines[row] = highlight_row(std::mem::take(&mut lines[row]));
    }

    let total = lines.len();
    let visible = body_area.height as usize;
    let max_scroll = total.saturating_sub(visible) as u16;

    // Keep the cursor row in view; otherwise honour the stored offset.
    let mut scroll = (*pane_scroll).min(max_scroll) as usize;
    if let Some(m) = cursor {
        let cy = m.rendered_row;
        if cy < scroll {
            scroll = cy;
        } else if visible > 0 && cy >= scroll + visible {
            scroll = cy + 1 - visible;
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
}

fn render_pane_header(
    frame: &mut Frame,
    path: &str,
    adds: u32,
    dels: u32,
    cursor: Option<(usize, bool)>,
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
    // Show where the cursor sits / where a comment would anchor.
    if let Some((line, removed)) = cursor {
        let label = if removed {
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

/// Truncate from the left if the path exceeds `max`, keeping the filename
/// visible and prefixing "…" so the reader sees the end of the path.
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

/// Build a single diff line with full-row background fill via the shared
/// `widgets::diff_bg_row` helper. Context lines have no bg tint, only the
/// gutter for alignment with `+`/`-` rows above and below.
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

/// Where a logical diff line landed once rendered, plus the anchor a comment on
/// it would use. Indexed by diff-line order (matching `pane_cursor`); the pane
/// uses it to highlight/scroll to the cursor and (later) to post comments.
struct DiffLineMeta {
    /// Index into the returned `Vec<Line>`.
    rendered_row: usize,
    /// File line number — new side, or old side for removed lines.
    line: usize,
    removed: bool,
}

fn file_to_lines(
    file: &FileDiff,
    threads: &[ReviewThread],
    width: u16,
) -> (Vec<Line<'static>>, Vec<DiffLineMeta>) {
    let theme = theme::current();
    let mut lines: Vec<Line> = Vec::new();
    let mut meta: Vec<DiffLineMeta> = Vec::new();
    // File path lives in the pane header above us — don't repeat it here.

    // Build two lookups: threads on added/context lines key off the new-file
    // line number, threads on removed lines key off the old-file line number.
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

    // Inline-thread box is rendered narrower than the diff row so the colored
    // bg around it visibly flows past the box on both sides. Width subtracts
    // 4 cols (2 each side); the box itself is then offset by 2 cols on its
    // left when added to the lines.
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
            meta.push(DiffLineMeta {
                rendered_row,
                line,
                removed,
            });

            // Anchor threads to the line just rendered: removed lines match on
            // the old-file line number, added/context on the new-file number.
            let threads_here = match diff_line {
                DiffLine::Removed(_) => comments_at_old.get(&old_line_num),
                _ => comments_at.get(&new_line_num),
            };
            if let Some(threads_here) = threads_here {
                for thread in threads_here {
                    push_thread_lines(&mut lines, thread, thread_width, now);
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

    (lines, meta)
}

/// Re-tint a whole row with the cursor highlight, keeping each span's fg so the
/// `+`/`-` colors still read through.
fn highlight_row(line: Line<'static>) -> Line<'static> {
    let bg = theme::current().highlight_bg;
    Line::from(
        line.spans
            .into_iter()
            .map(|s| {
                let style = s.style.bg(bg);
                Span::styled(s.content, style)
            })
            .collect::<Vec<_>>(),
    )
}

/// Render one review thread into the diff body, each line prefixed by the
/// left gutter so the box aligns with the +/- rows above it.
fn push_thread_lines(
    lines: &mut Vec<Line<'static>>,
    thread: &ReviewThread,
    thread_width: u16,
    now: chrono::DateTime<Utc>,
) {
    for tline in render_inline_thread(thread, thread_width, now) {
        let line_style = tline.style;
        let mut spans: Vec<Span<'static>> = Vec::with_capacity(tline.spans.len() + 1);
        spans.push(Span::raw(DIFF_GUTTER));
        spans.extend(tline.spans);
        lines.push(Line::from(spans).style(line_style));
    }
}

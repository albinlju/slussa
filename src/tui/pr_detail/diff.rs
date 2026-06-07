use std::collections::HashMap;

use chrono::Utc;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
};

use crate::{
    app::state::{DiffViewState, LoadState, PrData},
    domain::{
        comment::ReviewThread,
        diff::{Diff, DiffLine, FileDiff},
        pr::PullRequest,
    },
    tui::{
        pr_detail::{
            diff_bg_row,
            file_tree::{TreeRow, build_visible_rows},
            render_inline_thread,
        },
        spinner_frame, theme,
    },
};

pub fn render(
    frame: &mut Frame,
    _pr: &PullRequest,
    pr_data: Option<&PrData>,
    ui_diff: &DiffViewState,
    area: Rect,
) {
    let diff_state = pr_data.map(|d| &d.diff);
    let review_threads: &[ReviewThread] = pr_data
        .and_then(|d| match &d.review_threads {
            LoadState::Loaded(t) => Some(t.as_slice()),
            _ => None,
        })
        .unwrap_or(&[]);

    let theme = theme::current();
    match diff_state {
        None | Some(LoadState::NotRequested) | Some(LoadState::Loading) => {
            let paragraph = Paragraph::new(format!("{} Loading diff...", spinner_frame()))
                .style(Style::default().fg(theme.warning));
            frame.render_widget(paragraph, area);
        }
        Some(LoadState::Failed(msg)) => {
            let paragraph = Paragraph::new(format!("Couldn't load diff: {msg}"))
                .style(Style::default().fg(theme.error));
            frame.render_widget(paragraph, area);
        }
        Some(LoadState::Loaded(diff)) if diff.files.is_empty() => {
            let paragraph = Paragraph::new("(no diff)").style(Style::default().fg(theme.muted));
            frame.render_widget(paragraph, area);
        }
        Some(LoadState::Loaded(diff)) => {
            // 1-col empty gap between tree and diff so the tree's content
            // doesn't touch the diff box's left border, while we still rely
            // on the diff's `│┌└` glyphs as the single visual divider.
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(28),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ])
                .split(area);

            // Per-file (+adds, -dels) counted from the hunk lines — FileDiff
            // doesn't carry stats so we tally them here. Totals and the file
            // count are derived inside `render_tree` from this same slice.
            let file_stats: Vec<(u32, u32)> =
                diff.files.iter().map(count_file_stats).collect();

            let rows = build_visible_rows(&diff.files, &ui_diff.collapsed);
            render_tree(frame, &rows, ui_diff.cursor, &file_stats, chunks[0]);
            render_diff_pane(
                frame,
                diff,
                ui_diff.focused_file,
                &file_stats,
                review_threads,
                chunks[2],
            );
        }
    }
}

fn render_tree(
    frame: &mut Frame,
    rows: &[TreeRow],
    cursor: usize,
    file_stats: &[(u32, u32)],
    area: Rect,
) {
    let theme = theme::current();
    let file_count = file_stats.len();
    let (total_adds, total_dels) = file_stats
        .iter()
        .fold((0u32, 0u32), |(a, d), (na, nd)| (a + na, d + nd));

    // Full box around the tree so it mirrors the diff pane's framing — the
    // corners line up at the same y as the diff box.
    let tree_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.divider));
    let tree_inner = tree_block.inner(area);
    frame.render_widget(tree_block, area);

    let tree_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // header row + bottom divider
            Constraint::Min(0),    // file list
        ])
        .split(tree_inner);

    // Header: "N files   +A -B" with a bottom divider line below.
    let header_block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(theme.divider));
    let header_inner = header_block.inner(tree_chunks[0]);
    frame.render_widget(header_block, tree_chunks[0]);
    let files_label = format!("{file_count} files");
    let plus = format!("+{total_adds}");
    let minus = format!("-{total_dels}");
    let header_w = header_inner.width as usize;
    let visible_right = plus.chars().count() + 1 + minus.chars().count();
    // Same trailing 1-col gap before the right edge as the file rows below,
    // so the +A -D blocks vertically align.
    let header_pad = header_w
        .saturating_sub(files_label.chars().count() + visible_right + 1)
        .max(1);
    let header_line = Line::from(vec![
        Span::styled(
            files_label,
            Style::default()
                .fg(theme.fg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(header_pad)),
        Span::styled(plus, Style::default().fg(theme.diff_added)),
        Span::raw(" "),
        Span::styled(minus, Style::default().fg(theme.diff_removed)),
    ]);
    frame.render_widget(Paragraph::new(header_line), header_inner);

    // File-row width: inner is `tree_inner` and the list lives in
    // `tree_chunks[1]` — both have the same width since the right border was
    // consumed by `tree_block`.
    let row_width = tree_chunks[1].width as usize;

    let items: Vec<ListItem> = rows
        .iter()
        .map(|row| {
            let line = match row {
                TreeRow::Dir {
                    name,
                    depth,
                    expanded,
                    ..
                } => {
                    let marker = if *expanded { "▼" } else { "▶" };
                    let indent = "  ".repeat(*depth);
                    Line::from(vec![
                        Span::raw(indent),
                        Span::styled(format!("{marker} "), Style::default().fg(theme.muted)),
                        Span::styled(
                            format!("{name}/"),
                            Style::default()
                                .fg(theme.link)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ])
                }
                TreeRow::File {
                    name,
                    depth,
                    file_index,
                } => {
                    let (adds, dels) = file_stats.get(*file_index).copied().unwrap_or((0, 0));
                    let indent = "  ".repeat(*depth + 1);
                    let plus = format!("+{adds}");
                    let minus = format!("-{dels}");
                    let visible_left = indent.chars().count() + name.chars().count();
                    let visible_right = plus.chars().count() + 1 + minus.chars().count();
                    let pad = row_width
                        .saturating_sub(visible_left + visible_right + 1)
                        .max(1);
                    Line::from(vec![
                        Span::raw(indent),
                        Span::raw(name.clone()),
                        Span::raw(" ".repeat(pad)),
                        Span::styled(plus, Style::default().fg(theme.diff_added)),
                        Span::raw(" "),
                        Span::styled(minus, Style::default().fg(theme.diff_removed)),
                    ])
                }
            };
            ListItem::new(line)
        })
        .collect();

    let bounded_cursor = cursor.min(rows.len().saturating_sub(1));
    let mut list_state = ListState::default();
    list_state.select(Some(bounded_cursor));

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(theme.highlight_bg)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(list, tree_chunks[1], &mut list_state);
}

fn count_file_stats(file: &FileDiff) -> (u32, u32) {
    let mut adds = 0u32;
    let mut dels = 0u32;
    for hunk in &file.hunks {
        for line in &hunk.lines {
            match line {
                DiffLine::Added(_) => adds += 1,
                DiffLine::Removed(_) => dels += 1,
                DiffLine::Context(_) => {}
            }
        }
    }
    (adds, dels)
}

fn render_diff_pane(
    frame: &mut Frame,
    diff: &Diff,
    focused_file: usize,
    file_stats: &[(u32, u32)],
    threads: &[ReviewThread],
    area: Rect,
) {
    let bounded = focused_file.min(diff.files.len().saturating_sub(1));
    let Some(file) = diff.files.get(bounded) else {
        return;
    };
    let (adds, dels) = file_stats.get(bounded).copied().unwrap_or((0, 0));

    let theme = theme::current();

    // Outer border around the diff pane, with an inner header band showing
    // the file path on the left and the +A -D stats on the right.
    let pane_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.divider));
    let pane_inner = pane_block.inner(area);
    frame.render_widget(pane_block, area);

    let pane_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // header row + bottom divider
            Constraint::Min(0),    // diff body
        ])
        .split(pane_inner);

    render_pane_header(frame, &file.path, adds, dels, pane_chunks[0]);

    // Diff body fills the full `pane_inner` width so the row-background tint
    // on +/- lines flows to the box's left/right borders. Each line's own
    // padding handles the 2-col left/right gutter for the content itself.
    let body_area = pane_chunks[1];
    let lines = file_to_lines(file, threads, body_area.width);
    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, body_area);
}

fn render_pane_header(frame: &mut Frame, path: &str, adds: u32, dels: u32, area: Rect) {
    let theme = theme::current();

    let header_block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(theme.divider));
    let header_inner = header_block.inner(area);
    frame.render_widget(header_block, area);

    let plus = format!("+{adds}");
    let minus = format!("-{dels}");
    let stats_visible = plus.chars().count() + 1 + minus.chars().count();

    let header_w = header_inner.width as usize;
    // 2-col padding on each side inside the header band.
    let inner_w = header_w.saturating_sub(4);
    let max_path = inner_w.saturating_sub(stats_visible + 2);
    let displayed_path = truncate_path_left(path, max_path);
    let path_visible = displayed_path.chars().count();
    let gap = inner_w
        .saturating_sub(path_visible + stats_visible)
        .max(1);

    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled(
            displayed_path,
            Style::default()
                .fg(theme.fg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(gap)),
        Span::styled(plus, Style::default().fg(theme.diff_added)),
        Span::raw(" "),
        Span::styled(minus, Style::default().fg(theme.diff_removed)),
    ]);
    frame.render_widget(Paragraph::new(line), header_inner);
}

/// Truncate from the left if the path exceeds `max`, keeping the filename
/// visible and prefixing "…/" so the reader sees the end of the path.
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

/// 2-col gutter on each side of the diff body — the content (prefix +
/// text) sits inside this gutter, but the background tint extends all the
/// way across the row to the box's borders.
const DIFF_GUTTER: &str = "  ";
const DIFF_GUTTER_COLS: u16 = 2;

/// Build a single diff line with full-row background fill via the shared
/// `diff_bg_row` helper. Context lines have no bg tint, only the gutter
/// for alignment with `+`/`-` rows above and below.
fn styled_diff_line(diff_line: &DiffLine, width: u16) -> Line<'static> {
    let theme = theme::current();
    let row_w = width as usize;
    match diff_line {
        DiffLine::Added(c) => diff_bg_row(
            DIFF_GUTTER,
            "+",
            c,
            theme.diff_added,
            theme.diff_added_bg,
            theme.fg,
            row_w,
        ),
        DiffLine::Removed(c) => diff_bg_row(
            DIFF_GUTTER,
            "-",
            c,
            theme.diff_removed,
            theme.diff_removed_bg,
            theme.muted,
            row_w,
        ),
        DiffLine::Context(c) => Line::styled(
            format!("{DIFF_GUTTER} {c}"),
            Style::default().fg(theme.diff_context),
        ),
    }
}

fn file_to_lines(file: &FileDiff, threads: &[ReviewThread], width: u16) -> Vec<Line<'static>> {
    let theme = theme::current();
    let mut lines: Vec<Line> = Vec::new();
    // File path lives in the pane header above us — don't repeat it here.

    // Build a lookup of (new file line number) -> threads anchored there.
    let mut comments_at: HashMap<usize, Vec<&ReviewThread>> = HashMap::new();
    for thread in threads.iter().filter(|t| t.path == file.path) {
        if let Some(line) = thread.line {
            comments_at.entry(line).or_default().push(thread);
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
        for diff_line in &hunk.lines {
            lines.push(styled_diff_line(diff_line, width));

            // Removed lines have no new-file line number; comments anchored to a
            // "new" line number only correspond to Added/Context lines.
            let current_line = match diff_line {
                DiffLine::Removed(_) => None,
                _ => Some(new_line_num),
            };
            if let Some(ln) = current_line
                && let Some(threads_here) = comments_at.get(&ln)
            {
                for thread in threads_here {
                    for tline in render_inline_thread(thread, thread_width, now) {
                        let line_style = tline.style;
                        let mut spans: Vec<Span<'static>> =
                            Vec::with_capacity(tline.spans.len() + 1);
                        spans.push(Span::raw(DIFF_GUTTER));
                        spans.extend(tline.spans);
                        lines.push(Line::from(spans).style(line_style));
                    }
                }
            }

            if !matches!(diff_line, DiffLine::Removed(_)) {
                new_line_num += 1;
            }
        }
    }

    lines
}


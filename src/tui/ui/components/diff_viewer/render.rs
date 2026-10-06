use super::{DiffContext, DiffFocus, DiffViewer, ProposalAt, file_tree::FileComments, pane, tree};
use crate::{
    domain::{
        authorship::Authorship,
        comment::CommentThread,
        diff::{DiffLine, FileDiff, LineRef},
        proposal::Side,
    },
    tui::ui::{layout, widgets},
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Rect},
};
use std::collections::HashSet;

pub(super) fn render(
    frame: &mut Frame<'_>,
    ctx: &DiffContext<'_>,
    ui_diff: &mut DiffViewer,
    area: Rect,
) {
    let DiffContext {
        diff: diff_state,
        threads,
        pending,
        proposals,
        reading,
    } = *ctx;
    // Until a pane is drawn below, there is nothing in it to act on.
    ui_diff.pane = super::PaneNav::default();
    let Some(diff) = widgets::loaded_or_placeholder(frame, diff_state, "diff", area) else {
        return;
    };
    if diff.files.is_empty() {
        frame.render_widget(widgets::empty_state("(no diff)"), area);
        return;
    }

    let compact = area.width < 72;
    let [mut tree_area, _, mut pane_area] = layout::split(
        area,
        Direction::Horizontal,
        [
            Constraint::Length((area.width / 4).clamp(24, 36)),
            Constraint::Length(1),
            Constraint::Min(0),
        ],
    );

    let file_stats: Vec<(u32, u32)> = diff.files.iter().map(count_file_stats).collect();
    let comment_counts: Vec<FileComments> = diff
        .files
        .iter()
        .map(|f| file_comment_count(f, threads, proposals, diff.revision.as_ref()))
        .collect();

    let pane_focused = matches!(ui_diff.focus, DiffFocus::Pane);
    if compact {
        tree_area = area;
        pane_area = area;
    }
    if !compact || !pane_focused {
        tree::render(
            frame,
            diff,
            ui_diff,
            &file_stats,
            &comment_counts,
            tree_area,
        );
    }
    let has_files = !super::file_tree::build_visible_rows(
        &diff.files,
        &ui_diff.collapsed,
        &ui_diff.tree_search.query,
    )
    .is_empty();
    if has_files && (!compact || pane_focused) {
        pane::render(
            frame,
            diff,
            ui_diff,
            &file_stats,
            threads,
            pending,
            proposals,
            pane_focused,
            reading,
            pane_area,
        );
    }
}

/// The comments on a file's visible lines, counted by who wrote them. What an
/// agent proposed and the reader has not dealt with counts as an agent's, so that
/// the file list says which files have something to look at.
fn file_comment_count(
    file: &FileDiff,
    threads: &[CommentThread],
    proposals: &[ProposalAt<'_>],
    revision: Option<&crate::domain::diff::DiffRevision>,
) -> FileComments {
    // Only anchored (code) threads count toward a file; general discussion doesn't.
    let on_file = || {
        threads
            .iter()
            .filter(|t| t.matches_revision(revision))
            .filter_map(|t| t.anchor.as_ref().map(|a| (t, a)))
            .filter(|(_, a)| a.path == file.path)
    };
    let proposed_here = || proposals.iter().filter(|p| p.proposal.path() == file.path);
    if on_file().next().is_none() && proposed_here().next().is_none() {
        return FileComments::default();
    }
    let mut new_lines: HashSet<usize> = HashSet::new();
    let mut old_lines: HashSet<usize> = HashSet::new();
    for hunk in &file.hunks {
        for (line, new_no, old_no) in hunk.numbered_lines() {
            match line {
                DiffLine::Added(_) => {
                    new_lines.insert(new_no);
                }
                DiffLine::Removed(_) => {
                    old_lines.insert(old_no);
                }
                DiffLine::Context(_) => {
                    new_lines.insert(new_no);
                    old_lines.insert(old_no);
                }
            }
        }
    }
    let mut counts = on_file()
        .filter(|(_, a)| match a.line {
            Some(LineRef::New(line)) => new_lines.contains(&line),
            Some(LineRef::Old(line)) => old_lines.contains(&line),
            None => false,
        })
        .flat_map(|(t, _)| &t.comments)
        .fold(FileComments::default(), |mut counts, comment| {
            match comment.authorship {
                Authorship::Human => counts.people += 1,
                Authorship::Ai => counts.ai += 1,
            }
            counts
        });
    // Only those the pane draws: on a line of the diff.
    counts.ai += proposed_here()
        .filter(|p| match p.proposal.side() {
            Side::New => new_lines.contains(&p.proposal.line()),
            Side::Old => old_lines.contains(&p.proposal.line()),
        })
        .count();
    counts
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

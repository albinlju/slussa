//! One numbered row of a diff, as the code pane and a comment's excerpt draw it.

use crate::tui::ui::theme;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

#[expect(
    clippy::too_many_arguments,
    reason = "one styled diff row — splitting the args adds no clarity"
)]
pub(in crate::tui::ui) fn numbered_diff_row(
    line_num: Option<u32>,
    num_width: usize,
    prefix: &'static str,
    content: &str,
    prefix_fg: Color,
    bg: Option<Color>,
    text_fg: Color,
    row_w: usize,
) -> Line<'static> {
    let num_str = match line_num {
        Some(n) => format!("{n:>num_width$}"),
        None => " ".repeat(num_width),
    };
    let gutter = format!(" {num_str} ");
    let visible = gutter.len() + prefix.len() + 1 + Span::raw(content).width();
    let pad = row_w.saturating_sub(visible);

    let apply_bg = |style: Style| match bg {
        Some(bg) => style.bg(bg),
        None => style,
    };
    let gutter_style = apply_bg(Style::default().fg(theme::current().muted));
    let prefix_style = apply_bg(Style::default().fg(prefix_fg).add_modifier(Modifier::BOLD));
    let text_style = apply_bg(Style::default().fg(text_fg));

    Line::from(vec![
        Span::styled(gutter, gutter_style),
        Span::styled(prefix, prefix_style),
        Span::styled(format!(" {content}{}", " ".repeat(pad)), text_style),
    ])
}

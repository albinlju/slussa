//! Frames and what stands in for content: the panel, the empty state, the
//! loading line and the scrollbar.

use crate::tui::{
    app::store::LoadState,
    ui::{component::saturating_u16, layout, theme},
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Rect},
    style::Style,
    symbols,
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub(in crate::tui::ui) fn framed_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    focused: bool,
) -> (Rect, Rect) {
    let theme = theme::current();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(symbols::border::Set {
            vertical_left: "╎",
            vertical_right: "╎",
            ..symbols::border::PLAIN
        })
        .title(format!("{title} "))
        .border_style(Style::default().fg(if focused { theme.accent } else { theme.divider }));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [band_area, body_area] = layout::split(
        inner,
        Direction::Vertical,
        [
            Constraint::Length(if area.height < 12 { 1 } else { 2 }),
            Constraint::Min(0),
        ],
    );

    let header_band = Block::default()
        .borders(if area.height < 12 {
            Borders::NONE
        } else {
            Borders::BOTTOM
        })
        .border_style(Style::default().fg(theme.divider));
    let header_inner = header_band.inner(band_area);
    frame.render_widget(header_band, band_area);

    (header_inner, body_area)
}

pub(in crate::tui::ui) fn empty_state(text: &str) -> Paragraph<'static> {
    Paragraph::new(text.to_string())
        .style(Style::default().fg(theme::current().muted))
        .wrap(Wrap { trim: false })
}

pub(in crate::tui::ui) fn loaded_or_placeholder<'a, T>(
    frame: &mut Frame<'_>,
    state: Option<&'a LoadState<T>>,
    noun: &str,
    area: Rect,
) -> Option<&'a T> {
    let theme = theme::current();
    match state {
        Some(LoadState::Loaded(value)) => return Some(value),
        Some(LoadState::Failed(error)) => {
            let message = error.user_message();
            let p = Paragraph::new(format!("Couldn't load {noun}. F: retry\n{message}"))
                .style(Style::default().fg(theme.error))
                .wrap(Wrap { trim: false });
            frame.render_widget(p, area);
        }
        Some(LoadState::Loading) => {
            frame.render_widget(Paragraph::new(loading(&format!("Loading {noun}..."))), area);
        }
        None | Some(LoadState::NotRequested) => {
            frame.render_widget(empty_state(&format!("No {noun} loaded. F: refresh")), area);
        }
    }
    None
}

pub(in crate::tui::ui) fn scrolled_paragraph(
    frame: &mut Frame<'_>,
    lines: Vec<Line<'static>>,
    scroll: &mut u16,
    viewport: &mut u16,
    area: Rect,
) {
    let max_scroll = saturating_u16(lines.len().saturating_sub(area.height as usize));
    *scroll = (*scroll).min(max_scroll);
    *viewport = area.height;

    let content_area = Rect {
        width: area.width.saturating_sub(1),
        ..area
    };
    frame.render_widget(Paragraph::new(lines).scroll((*scroll, 0)), content_area);

    if max_scroll > 0 {
        let bar = scrollbar(*scroll, max_scroll, area.height);
        frame.render_widget(Paragraph::new(bar), layout::scrollbar_area(area));
    }
}

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub(in crate::tui::ui) fn loading(text: &str) -> Line<'static> {
    Line::styled(
        format!("{}  {text}", spinner_frame()),
        Style::default().fg(theme::current().warning),
    )
}

pub(in crate::tui::ui) fn spinner_frame() -> &'static str {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let idx = (now / 100) as usize % SPINNER_FRAMES.len();
    SPINNER_FRAMES.get(idx).copied().unwrap_or(" ")
}

pub(in crate::tui::ui) fn scrollbar(
    scroll: u16,
    max_scroll: u16,
    height: u16,
) -> Vec<Line<'static>> {
    if max_scroll == 0 || height == 0 {
        return Vec::new();
    }
    let theme = theme::current();
    let track_len = height as usize;
    let thumb_size = 3usize.min(track_len);
    let max_thumb_top = track_len.saturating_sub(1);
    let thumb_top = (scroll as usize * max_thumb_top) / max_scroll as usize;

    (0..track_len)
        .map(|y| {
            if y >= thumb_top && y < thumb_top + thumb_size {
                Line::styled("█", Style::default().fg(theme.accent))
            } else {
                Line::styled("│", Style::default().fg(theme.muted))
            }
        })
        .collect()
}

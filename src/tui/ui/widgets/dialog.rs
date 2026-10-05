//! Shared dialog geometry and keyboard hints; interaction stays in components.
use crate::tui::ui::{component::saturating_u16, theme};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph},
};

pub fn frame(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    size: (u16, u16),
    hints: &[(&str, &str)],
) -> Rect {
    let width = size.0.min(area.width);
    let theme = theme::current();
    let mut lines = Vec::new();
    let mut line = Line::default();
    let available = width.saturating_sub(4) as usize;
    for &(key, description) in hints {
        let used = line.width();
        if used > 0 && used + key.len() + description.len() + 4 > available {
            lines.push(std::mem::take(&mut line));
        }
        if !line.spans.is_empty() {
            line.spans
                .push(Span::styled(" · ", Style::default().fg(theme.muted)));
        }
        line.spans
            .push(Span::styled(key.to_string(), Style::default().fg(theme.fg)));
        line.spans.push(Span::styled(
            format!(" {description}"),
            Style::default().fg(theme.muted),
        ));
    }
    lines.push(line);
    let footer_height = saturating_u16(lines.len()).saturating_add(1);
    let height = size.1.saturating_add(footer_height + 2).min(area.height);
    let popup = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(format!(" {title} "))
        .border_style(Style::default().fg(theme.accent));
    let inner = block.inner(popup);
    frame.render_widget(Clear, popup);
    frame.render_widget(block, popup);
    let body_height = inner.height.saturating_sub(footer_height);
    let footer = Rect::new(
        inner.x,
        inner.y + body_height,
        inner.width,
        footer_height.min(inner.height),
    );
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(theme.divider))
                .padding(Padding::horizontal(1)),
        ),
        footer,
    );
    Rect::new(
        inner.x + u16::from(inner.width > 0),
        inner.y,
        inner.width.saturating_sub(2),
        body_height,
    )
}

/// `lines` without the empty ones, and where the `selected` one is then.
fn without_blank_lines(lines: Vec<Line<'static>>, selected: usize) -> (Vec<Line<'static>>, usize) {
    let blank_before = lines
        .iter()
        .take(selected)
        .filter(|line| line.width() == 0)
        .count();
    let kept = lines.into_iter().filter(|line| line.width() > 0).collect();
    (kept, selected.saturating_sub(blank_before))
}

pub fn choices(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    lines: Vec<Line<'static>>,
    selected: usize,
) {
    choices_with_hints(
        frame,
        area,
        title,
        lines,
        selected,
        &[("j/k", "move"), ("Enter", "select"), ("Esc", "cancel")],
    );
}

/// How narrow a choice dialog may be, how narrow a roomier one may be, and how
/// wide either may grow.
const NARROW: u16 = 44;
const ROOMY: u16 = 60;
const WIDEST: u16 = 72;
const _: () = assert!(NARROW <= ROOMY && ROOMY <= WIDEST);

/// How much width a choice dialog is given at least.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Room {
    Narrow,
    Roomy,
}

impl Room {
    const fn min_width(self) -> u16 {
        match self {
            Self::Narrow => NARROW,
            Self::Roomy => ROOMY,
        }
    }
}

pub fn choices_with_hints(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    lines: Vec<Line<'static>>,
    selected: usize,
    hints: &[(&str, &str)],
) {
    choices_with_room(frame, area, title, lines, selected, hints, Room::Narrow);
}

pub fn choices_with_room(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    lines: Vec<Line<'static>>,
    selected: usize,
    hints: &[(&str, &str)],
    room: Room,
) {
    let width = saturating_u16(lines.iter().map(Line::width).max().unwrap_or(0))
        .saturating_add(4)
        .clamp(room.min_width(), WIDEST);
    let body = self::frame(
        frame,
        area,
        title,
        (width, saturating_u16(lines.len())),
        hints,
    );
    // Blank lines are air: where the dialog does not fit, they go before a
    // choice does.
    let (lines, selected) = if lines.len() > body.height as usize {
        without_blank_lines(lines, selected)
    } else {
        (lines, selected)
    };
    let scroll = selected
        .saturating_add(1)
        .saturating_sub(body.height as usize);
    let lines = lines
        .into_iter()
        .map(|line| Line::from(super::truncate_to_width(line.spans, body.width as usize)))
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(lines).scroll((saturating_u16(scroll), 0)),
        body,
    );
}

use crate::{
    domain::capabilities::{Capabilities, Feature},
    tui::theme,
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph},
};

const HELP_KEYS: &[(&str, &str)] = &[
    ("j/k", "move up/down"),
    ("^d/^u", "half-page"),
    ("h/l", "tab / pane / fold"),
    ("1-5", "select tab"),
    ("enter", "open / view"),
    ("space", "toggle fold"),
    ("/", "search"),
    ("n/N", "next/prev match"),
    ("[ ]", "prev/next tab/commit"),
    ("esc", "back"),
    ("a", "quick verdict"),
    ("v", "start/finish review"),
    ("V", "discard review"),
    ("m", "merge"),
    ("x", "decline"),
    ("c", "comment"),
    ("r", "reply"),
    ("^j/^k", "step comment"),
    ("e", "edit own"),
    ("d", "delete own"),
    ("R", "resolve thread"),
    ("F", "refresh"),
    ("?", "toggle help"),
    ("q", "quit"),
];

pub(in crate::tui::screens::pr_detail) fn render(
    frame: &mut Frame,
    area: Rect,
    caps: &Capabilities,
) {
    let keys: Vec<_> = HELP_KEYS
        .iter()
        .copied()
        .filter(|(key, _)| match *key {
            "a" | "v" | "V" => caps.reviews(),
            "m" => !caps.merge_strategies.is_empty(),
            "x" => caps.supports(Feature::ClosePr),
            "c" => {
                caps.supports(Feature::PrComments)
                    || caps.supports(Feature::InlineComments)
                    || caps.supports(Feature::Replies)
            }
            "r" => caps.supports(Feature::Replies),
            "e" => caps.supports(Feature::EditComments),
            "d" => caps.supports(Feature::DeleteComments),
            "R" => caps.supports(Feature::ResolveThreads),
            _ => true,
        })
        .map(|(key, desc)| {
            (
                if key == "1-5" && !caps.supports(Feature::Builds) {
                    "1-4"
                } else {
                    key
                },
                desc,
            )
        })
        .collect();
    let theme = theme::current();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Help ")
        .border_style(Style::default().fg(theme.accent))
        .padding(Padding::symmetric(1, 1));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let key_style = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(theme.muted);
    let key_w = keys.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    let desc_w = keys.iter().map(|(_, d)| d.len()).max().unwrap_or(0);
    let rows = (inner.height as usize).clamp(1, keys.len());
    let cols = keys.len().div_ceil(rows);

    let lines: Vec<Line<'static>> = (0..rows)
        .map(|r| {
            let mut spans: Vec<Span<'static>> = Vec::new();
            for c in 0..cols {
                if let Some((k, d)) = keys.get(c * rows + r) {
                    spans.push(Span::styled(format!("{k:<key_w$}  "), key_style));
                    spans.push(Span::styled(format!("{d:<w$}", w = desc_w + 3), desc_style));
                }
            }
            Line::from(spans)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

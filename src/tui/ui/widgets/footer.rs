//! The footer line: the hints, the search prompt.

use super::text::{justify_between, truncate_to_width};
use crate::tui::ui::theme;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

/// A footer action hint. `enabled == false` renders it dimmed (key not accented)
/// — the "disabled with affordance" pattern: the action stays visible with its
/// reason instead of being hidden.
pub(in crate::tui::ui) struct Hint {
    text: String,
    enabled: bool,
}

impl Hint {
    pub(in crate::tui::ui) fn on(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            enabled: true,
        }
    }

    pub(in crate::tui::ui) fn off(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            enabled: false,
        }
    }
}

/// Build a row of all-enabled hints from a `"a: x  b: y"` string.
pub(in crate::tui::ui) fn hints_on(s: &str) -> Vec<Hint> {
    s.split("  ").map(Hint::on).collect()
}

pub(in crate::tui::ui) fn footer(width: u16, hints: &[Hint], refreshing: bool) -> Line<'static> {
    let theme = theme::current();
    let muted = Style::default().fg(theme.muted);
    let key = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);

    let width = width as usize;
    let right = if refreshing && width >= 30 {
        "refreshing…  ?: help "
    } else {
        "?: help "
    };
    let right = truncate_to_width(vec![Span::styled(right, muted)], width);
    let right_width: usize = right.iter().map(Span::width).sum();
    let budget = width.saturating_sub(right_width + 1);
    let mut left = Vec::new();
    let mut used = 0;
    for hint in hints {
        let hint_width = Span::raw(&hint.text).width() + 2;
        if used + hint_width > budget {
            continue;
        }
        left.push(Span::raw("  "));
        let key_style = if hint.enabled { key } else { muted };
        match hint.text.split_once(": ") {
            Some((keys, desc)) => {
                left.push(Span::styled(keys.to_string(), key_style));
                left.push(Span::styled(format!(": {desc}"), muted));
            }
            None => left.push(Span::styled(hint.text.clone(), muted)),
        }
        used += hint_width;
    }
    left.push(Span::raw(
        " ".repeat(width.saturating_sub(used + right_width)),
    ));
    left.extend(right);
    Line::from(left)
}

pub(in crate::tui::ui) fn search_input_spans(query: &str) -> Vec<Span<'static>> {
    let theme = theme::current();
    vec![
        Span::styled(format!("  Search: {query}"), Style::default().fg(theme.fg)),
        Span::styled("█", Style::default().fg(theme.accent)),
    ]
}

pub(in crate::tui::ui) fn search_prompt(query: &str, count: usize, width: u16) -> Line<'static> {
    let theme = theme::current();
    let right = vec![Span::styled(
        format!("{count} match  "),
        Style::default().fg(theme.muted),
    )];
    Line::from(justify_between(
        search_input_spans(query),
        right,
        width as usize,
    ))
}

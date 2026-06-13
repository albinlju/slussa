use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::tui::theme;

#[derive(Clone, Copy)]
pub(crate) enum Width {
    Fixed(u16),
    Flex(u16),
}

pub(crate) struct Column {
    pub title: &'static str,
    pub width: Width,
}

pub(crate) type Cell = Vec<Span<'static>>;

pub(crate) struct Table<'a> {
    cols: &'a [Column],
    widths: Vec<usize>,
}

impl<'a> Table<'a> {
    pub fn new(cols: &'a [Column], total: u16) -> Self {
        let total = total as usize;
        let fixed: usize = cols
            .iter()
            .filter_map(|c| match c.width {
                Width::Fixed(w) => Some(w as usize),
                Width::Flex(_) => None,
            })
            .sum();
        let flex_total: usize = cols
            .iter()
            .filter_map(|c| match c.width {
                Width::Flex(f) => Some(f as usize),
                Width::Fixed(_) => None,
            })
            .sum();
        let remaining = total.saturating_sub(fixed);
        let widths = cols
            .iter()
            .map(|c| match c.width {
                Width::Fixed(w) => w as usize,
                Width::Flex(f) if flex_total > 0 => remaining * f as usize / flex_total,
                Width::Flex(_) => 0,
            })
            .collect();
        Self { cols, widths }
    }

    pub fn header(&self) -> Line<'static> {
        let style = Style::default()
            .fg(theme::current().fg)
            .add_modifier(Modifier::BOLD);
        let cells: Vec<Cell> = self
            .cols
            .iter()
            .map(|c| vec![Span::styled(c.title, style)])
            .collect();
        self.row(&cells)
    }

    pub fn row(&self, cells: &[Cell]) -> Line<'static> {
        let mut spans: Vec<Span<'static>> = Vec::new();
        for (i, &width) in self.widths.iter().enumerate() {
            let cell = cells.get(i).map_or(&[][..], Vec::as_slice);
            spans.extend(fit_cell(cell, width));
        }
        Line::from(spans)
    }
}

/// Truncate (tail `…`) and left-pad a cell's spans to exactly `width`.
fn fit_cell(cell: &[Span<'static>], width: usize) -> Vec<Span<'static>> {
    let total: usize = cell.iter().map(Span::width).sum();
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut used = 0;

    if total <= width {
        out.extend(cell.iter().cloned());
        used = total;
    } else {
        let budget = width.saturating_sub(1);
        for span in cell {
            let w = span.width();
            if used + w <= budget {
                out.push(span.clone());
                used += w;
            } else {
                let kept = take_to_width(&span.content, budget - used);
                used += Span::raw(kept.as_str()).width();
                if !kept.is_empty() {
                    out.push(Span::styled(kept, span.style));
                }
                break;
            }
        }
        out.push(Span::raw("…"));
        used += 1;
    }

    let pad = width.saturating_sub(used);
    if pad > 0 {
        out.push(Span::raw(" ".repeat(pad)));
    }
    out
}

fn take_to_width(s: &str, max: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let w = Span::raw(c.to_string()).width();
        if used + w > max {
            break;
        }
        used += w;
        out.push(c);
    }
    out
}

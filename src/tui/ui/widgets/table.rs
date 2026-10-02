use crate::tui::ui::theme;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

#[derive(Clone, Copy)]
pub(crate) enum Width {
    Fixed(u16),
    Flex(u16),
}

#[derive(Clone, Copy)]
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
    pub fn new(cols: &'a [Column], total_width: u16) -> Self {
        let total = total_width as usize;
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
            .fg(theme::current().muted)
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

fn fit_cell(cell: &[Span<'static>], width: usize) -> Vec<Span<'static>> {
    let mut out = super::truncate_to_width(cell.to_vec(), width);
    let used: usize = out.iter().map(Span::width).sum();
    let pad = width.saturating_sub(used);
    if pad > 0 {
        out.push(Span::raw(" ".repeat(pad)));
    }
    out
}

use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub(super) fn split<const N: usize>(
    area: Rect,
    direction: Direction,
    constraints: [Constraint; N],
) -> [Rect; N] {
    let chunks = Layout::default()
        .direction(direction)
        .constraints(constraints)
        .split(area);
    std::array::from_fn(|i| chunks[i])
}

pub(super) const fn scrollbar_area(area: Rect) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(1),
        y: area.y,
        width: 1,
        height: area.height,
    }
}

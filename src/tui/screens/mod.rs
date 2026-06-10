pub mod pr_detail;
pub mod pr_list;

/// The Ctrl+D/U and PageDown/Up step: half the viewport, at least one line.
pub(in crate::tui) fn half_page(viewport: u16) -> i16 {
    (viewport / 2).max(1) as i16
}

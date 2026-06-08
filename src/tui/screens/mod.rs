pub mod pr_detail;
pub mod pr_list;

/// Half the viewport height, at least one line — the Ctrl+D/U & PageDown/Up
/// scroll/jump step, shared by every scrollable view.
pub(in crate::tui) fn half_page(viewport: u16) -> i16 {
    (viewport / 2).max(1) as i16
}

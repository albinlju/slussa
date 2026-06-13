pub mod pr_detail;
pub mod pr_list;

pub(in crate::tui) fn half_page(viewport: u16) -> i16 {
    (viewport / 2).max(1) as i16
}

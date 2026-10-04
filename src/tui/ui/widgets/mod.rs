pub mod comment;
pub mod dialog;
mod diff_row;
mod footer;
pub mod markdown;
mod panel;
pub mod table;
mod text;

pub(super) use diff_row::numbered_diff_row;
pub(super) use footer::{
    Hint, footer, hints_on, search_input_spans, search_prompt, search_prompt_with_hint,
};
pub(super) use panel::{
    empty_state, framed_panel, loaded_or_placeholder, loading, scrollbar, scrolled_paragraph,
    spinner_frame,
};
pub(super) use text::{fitted_row, highlight_query, justify_between, truncate_to_width, wrap_text};

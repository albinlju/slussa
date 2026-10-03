//! Comments and threads: the boxes they are drawn in, their author line, folds
//! and suggestions.

mod code;
pub mod fold;
mod frame;
pub mod meta;
mod render;

pub(in crate::tui::ui) use render::{
    InlineThread, comment_box, comment_thread_box, render_inline_thread,
};

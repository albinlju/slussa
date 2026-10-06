mod bindings;
mod build_status;
pub mod dialogs;
mod footer;
mod header;
mod interactions;
pub mod keys;
mod opening;
mod render;
mod screen;
mod since;
pub mod tabs;
pub mod view;

pub use opening::render as render_opening;
pub use screen::{Overlay, PrDetailScreen, Surface};
pub use view::{DetailContext, DetailView};

#[cfg(test)]
mod tests;

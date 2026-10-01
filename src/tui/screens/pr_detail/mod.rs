mod build_status;
pub mod dialogs;
mod footer;
mod header;
mod interactions;
pub mod keys;
mod render;
mod screen;
pub mod tabs;
pub mod view;

pub use screen::{Overlay, PrDetailScreen, Surface};
pub use view::{DetailContext, DetailView};

#[cfg(test)]
mod tests;

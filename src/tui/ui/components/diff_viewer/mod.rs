pub(crate) mod file_tree;
mod inline;
mod keys;
mod nav;
mod pane;
mod render;
mod threads;
mod tree;
mod viewer;

pub use viewer::{DiffContext, DiffFocus, DiffViewer, FocusedNav, NavTarget, PaneNav, ProposalAt};

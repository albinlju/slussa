// A match on one of our own enums names every variant, so that adding one is a
// compile error wherever it has to be handled. Tests assert by a catch-all.
#![cfg_attr(not(test), warn(clippy::wildcard_enum_match_arm))]

pub mod activity;
pub mod attention;
pub mod authorship;
pub mod ci;
pub mod comment;
pub mod commit;
pub mod diff;
pub mod event;
pub mod pr;
pub mod review;
pub mod seen;
pub mod user;

pub mod capabilities;

//! Semantic color theme. UI code should reference these named fields instead
//! of hardcoded `Color::*` so the entire palette can be swapped in one place.

use ratatui::style::Color;

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    /// Default foreground for body text.
    pub fg: Color,
    /// Default background — also used as dark text on saturated accent
    /// backgrounds (status badges) where light fg would have poor contrast.
    pub bg: Color,
    /// Dimmed text — timestamps, separators, hints.
    pub muted: Color,
    /// Borders, structural chrome.
    pub border: Color,
    /// Background for highlighted/selected rows.
    pub highlight_bg: Color,
    /// Primary accent — active tab, scroll thumb, "more" indicators.
    pub accent: Color,
    /// Positive / success state.
    pub success: Color,
    /// Warning / pending / spinner.
    pub warning: Color,
    /// Error / failed state.
    pub error: Color,
    /// Informational — author handles, hunk headers, inline-comment bar.
    pub info: Color,
    /// Links, branches, dir names in tree.
    pub link: Color,
    /// Purple — file paths in diff headers.
    pub purple: Color,
    /// Diff added line color.
    pub diff_added: Color,
    /// Diff removed line color.
    pub diff_removed: Color,
    /// Diff context fallback (when not specifically Added/Removed).
    pub diff_context: Color,
    /// PR status colors.
    pub status_open: Color,
    pub status_draft: Color,
    pub status_merged: Color,
    pub status_declined: Color,
}

/// Gruvbox dark palette. See https://github.com/morhetz/gruvbox.
pub const GRUVBOX: Theme = Theme {
    fg: Color::Rgb(0xeb, 0xdb, 0xb2),            // #ebdbb2 fg1
    bg: Color::Rgb(0x28, 0x28, 0x28),            // #282828 bg0
    muted: Color::Rgb(0x92, 0x83, 0x74),         // #928374 gray
    border: Color::Rgb(0x66, 0x5c, 0x54),        // #665c54 bg3
    highlight_bg: Color::Rgb(0x50, 0x49, 0x45),  // #504945 bg2
    accent: Color::Rgb(0xfa, 0xbd, 0x2f),        // #fabd2f yellow
    success: Color::Rgb(0xb8, 0xbb, 0x26),       // #b8bb26 green
    warning: Color::Rgb(0xfa, 0xbd, 0x2f),       // #fabd2f yellow
    error: Color::Rgb(0xfb, 0x49, 0x34),         // #fb4934 red
    info: Color::Rgb(0x8e, 0xc0, 0x7c),          // #8ec07c aqua
    link: Color::Rgb(0x83, 0xa5, 0x98),          // #83a598 blue
    purple: Color::Rgb(0xd3, 0x86, 0x9b),        // #d3869b purple
    diff_added: Color::Rgb(0xb8, 0xbb, 0x26),    // green
    diff_removed: Color::Rgb(0xfb, 0x49, 0x34),  // red
    diff_context: Color::Rgb(0xa8, 0x99, 0x84),  // #a89984 fg3 (lighter muted)
    status_open: Color::Rgb(0xb8, 0xbb, 0x26),
    status_draft: Color::Rgb(0x92, 0x83, 0x74),
    status_merged: Color::Rgb(0xd3, 0x86, 0x9b),
    status_declined: Color::Rgb(0xfb, 0x49, 0x34),
};

pub fn current() -> &'static Theme {
    &GRUVBOX
}

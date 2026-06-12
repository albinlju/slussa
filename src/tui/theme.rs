use ratatui::style::Color;

use crate::domain::pr::PrStatus;

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
    /// Dimmer divider line — separators inside a bordered frame so the inner
    /// rule reads as secondary chrome compared to the outer border.
    pub divider: Color,
    /// Background for highlighted/selected rows.
    pub highlight_bg: Color,
    /// Primary accent — active tab, scroll thumb, "more" indicators.
    pub accent: Color,
    /// Dark accent-tinted background — "yours" chips (own reactions), where
    /// a full accent fill would drown yellow emojis.
    pub accent_bg: Color,
    /// Suggested-change blocks — header diamond and box border.
    pub suggestion: Color,
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
    /// Gruvbox bright orange — block titles / labels in borders.
    pub orange: Color,
    /// Diff added line color — strong green used for the "+" marker.
    pub diff_added: Color,
    /// Diff removed line color — strong red used for the "-" marker.
    pub diff_removed: Color,
    /// Diff context fallback (when not specifically Added/Removed).
    pub diff_context: Color,
    /// Subtle background fill for added lines.
    pub diff_added_bg: Color,
    /// Subtle background fill for removed lines.
    pub diff_removed_bg: Color,
    /// PR status colors.
    pub status_open: Color,
    pub status_draft: Color,
    pub status_merged: Color,
    pub status_declined: Color,
}

pub const GRUVBOX: Theme = Theme {
    fg: Color::Rgb(0xfb, 0xf1, 0xc7),
    bg: Color::Rgb(0x28, 0x28, 0x28),
    muted: Color::Rgb(0x92, 0x83, 0x74),
    border: Color::Rgb(0xfa, 0xbd, 0x2f),
    divider: Color::Rgb(0x92, 0x83, 0x74),
    highlight_bg: Color::Rgb(0x50, 0x49, 0x45),
    accent: Color::Rgb(0xfa, 0xbd, 0x2f),
    accent_bg: Color::Rgb(0x42, 0x39, 0x14),
    suggestion: Color::Rgb(0xd3, 0x86, 0x9b),
    success: Color::Rgb(0xb8, 0xbb, 0x26),
    warning: Color::Rgb(0xfa, 0xbd, 0x2f),
    error: Color::Rgb(0xfb, 0x49, 0x34),
    info: Color::Rgb(0x8e, 0xc0, 0x7c),
    link: Color::Rgb(0x83, 0xa5, 0x98),
    orange: Color::Rgb(0xfe, 0x80, 0x19),
    diff_added: Color::Rgb(0xb8, 0xbb, 0x26),
    diff_removed: Color::Rgb(0xfb, 0x49, 0x34),
    diff_context: Color::Rgb(0xa8, 0x99, 0x84),
    diff_added_bg: Color::Rgb(0x32, 0x40, 0x1e),
    diff_removed_bg: Color::Rgb(0x40, 0x22, 0x1e),
    status_open: Color::Rgb(0xb8, 0xbb, 0x26),
    status_draft: Color::Rgb(0x92, 0x83, 0x74),
    status_merged: Color::Rgb(0xd3, 0x86, 0x9b),
    status_declined: Color::Rgb(0xfb, 0x49, 0x34),
};

impl Theme {
    pub fn status_color(&self, status: &PrStatus) -> Color {
        match status {
            PrStatus::Open => self.status_open,
            PrStatus::Draft => self.status_draft,
            PrStatus::Merged => self.status_merged,
            PrStatus::Declined => self.status_declined,
        }
    }
}

pub fn current() -> &'static Theme {
    &GRUVBOX
}

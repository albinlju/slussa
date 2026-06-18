use std::sync::OnceLock;

use ratatui::style::Color;

use crate::domain::pr::PrStatus;

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub fg: Color,
    pub bg: Color,
    pub muted: Color,
    pub border: Color,
    pub divider: Color,
    pub highlight_bg: Color,
    pub accent: Color,
    pub accent_bg: Color,
    pub suggestion: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,
    pub link: Color,
    pub orange: Color,
    pub diff_added: Color,
    pub diff_removed: Color,
    pub diff_context: Color,
    pub status_open: Color,
    pub status_draft: Color,
    pub status_merged: Color,
    pub status_declined: Color,
}

pub const GRUVBOX: Theme = Theme {
    fg: Color::Rgb(0xfb, 0xf1, 0xc7),
    bg: Color::Rgb(0x28, 0x28, 0x28),
    muted: Color::Rgb(0xa8, 0x99, 0x84),
    border: Color::Rgb(0xfa, 0xc8, 0x1c),
    divider: Color::Rgb(0x46, 0x40, 0x3d),
    highlight_bg: Color::Rgb(0x50, 0x49, 0x45),
    accent: Color::Rgb(0xfa, 0xc8, 0x1c),
    accent_bg: Color::Rgb(0x42, 0x39, 0x14),
    suggestion: Color::Rgb(0xd3, 0x86, 0x9b),
    success: Color::Rgb(0xb8, 0xbb, 0x26),
    warning: Color::Rgb(0xfa, 0xbd, 0x2f),
    error: Color::Rgb(0xfb, 0x49, 0x34),
    info: Color::Rgb(0x9f, 0xcb, 0x90),
    link: Color::Rgb(0x83, 0xa5, 0x98),
    orange: Color::Rgb(0xfe, 0x80, 0x19),
    diff_added: Color::Rgb(0xb8, 0xbb, 0x26),
    diff_removed: Color::Rgb(0xfb, 0x49, 0x34),
    diff_context: Color::Rgb(0xa8, 0x99, 0x84),
    status_open: Color::Rgb(0xb8, 0xbb, 0x26),
    status_draft: Color::Rgb(0x92, 0x83, 0x74),
    status_merged: Color::Rgb(0xd3, 0x86, 0x9b),
    status_declined: Color::Rgb(0xfb, 0x49, 0x34),
};

// Named ANSI colors so the palette inherits the terminal's own theme and degrades
// on low-color terminals. fg/bg stay Reset (terminal default). The Indexed(_) ones
// are the spots ANSI can't express adaptively (subtle backgrounds, orange).
pub const TERMINAL: Theme = Theme {
    fg: Color::Reset,
    bg: Color::Reset,
    muted: Color::Gray,
    border: Color::Yellow,
    divider: Color::Indexed(239),
    highlight_bg: Color::Indexed(238),
    accent: Color::Yellow,
    accent_bg: Color::Indexed(238),
    suggestion: Color::Magenta,
    success: Color::Green,
    warning: Color::Yellow,
    error: Color::Red,
    info: Color::Cyan,
    link: Color::Blue,
    orange: Color::Indexed(208),
    diff_added: Color::Green,
    diff_removed: Color::Red,
    diff_context: Color::DarkGray,
    status_open: Color::Green,
    status_draft: Color::DarkGray,
    status_merged: Color::Magenta,
    status_declined: Color::Red,
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
    static SELECTED: OnceLock<&'static Theme> = OnceLock::new();
    SELECTED.get_or_init(|| match std::env::var("TUIPR_THEME").as_deref() {
        Ok("gruvbox") => &GRUVBOX,
        _ => &TERMINAL,
    })
}

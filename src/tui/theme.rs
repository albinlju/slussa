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
    pub reaction_mine: Color,
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
    reaction_mine: Color::Rgb(0xa8, 0x99, 0x84),
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

pub const CATPPUCCIN: Theme = Theme {
    fg: Color::Rgb(0xcd, 0xd6, 0xf4),
    bg: Color::Rgb(0x1e, 0x1e, 0x2e),
    muted: Color::Rgb(0xa6, 0xad, 0xc8),
    border: Color::Rgb(0x89, 0xb4, 0xfa),
    divider: Color::Rgb(0x4c, 0x4e, 0x63),
    highlight_bg: Color::Rgb(0x45, 0x47, 0x5a),
    accent: Color::Rgb(0x89, 0xb4, 0xfa),
    reaction_mine: Color::Rgb(0x89, 0xb4, 0xfa),
    suggestion: Color::Rgb(0xf5, 0xc2, 0xe7),
    success: Color::Rgb(0xa6, 0xe3, 0xa1),
    warning: Color::Rgb(0xf9, 0xe2, 0xaf),
    error: Color::Rgb(0xf3, 0x8b, 0xa8),
    info: Color::Rgb(0x94, 0xe2, 0xd5),
    link: Color::Rgb(0x74, 0xc7, 0xec),
    orange: Color::Rgb(0xfa, 0xb3, 0x87),
    diff_added: Color::Rgb(0xa6, 0xe3, 0xa1),
    diff_removed: Color::Rgb(0xf3, 0x8b, 0xa8),
    diff_context: Color::Rgb(0x6c, 0x70, 0x86),
    status_open: Color::Rgb(0xa6, 0xe3, 0xa1),
    status_draft: Color::Rgb(0x6c, 0x70, 0x86),
    status_merged: Color::Rgb(0xcb, 0xa6, 0xf7),
    status_declined: Color::Rgb(0xf3, 0x8b, 0xa8),
};

pub const TERMINAL: Theme = Theme {
    fg: Color::Reset,
    bg: Color::Reset,
    muted: Color::Gray,
    border: Color::Reset,
    divider: Color::Indexed(239),
    highlight_bg: Color::Indexed(238),
    accent: Color::Cyan,
    reaction_mine: Color::Gray,
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
    SELECTED.get_or_init(|| {
        let name = std::env::var("TUIPR_THEME")
            .ok()
            .or_else(|| crate::config::load().theme);
        match name.as_deref() {
            Some("gruvbox") => &GRUVBOX,
            Some("catppuccin") => &CATPPUCCIN,
            _ => &TERMINAL,
        }
    })
}

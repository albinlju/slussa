use std::sync::OnceLock;

use ratatui::style::Color;

use crate::domain::pr::PrStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub fg: Color,
    pub bg: Color,
    pub muted: Color,
    pub border: Color,
    pub divider: Color,
    pub highlight_bg: Color,
    pub accent: Color,
    /// Non-interactive emphasis: headings, labels and commit identifiers.
    pub decorative: Color,
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
    decorative: Color::Rgb(0xfa, 0xc8, 0x1c),
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
    decorative: Color::Rgb(0x89, 0xb4, 0xfa),
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

/// Slate: neutral text, blue focus and restrained semantic colors on a dark
/// terminal. `bg` is contrast ink for badges/search, not a canvas override.
pub const SLATE: Theme = Theme {
    fg: Color::Rgb(0xde, 0xe3, 0xea),
    bg: Color::Rgb(0x18, 0x1c, 0x22),
    muted: Color::Rgb(0x9a, 0xa6, 0xb5),
    border: Color::Rgb(0x65, 0x75, 0x8a),
    divider: Color::Rgb(0x46, 0x52, 0x61),
    highlight_bg: Color::Rgb(0x29, 0x38, 0x49),
    accent: Color::Rgb(0x87, 0xbf, 0xff),
    decorative: Color::Rgb(0x87, 0xbf, 0xff),
    reaction_mine: Color::Rgb(0x87, 0xbf, 0xff),
    suggestion: Color::Rgb(0xb3, 0xa2, 0xd8),
    success: Color::Rgb(0x8b, 0xc6, 0xa0),
    warning: Color::Rgb(0xde, 0xbc, 0x7c),
    error: Color::Rgb(0xee, 0x97, 0x97),
    info: Color::Rgb(0xb7, 0xc8, 0xda),
    link: Color::Rgb(0x91, 0xbd, 0xe0),
    orange: Color::Rgb(0xe1, 0xab, 0x83),
    diff_added: Color::Rgb(0x8b, 0xc6, 0xa0),
    diff_removed: Color::Rgb(0xee, 0x97, 0x97),
    diff_context: Color::Rgb(0x9a, 0xa6, 0xb5),
    status_open: Color::Rgb(0x8b, 0xc6, 0xa0),
    status_draft: Color::Rgb(0x9a, 0xa6, 0xb5),
    status_merged: Color::Rgb(0xb3, 0xa2, 0xd8),
    status_declined: Color::Rgb(0xee, 0x97, 0x97),
};

/// Graphite reserves blue for interaction; passive information is neutral.
pub const GRAPHITE: Theme = Theme {
    fg: Color::Rgb(0xe6, 0xe6, 0xe3),
    bg: Color::Rgb(0x1c, 0x1d, 0x1f),
    muted: Color::Rgb(0xa3, 0xa4, 0xa6),
    border: Color::Rgb(0x68, 0x6b, 0x70),
    divider: Color::Rgb(0x4a, 0x4c, 0x50),
    highlight_bg: Color::Rgb(0x34, 0x36, 0x38),
    accent: Color::Rgb(0x81, 0xb4, 0xf4),
    decorative: Color::Rgb(0xd3, 0xd4, 0xd5),
    reaction_mine: Color::Rgb(0x81, 0xb4, 0xf4),
    info: Color::Rgb(0xd3, 0xd4, 0xd5),
    link: Color::Rgb(0xc3, 0xc5, 0xc8),
    success: Color::Rgb(0x91, 0xb7, 0x9b),
    warning: Color::Rgb(0xce, 0xb5, 0x81),
    error: Color::Rgb(0xd9, 0x98, 0x98),
    suggestion: Color::Rgb(0xb0, 0xa3, 0xc5),
    orange: Color::Rgb(0xd0, 0xad, 0x91),
    diff_added: Color::Rgb(0x91, 0xb7, 0x9b),
    diff_removed: Color::Rgb(0xd9, 0x98, 0x98),
    diff_context: Color::Rgb(0xa3, 0xa4, 0xa6),
    status_open: Color::Rgb(0x91, 0xb7, 0x9b),
    status_draft: Color::Rgb(0xa3, 0xa4, 0xa6),
    status_merged: Color::Rgb(0xb0, 0xa3, 0xc5),
    status_declined: Color::Rgb(0xd9, 0x98, 0x98),
};

pub const TERMINAL: Theme = Theme {
    fg: Color::Reset,
    bg: Color::Reset,
    muted: Color::Gray,
    border: Color::Reset,
    divider: Color::Indexed(239),
    highlight_bg: Color::Indexed(238),
    accent: Color::Cyan,
    decorative: Color::Cyan,
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
    pub const fn status_color(&self, status: &PrStatus) -> Color {
        match status {
            PrStatus::Open => self.status_open,
            PrStatus::Draft => self.status_draft,
            PrStatus::Merged => self.status_merged,
            PrStatus::Declined => self.status_declined,
        }
    }
}

/// The theme used when none is configured, or when the configured name is unknown.
pub const DEFAULT: &Theme = &GRAPHITE;

/// Look a theme up by its config name.
fn named(name: &str) -> Option<&'static Theme> {
    match name {
        "graphite" => Some(&GRAPHITE),
        "slate" => Some(&SLATE),
        "gruvbox" => Some(&GRUVBOX),
        "catppuccin" => Some(&CATPPUCCIN),
        "terminal" => Some(&TERMINAL),
        _ => None,
    }
}

pub fn current() -> &'static Theme {
    static SELECTED: OnceLock<&'static Theme> = OnceLock::new();
    SELECTED.get_or_init(|| {
        let name = std::env::var("TUIPR_THEME")
            .ok()
            .or_else(|| crate::config::load().theme);
        let Some(name) = name else {
            return DEFAULT;
        };
        named(&name).unwrap_or_else(|| {
            tracing::warn!("unknown theme {name:?}, using graphite");
            DEFAULT
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphite_is_the_default() {
        assert_eq!(*DEFAULT, GRAPHITE);
    }

    #[test]
    fn every_documented_name_resolves_to_its_own_theme() {
        for (name, theme) in [
            ("graphite", &GRAPHITE),
            ("slate", &SLATE),
            ("gruvbox", &GRUVBOX),
            ("catppuccin", &CATPPUCCIN),
            ("terminal", &TERMINAL),
        ] {
            assert_eq!(named(name), Some(theme), "{name}");
        }
    }

    #[test]
    fn unknown_names_are_not_themes() {
        for name in ["auto", "Graphite", "", "dark"] {
            assert!(named(name).is_none(), "{name:?}");
        }
    }
}

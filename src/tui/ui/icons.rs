//! Status glyphs — deliberately plain Unicode (geometric shapes, dingbats,
//! arrows) so slussa needs no Nerd Font. All are single-width and render in any
//! monospace font; meaning is reinforced by colour at each call site.

pub const CHECK_CIRCLE: &str = "✓"; // U+2713  success / approved
pub const TIMES_CIRCLE: &str = "✗"; // U+2717  failure / changes requested
pub const QUESTION_CIRCLE: &str = "?"; //        unknown / help
pub const CIRCLE: &str = "●"; // U+25CF  filled — running / active
pub const CIRCLE_O: &str = "○"; // U+25CB  hollow — commented
pub const BAN: &str = "⊘"; // U+2298  cancelled
pub const CLOCK: &str = "◷"; // U+25F7  pending
pub const ADJUST: &str = "◐"; // U+25D0  partial / unknown
pub const COMMENT: &str = "•"; // U+2022  comment marker
pub const AI: &str = "◆"; // U+25C6  an AI agent's comments
pub const AI_STALE: &str = "◈"; // U+25C8  an AI review of an older head
pub const AI_NONE: &str = "◇"; // U+25C7  no AI review
pub const GIT_COMMIT: &str = "●"; // U+25CF  commit node

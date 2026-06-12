use chrono::{DateTime, Utc};
use ratatui::text::Span;

/// Truncate to `max` display columns, ending with `…` when cut.
pub(super) fn truncate_ellipsis(s: &str, max: usize) -> String {
    if Span::raw(s).width() <= max {
        return s.to_string();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let w = Span::raw(c.to_string()).width();
        if used + w > max.saturating_sub(1) {
            break;
        }
        used += w;
        out.push(c);
    }
    out.push('…');
    out
}

pub(super) fn relative_age(when: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let delta = now - when;
    let days = delta.num_days();
    if days >= 1 {
        format!("{days}d ago")
    } else {
        let hours = delta.num_hours();
        if hours >= 1 {
            format!("{hours}h ago")
        } else {
            "just now".to_string()
        }
    }
}

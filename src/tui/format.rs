use chrono::{DateTime, Utc};

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

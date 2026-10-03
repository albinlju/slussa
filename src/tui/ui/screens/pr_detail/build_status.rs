use ratatui::{
    style::{Color, Style},
    text::Span,
};

use crate::{
    domain::ci::{Build, BuildState},
    tui::ui::{icons, theme, widgets},
};

pub(super) struct BuildStats {
    pub passing: usize,
    pub total: usize,
    pub any_running: bool,
    pub any_failed: bool,
}

impl BuildStats {
    pub(super) fn accent(&self) -> Color {
        OverallState::of(self).color()
    }
}

pub(super) fn build_stats(builds: &[Build]) -> BuildStats {
    BuildStats {
        passing: builds
            .iter()
            .filter(|b| b.state == BuildState::Successful)
            .count(),
        total: builds.len(),
        any_running: builds.iter().any(|b| b.state == BuildState::InProgress),
        any_failed: builds
            .iter()
            .any(|b| matches!(b.state, BuildState::Failed | BuildState::Cancelled)),
    }
}

#[derive(Clone, Copy)]
pub(super) enum OverallState {
    Running,
    Failed,
    AllPassed,
    Partial,
}

impl OverallState {
    pub(super) const fn of(stats: &BuildStats) -> Self {
        if stats.any_running {
            Self::Running
        } else if stats.any_failed {
            Self::Failed
        } else if stats.passing == stats.total {
            Self::AllPassed
        } else {
            Self::Partial
        }
    }

    pub(super) fn color(self) -> Color {
        let theme = theme::current();
        match self {
            Self::Running => theme.warning,
            Self::Failed => theme.error,
            Self::AllPassed => theme.success,
            Self::Partial => theme.muted,
        }
    }

    pub(super) fn glyph(self) -> (String, &'static str) {
        match self {
            Self::Running => (widgets::spinner_frame().to_string(), "Checks running"),
            Self::Failed => (icons::TIMES_CIRCLE.to_string(), "Some checks failed"),
            Self::AllPassed => (icons::CHECK_CIRCLE.to_string(), "All checks passed"),
            Self::Partial => (icons::QUESTION_CIRCLE.to_string(), "Checks complete"),
        }
    }
}

pub(super) fn progress_bar(builds: &[Build]) -> Vec<Span<'static>> {
    builds
        .iter()
        .map(|b| Span::styled("\u{25b0}", Style::default().fg(state_color(b.state))))
        .collect()
}

pub(super) fn state_color(state: BuildState) -> Color {
    let theme = theme::current();
    match state {
        BuildState::Successful => theme.success,
        BuildState::Failed => theme.error,
        BuildState::InProgress => theme.warning,
        BuildState::Cancelled | BuildState::Unknown => theme.muted,
    }
}

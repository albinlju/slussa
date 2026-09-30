pub mod builds;
pub mod commits;
pub mod description;
pub mod overview;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DetailTab {
    #[default]
    Description,
    Overview,
    Diff,
    Commits,
    Builds,
}

impl DetailTab {
    pub const ALL: [Self; 5] = [
        Self::Description,
        Self::Overview,
        Self::Diff,
        Self::Commits,
        Self::Builds,
    ];
}

impl DetailTab {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Description => "Description",
            Self::Overview => "Overview",
            Self::Diff => "Diff",
            Self::Commits => "Commits",
            Self::Builds => "Builds",
        }
    }
}

impl DetailTab {
    pub fn supported_by(self, caps: &crate::domain::capabilities::Capabilities) -> bool {
        self != Self::Builds || caps.supports(crate::domain::capabilities::Feature::Builds)
    }
    pub fn available(caps: &crate::domain::capabilities::Capabilities) -> Vec<Self> {
        Self::ALL
            .into_iter()
            .filter(|tab| tab.supported_by(caps))
            .collect()
    }
    pub fn step(self, delta: i16, caps: &crate::domain::capabilities::Capabilities) -> Self {
        let tabs = Self::available(caps);
        let current = tabs.iter().position(|tab| *tab == self).unwrap_or(0);
        tabs[(current as i16 + delta).rem_euclid(tabs.len() as i16) as usize]
    }
}

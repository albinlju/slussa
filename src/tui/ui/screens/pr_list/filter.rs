//! Which PRs the list shows and in what order.
use crate::domain::pr::{PrGroup, PrStatus};

/// How the list is ordered. All but the first keep the provider's order
/// (newest first) for rows that tie.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Sort {
    /// PRs that need the viewer first, most urgent first, then the rest.
    #[default]
    Attention,
    /// The newest PR first: the provider's order only.
    Recent,
    /// The PR with the latest activity first.
    Updated,
    /// The oldest PR first, to find what has been left.
    Oldest,
}

impl Sort {
    /// Every sort, in the order the picker lists them.
    pub const CYCLE: [Self; 4] = [Self::Attention, Self::Recent, Self::Updated, Self::Oldest];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Attention => "Needs you first",
            Self::Recent => "Newest",
            Self::Updated => "Recently updated",
            Self::Oldest => "Oldest",
        }
    }

    /// The sort below this one in the picker; the last stays where it is.
    pub const fn next(self) -> Self {
        match self {
            Self::Attention => Self::Recent,
            Self::Recent => Self::Updated,
            Self::Updated | Self::Oldest => Self::Oldest,
        }
    }

    /// The sort above this one in the picker; the first stays where it is.
    pub const fn previous(self) -> Self {
        match self {
            Self::Attention | Self::Recent => Self::Attention,
            Self::Updated => Self::Recent,
            Self::Oldest => Self::Updated,
        }
    }

    /// The value written in `config.toml`; unset or unknown means attention.
    pub fn from_config(name: Option<&str>) -> Self {
        match name {
            None | Some("attention") => Self::Attention,
            Some("recent") => Self::Recent,
            Some("updated") => Self::Updated,
            Some("oldest") => Self::Oldest,
            Some(other) => {
                tracing::warn!("unknown sort {other:?}, using attention");
                Self::Attention
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StatusFilter {
    #[default]
    Open,
    Draft,
    Merged,
    Declined,
    All,
}

impl StatusFilter {
    /// Every filter, in the order the picker lists them.
    pub const CYCLE: [Self; 5] = [
        Self::Open,
        Self::Draft,
        Self::Merged,
        Self::Declined,
        Self::All,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Draft => "Draft",
            Self::Merged => "Merged",
            Self::Declined => "Declined",
            Self::All => "All",
        }
    }

    /// The filter below this one in the picker; the last stays where it is.
    pub const fn next(self) -> Self {
        match self {
            Self::Open => Self::Draft,
            Self::Draft => Self::Merged,
            Self::Merged => Self::Declined,
            Self::Declined | Self::All => Self::All,
        }
    }

    /// The filter above this one in the picker; the first stays where it is.
    pub const fn previous(self) -> Self {
        match self {
            Self::Open | Self::Draft => Self::Open,
            Self::Merged => Self::Draft,
            Self::Declined => Self::Merged,
            Self::All => Self::Declined,
        }
    }

    /// The groups of PRs this view shows. Open and Draft share one: a provider
    /// does not separate drafts when asked for the open ones.
    pub const fn groups(self) -> &'static [PrGroup] {
        match self {
            Self::Open | Self::Draft => &[PrGroup::Open],
            Self::Merged => &[PrGroup::Merged],
            Self::Declined => &[PrGroup::Declined],
            Self::All => &PrGroup::ALL,
        }
    }

    /// The list's heading. PRs are read a batch at a time past a limit, so
    /// while more remain unread the views that show them say so rather than
    /// imply the count is everything.
    pub const fn title(self, more: bool) -> &'static str {
        match (self, more) {
            (Self::Open, false) => "Open",
            (Self::Open, true) => "Open, more unread",
            (Self::Draft, false) => "Draft",
            (Self::Draft, true) => "Draft, more unread",
            (Self::Merged, false) => "Merged",
            (Self::Merged, true) => "Merged, recent",
            (Self::Declined, false) => "Declined",
            (Self::Declined, true) => "Declined, recent",
            (Self::All, false) => "All",
            (Self::All, true) => "All, more unread",
        }
    }

    pub const fn matches(self, status: &PrStatus) -> bool {
        matches!(
            (self, status),
            (Self::All, _)
                | (Self::Open, PrStatus::Open)
                | (Self::Draft, PrStatus::Draft)
                | (Self::Merged, PrStatus::Merged)
                | (Self::Declined, PrStatus::Declined)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Sort, StatusFilter};
    use crate::doc_contract;

    #[test]
    fn the_readme_lists_every_sort_and_marks_the_default() {
        let listed: Vec<_> = doc_contract::readme_values("sort")
            .iter()
            .map(|name| Sort::from_config(Some(name)))
            .collect();
        // An unknown name falls back to attention, so a misspelt one shows up
        // here as attention twice.
        assert_eq!(listed, Sort::CYCLE, "the README's `sort` line against Sort");
        let default = doc_contract::readme_default("sort");
        assert_eq!(Sort::from_config(Some(&default)), Sort::default());
        assert_eq!(default, "attention");
    }

    #[test]
    fn the_sort_picker_steps_through_every_sort_and_stops_at_the_ends() {
        let mut down = vec![Sort::Attention];
        while let Some(&last) = down.last()
            && last.next() != last
        {
            down.push(last.next());
        }
        assert_eq!(down, Sort::CYCLE);

        let mut up = vec![Sort::Oldest];
        while let Some(&last) = up.last()
            && last.previous() != last
        {
            up.push(last.previous());
        }
        up.reverse();
        assert_eq!(up, Sort::CYCLE);
    }

    #[test]
    fn the_picker_steps_through_every_filter_and_stops_at_the_ends() {
        let mut down = vec![StatusFilter::Open];
        while let Some(&last) = down.last()
            && last.next() != last
        {
            down.push(last.next());
        }
        assert_eq!(down, StatusFilter::CYCLE);

        let mut up = vec![StatusFilter::All];
        while let Some(&last) = up.last()
            && last.previous() != last
        {
            up.push(last.previous());
        }
        up.reverse();
        assert_eq!(up, StatusFilter::CYCLE);
    }
}

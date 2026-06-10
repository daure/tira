/// A top-level application tab. Declaration order is display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationTab {
    Board,
    List,
    Timeline,
    Filters,
}

impl ApplicationTab {
    pub fn title(self) -> &'static str {
        match self {
            Self::Board => "Board",
            Self::List => "List",
            Self::Timeline => "Timeline",
            Self::Filters => "Filters",
        }
    }

    pub fn all() -> [ApplicationTab; 4] {
        [Self::Board, Self::List, Self::Timeline, Self::Filters]
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(index: usize) -> Option<ApplicationTab> {
        Self::all().get(index).copied()
    }
}

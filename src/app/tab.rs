/// A top-level application tab. Declaration order is display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationTab {
    Board,
    List,
    Timeline,
}

impl ApplicationTab {
    pub fn title(self) -> &'static str {
        match self {
            Self::Board => "Board",
            Self::List => "List",
            Self::Timeline => "Timeline",
        }
    }

    pub fn all() -> [ApplicationTab; 3] {
        [Self::Board, Self::List, Self::Timeline]
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(index: usize) -> Option<ApplicationTab> {
        Self::all().get(index).copied()
    }
}

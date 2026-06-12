use crate::components::{
    generic::{dropdown::DropdownAction, filter::FilterAction, tabs::TabAction, tree::TreeAction},
    jira::filtered_tree::JiraFilteredTreeAction,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Tabs(TabAction),
    JiraFilteredTree(JiraFilteredTreeAction),
    /// Pan the issue table horizontally by N cells (negative = left).
    ScrollListHorizontal(i32),
    ReloadList,
    ReloadBoard,
    ReloadTimeline,
    Board(BoardAction),
    MoveBoardTicket(BoardTicketDirection),
    ToggleBoardTicketMoveMode,
    PlaceBoardTicketMoveMode,
    Leader,
    FocusBoardFilter,
    ClearBoardFilter,
    ToggleBoardGrouping,
    ToggleCommandLog,
    CloseCommandLog,
    ScrollCommandLog(isize),
    PageCommandLog(isize),
    HalfPageCommandLog(isize),
    CommandLogToStart,
    CommandLogToStartPrefix,
    CommandLogToEnd,
    ToggleSprintDetails,
    CloseSprintDetails,
    ToggleProjectDropdown,
    ProjectDropdown(DropdownAction),
    ToggleQuickSwitcher,
    QuickSwitcher(DropdownAction),
    ToggleThemeDropdown,
    ThemeDropdown(DropdownAction),
    ToggleAssigneeDropdown,
    AssigneeDropdown(DropdownAction),
    BoardGroupDropdown(DropdownAction),
    AssignSelectedToMe,
    UnassignSelected,
    GoToBoard,
    GoToList,
    GoToTimeline,
    Timeline(TimelineAction),
    OpenTicketDialog,
    CloseTicketDialog,
    TicketDialog(TicketDialogAction),
    OpenHelp,
    CloseHelp,
    Quit,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardTicketDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardAction {
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    HalfPageUp,
    HalfPageDown,
    GoToStart,
    GoToEnd,
    GoToStartPrefix,
    ToggleCollapse,
    CollapseAllGroups,
    ExpandAllGroups,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineAction {
    /// A row-tree action (navigation, expand/collapse) routed to the timeline's
    /// backing tree — the same actions the List view uses.
    Tree(TreeAction),
    FocusFilter,
    ClearFilter,
    Filter(FilterAction),
    ScrollLeft,
    ScrollRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TicketDialogAction {
    PreviousTab,
    NextTab,
}

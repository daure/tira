use super::BoardAction;
use crate::services::jira::{
    BoardColumnSummary, BoardData, BoardSwimlaneSummary, IssueSummary, UserSummary,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardGrouping {
    None,
    Assignee,
    Epic,
    Stories,
    Spaces,
}

impl BoardGrouping {
    pub const ALL: [Self; 5] = [
        Self::None,
        Self::Assignee,
        Self::Epic,
        Self::Stories,
        Self::Spaces,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Assignee => "Assignee",
            Self::Epic => "Epic",
            Self::Stories => "Stories",
            Self::Spaces => "Spaces",
        }
    }

    /// Whether this grouping splits the board into swimlanes (anything but None).
    pub fn is_grouped(self) -> bool {
        self != Self::None
    }

    /// Label for the catch-all swimlane that always sorts to the bottom.
    fn catch_all_label(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Assignee => "Unassigned",
            Self::Epic => "No Epic",
            Self::Stories => "Other work items",
            Self::Spaces => "Other",
        }
    }

    /// Resolve the swimlane label for an issue under this grouping, or `None`
    /// when the issue belongs to the catch-all lane.
    fn group_label(self, issue: &crate::services::jira::IssueSummary) -> Option<String> {
        let field = match self {
            Self::None => return None,
            Self::Assignee => "assignee",
            Self::Epic => "epic_summary",
            Self::Stories => "parent",
            Self::Spaces => {
                // "Spaces" == projects; derive the project key from the issue key
                // prefix (e.g. "DPP-123" -> "DPP").
                return issue
                    .key
                    .split_once('-')
                    .map(|(prefix, _)| prefix.to_owned());
            }
        };
        issue
            .field_values
            .get(field)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    }
}

#[derive(Debug)]
pub struct BoardState {
    data: Option<BoardData>,
    error: Option<String>,
    selected_issue_key: Option<String>,
    /// Column the user is navigating in, preserved across group-header rows so
    /// vertical movement stays in the same column (landing on empty cells too).
    preferred_column: usize,
    collapsed_groups: std::collections::BTreeSet<String>,
    pub scroll_offset: std::cell::Cell<usize>,
    pub col_scroll_offset: std::cell::Cell<usize>,
    /// Smoothly glides the rendered vertical line offset toward `scroll_offset`.
    pub v_scroll: crate::components::generic::scroll_animator::ScrollAnimator,
    /// Smoothly glides the rendered horizontal cell offset toward its target.
    pub h_scroll: crate::components::generic::scroll_animator::ScrollAnimator,
    /// When true the viewport was scrolled by the user (wheel), so rendering
    /// shows the manual offset instead of following the selection. Cleared on
    /// the next keyboard navigation.
    pub manual_v_scroll: std::cell::Cell<bool>,
    pub manual_h_scroll: std::cell::Cell<bool>,
    /// Manual horizontal offset in cells, used while `manual_h_scroll` is set.
    pub manual_h_offset: std::cell::Cell<u16>,
    pub column_widths: std::cell::RefCell<Vec<usize>>,
}

impl Clone for BoardState {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            error: self.error.clone(),
            selected_issue_key: self.selected_issue_key.clone(),
            preferred_column: self.preferred_column,
            collapsed_groups: self.collapsed_groups.clone(),
            scroll_offset: std::cell::Cell::new(self.scroll_offset.get()),
            col_scroll_offset: std::cell::Cell::new(self.col_scroll_offset.get()),
            v_scroll: self.v_scroll.clone(),
            h_scroll: self.h_scroll.clone(),
            manual_v_scroll: std::cell::Cell::new(self.manual_v_scroll.get()),
            manual_h_scroll: std::cell::Cell::new(self.manual_h_scroll.get()),
            manual_h_offset: std::cell::Cell::new(self.manual_h_offset.get()),
            column_widths: std::cell::RefCell::new(self.column_widths.borrow().clone()),
        }
    }
}

impl PartialEq for BoardState {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
            && self.error == other.error
            && self.selected_issue_key == other.selected_issue_key
            && self.collapsed_groups == other.collapsed_groups
    }
}

impl Eq for BoardState {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BoardCell {
    lane: usize,
    column: usize,
    index: usize,
    key: String,
    group: String,
    is_group: bool,
}

impl BoardState {
    pub(crate) fn empty() -> Self {
        Self {
            data: None,
            error: None,
            selected_issue_key: None,
            preferred_column: 0,
            collapsed_groups: std::collections::BTreeSet::new(),
            scroll_offset: std::cell::Cell::new(0),
            col_scroll_offset: std::cell::Cell::new(0),
            v_scroll: crate::components::generic::scroll_animator::ScrollAnimator::new(),
            h_scroll: crate::components::generic::scroll_animator::ScrollAnimator::new(),
            manual_v_scroll: std::cell::Cell::new(false),
            manual_h_scroll: std::cell::Cell::new(false),
            manual_h_offset: std::cell::Cell::new(0),
            column_widths: std::cell::RefCell::new(Vec::new()),
        }
    }

    pub(crate) fn from_issues(issues: Vec<IssueSummary>) -> Self {
        let mut board = Self::empty();
        board.set_data(BoardData::from_issues(issues));
        board
    }

    pub(crate) fn set_data(&mut self, data: BoardData) {
        let previous = self.selected_issue_key.clone();
        let cells = board_cells_for_lanes(
            &data,
            &data.swimlanes,
            "",
            &self.collapsed_groups,
            BoardGrouping::None,
        );
        self.selected_issue_key = previous
            .filter(|key| cells.iter().any(|cell| cell.key == *key))
            .or_else(|| cells.first().map(|cell| cell.key.clone()));
        self.data = Some(data);
        self.error = None;
    }

    pub(crate) fn select_first(&mut self, search: &str, grouping: BoardGrouping) {
        let Some(data) = &self.data else {
            self.selected_issue_key = None;
            return;
        };
        let lanes = board_grouped_lanes(data, grouping);
        let cells = board_cells_for_lanes(data, &lanes, search, &self.collapsed_groups, grouping);
        self.selected_issue_key = cells.first().map(|cell| cell.key.clone());
    }

    pub(crate) fn set_error(&mut self, error: String) {
        self.error = Some(error);
    }

    pub fn data(&self) -> Option<&BoardData> {
        self.data.as_ref()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn selected_issue_key(&self) -> Option<&str> {
        self.selected_issue_key
            .as_deref()
            .filter(|key| !is_board_group_key(key) && !is_board_empty_key(key))
    }

    pub fn selected_group(&self) -> Option<&str> {
        self.selected_issue_key
            .as_deref()
            .and_then(board_group_key_name)
    }

    /// The raw selected key (issue, group header, or empty cell), used to look up
    /// the selection's position for scrolling.
    pub fn selected_raw_key(&self) -> Option<&str> {
        self.selected_issue_key.as_deref()
    }

    /// The `(group, column)` of a focused empty column, if one is selected.
    pub fn selected_empty_cell(&self) -> Option<(&str, usize)> {
        self.selected_issue_key
            .as_deref()
            .and_then(board_empty_cell_parts)
    }

    pub fn is_group_collapsed(&self, group: &str) -> bool {
        self.collapsed_groups.contains(group)
    }

    pub(crate) fn update_assignee(&mut self, issue_key: &str, assignee_name: Option<String>) {
        let Some(data) = &mut self.data else {
            return;
        };
        let Some(issue) = data.issues.iter_mut().find(|issue| issue.key == issue_key) else {
            return;
        };
        match assignee_name {
            Some(name) => {
                issue.field_values.insert(String::from("assignee"), name);
            }
            None => {
                issue.field_values.remove("assignee");
            }
        }
    }

    pub fn selected_issue_index(&self, search: &str, grouping: BoardGrouping) -> usize {
        let Some(data) = &self.data else {
            return 0;
        };
        let Some(selected) = &self.selected_issue_key else {
            return 0;
        };
        let lanes = board_grouped_lanes(data, grouping);
        let cells = board_cells_for_lanes(data, &lanes, search, &self.collapsed_groups, grouping);
        cells
            .iter()
            .position(|cell| &cell.key == selected)
            .unwrap_or(0)
    }

    /// Scroll the board viewport vertically by `delta` lines without moving the
    /// selection. Render clamps the upper bound (it knows the content height).
    pub fn scroll_viewport(&self, delta: isize) {
        let next = self.scroll_offset.get().saturating_add_signed(delta);
        self.scroll_offset.set(next);
        self.manual_v_scroll.set(true);
    }

    /// Scroll the board viewport horizontally by `delta` cells without moving
    /// the selection. Seeds from the currently rendered offset on the first
    /// tick so the view doesn't jump. Render clamps to the strip width.
    pub fn scroll_viewport_horizontal(&self, delta: i32) {
        let base = if self.manual_h_scroll.get() {
            self.manual_h_offset.get()
        } else {
            self.h_scroll.current().round().max(0.0) as u16
        };
        let step = delta.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
        self.manual_h_offset.set(base.saturating_add_signed(step));
        self.manual_h_scroll.set(true);
    }

    pub(crate) fn dispatch(&mut self, action: BoardAction, search: &str, grouping: BoardGrouping) {
        // Keyboard navigation re-enables selection-following for both axes.
        self.manual_v_scroll.set(false);
        self.manual_h_scroll.set(false);
        let Some(data) = &self.data else {
            return;
        };
        let lanes = board_grouped_lanes(data, grouping);
        let cells = board_cells_for_lanes(data, &lanes, search, &self.collapsed_groups, grouping);
        if cells.is_empty() {
            self.selected_issue_key = None;
            return;
        }
        let Some(selected_index) = self
            .selected_issue_key
            .as_ref()
            .and_then(|key| cells.iter().position(|cell| &cell.key == key))
        else {
            self.selected_issue_key = cells.first().map(|cell| cell.key.clone());
            return;
        };

        let selected = &cells[selected_index];
        // Track the column the user is in so vertical moves preserve it across
        // group headers (where the cell's own column is meaningless).
        if !selected.is_group {
            self.preferred_column = selected.column;
        }
        let preferred_column = self
            .preferred_column
            .min(data.columns.len().saturating_sub(1));
        let next = match action {
            BoardAction::MoveLeft if selected.is_group => {
                self.collapsed_groups.insert(selected.group.clone());
                None
            }
            BoardAction::MoveRight if selected.is_group => {
                self.collapsed_groups.remove(&selected.group);
                Some(board_lane_column_entry(
                    data,
                    &lanes[selected.lane],
                    preferred_column,
                    search,
                    false,
                ))
            }
            BoardAction::ToggleCollapse if selected.is_group => {
                if !self.collapsed_groups.remove(&selected.group) {
                    self.collapsed_groups.insert(selected.group.clone());
                }
                None
            }
            BoardAction::MoveLeft => board_horizontal_target(data, &lanes, selected, -1, search)
                .or_else(|| {
                    if grouping.is_grouped() {
                        Some(board_group_key(&selected.group))
                    } else {
                        None
                    }
                }),
            BoardAction::MoveRight => board_horizontal_target(data, &lanes, selected, 1, search),
            BoardAction::MoveUp => board_vertical_target(
                data,
                &lanes,
                &self.collapsed_groups,
                grouping,
                selected,
                preferred_column,
                search,
                -1,
            ),
            BoardAction::MoveDown => board_vertical_target(
                data,
                &lanes,
                &self.collapsed_groups,
                grouping,
                selected,
                preferred_column,
                search,
                1,
            ),
            BoardAction::HalfPageUp => board_page_vertical(
                data,
                &lanes,
                &cells,
                &self.collapsed_groups,
                grouping,
                selected,
                preferred_column,
                search,
                -1,
            ),
            BoardAction::HalfPageDown => board_page_vertical(
                data,
                &lanes,
                &cells,
                &self.collapsed_groups,
                grouping,
                selected,
                preferred_column,
                search,
                1,
            ),
            BoardAction::GoToStart => cells
                .iter()
                .find(|cell| !cell.is_group && cell.column == preferred_column)
                .map(|cell| cell.key.clone()),
            BoardAction::GoToEnd => cells
                .iter()
                .rev()
                .find(|cell| !cell.is_group && cell.column == preferred_column)
                .map(|cell| cell.key.clone()),
            BoardAction::GoToStartPrefix => None,
            BoardAction::CollapseAllGroups => {
                let selected_group = selected.group.clone();
                self.collapsed_groups.extend(
                    lanes
                        .iter()
                        .enumerate()
                        .filter(|(index, _lane)| {
                            board_first_group_issue_key(data, &lanes, *index, search).is_some()
                        })
                        .map(|(_, lane)| lane.name.clone()),
                );
                Some(board_group_key(&selected_group))
            }
            BoardAction::ExpandAllGroups => {
                self.collapsed_groups.clear();
                None
            }
            BoardAction::ToggleCollapse => None,
        };

        if let Some(next) = next {
            // Keep the preferred column in sync with the landing cell (but not
            // group headers, which would reset it to 0).
            if let Some(cell) = cells.iter().find(|cell| cell.key == next)
                && !cell.is_group
            {
                self.preferred_column = cell.column;
            }
            self.selected_issue_key = Some(next);
        }
    }
}

fn board_cells_for_lanes(
    data: &BoardData,
    lanes: &[BoardSwimlaneSummary],
    search: &str,
    collapsed_groups: &std::collections::BTreeSet<String>,
    grouping: BoardGrouping,
) -> Vec<BoardCell> {
    let mut cells = Vec::new();
    for (lane_index, lane) in lanes.iter().enumerate() {
        let group = lane.name.clone();
        let show_header = grouping.is_grouped();
        if show_header
            && lane.issue_keys.iter().any(|key| {
                data.issues
                    .iter()
                    .find(|issue| issue.key.as_str() == key.as_str())
                    .is_some_and(|issue| board_issue_matches_search(issue, search))
            })
        {
            cells.push(BoardCell {
                lane: lane_index,
                column: 0,
                index: 0,
                key: board_group_key(&group),
                group: group.clone(),
                is_group: true,
            });
        }
        if show_header && collapsed_groups.contains(&group) {
            continue;
        }
        for (column_index, _column) in data.columns.iter().enumerate() {
            let keys = board_lane_column_keys(data, lane, column_index, search);
            if keys.is_empty() {
                // Empty columns are still focusable so left/right navigation
                // steps through them instead of jumping over the gaps.
                cells.push(BoardCell {
                    lane: lane_index,
                    column: column_index,
                    index: 0,
                    key: board_empty_cell_key(&group, column_index),
                    group: group.clone(),
                    is_group: false,
                });
                continue;
            }
            cells.extend(keys.into_iter().enumerate().map(|(index, key)| BoardCell {
                lane: lane_index,
                column: column_index,
                index,
                key,
                group: group.clone(),
                is_group: false,
            }));
        }
    }
    cells
}

pub(crate) fn board_group_key(group: &str) -> String {
    format!("__board_group__:{group}")
}

pub(crate) fn is_board_group_key(key: &str) -> bool {
    key.starts_with("__board_group__:")
}

pub(crate) fn board_group_key_name(key: &str) -> Option<&str> {
    key.strip_prefix("__board_group__:")
}

/// Synthetic selection key for a column that currently has no issues, so empty
/// columns are focusable when navigating (carries the lane group and column).
/// The unit separator keeps it unambiguous against arbitrary group names.
pub(crate) fn board_empty_cell_key(group: &str, column: usize) -> String {
    format!("__board_empty__:{column}\u{1f}{group}")
}

pub(crate) fn is_board_empty_key(key: &str) -> bool {
    key.starts_with("__board_empty__:")
}

/// Parses an empty-cell key back into `(group, column)`.
pub(crate) fn board_empty_cell_parts(key: &str) -> Option<(&str, usize)> {
    let rest = key.strip_prefix("__board_empty__:")?;
    let (column, group) = rest.split_once('\u{1f}')?;
    Some((group, column.parse().ok()?))
}

pub(crate) fn board_grouped_lanes(
    data: &BoardData,
    grouping: BoardGrouping,
) -> Vec<BoardSwimlaneSummary> {
    if !grouping.is_grouped() {
        return data.swimlanes.clone();
    }
    let catch_all = grouping.catch_all_label();
    let mut groups = std::collections::BTreeMap::<String, Vec<String>>::new();
    for issue in &data.issues {
        let group = grouping
            .group_label(issue)
            .unwrap_or_else(|| catch_all.to_owned());
        groups.entry(group).or_default().push(issue.key.clone());
    }
    let mut lanes: Vec<BoardSwimlaneSummary> = groups
        .into_iter()
        .map(|(name, issue_keys)| BoardSwimlaneSummary {
            id: None,
            name,
            issue_keys,
        })
        .collect();
    // Keep names alphabetical but always push the catch-all lane to the bottom.
    lanes.sort_by(|a, b| {
        let a_catch = a.name == catch_all;
        let b_catch = b.name == catch_all;
        a_catch.cmp(&b_catch).then_with(|| a.name.cmp(&b.name))
    });
    lanes
}

fn board_horizontal_target(
    data: &BoardData,
    lanes: &[BoardSwimlaneSummary],
    selected: &BoardCell,
    direction: isize,
    search: &str,
) -> Option<String> {
    let column = selected.column as isize + direction;
    if column < 0 || column as usize >= data.columns.len() {
        return None;
    }
    let column = column as usize;
    let keys = board_lane_column_keys(data, &lanes[selected.lane], column, search);
    if keys.is_empty() {
        // Land on the empty column rather than skipping to the next populated
        // one, so horizontal movement is steady instead of jumpy.
        Some(board_empty_cell_key(&lanes[selected.lane].name, column))
    } else {
        keys.get(selected.index.min(keys.len() - 1)).cloned()
    }
}

fn board_first_group_issue_key(
    data: &BoardData,
    lanes: &[BoardSwimlaneSummary],
    lane_index: usize,
    search: &str,
) -> Option<String> {
    for column in 0..data.columns.len() {
        if let Some(key) = board_lane_column_keys(data, &lanes[lane_index], column, search).first()
        {
            return Some(key.clone());
        }
    }
    None
}

/// The selection key for entering `column` of `lane` — the first/last card, or
/// the empty-cell placeholder when the column has no issues.
fn board_lane_column_entry(
    data: &BoardData,
    lane: &BoardSwimlaneSummary,
    column: usize,
    search: &str,
    take_last: bool,
) -> String {
    let keys = board_lane_column_keys(data, lane, column, search);
    let chosen = if take_last { keys.last() } else { keys.first() };
    chosen
        .cloned()
        .unwrap_or_else(|| board_empty_cell_key(&lane.name, column))
}

/// Next selection one row up (`direction < 0`) or down (`direction > 0`),
/// preserving `preferred_column` across group-header rows and landing on empty
/// column cells rather than skipping to a populated column.
#[allow(clippy::too_many_arguments)]
fn board_vertical_target(
    data: &BoardData,
    lanes: &[BoardSwimlaneSummary],
    collapsed: &std::collections::BTreeSet<String>,
    grouping: BoardGrouping,
    selected: &BoardCell,
    preferred_column: usize,
    search: &str,
    direction: isize,
) -> Option<String> {
    let show_header = grouping.is_grouped();
    if direction < 0 {
        if selected.is_group {
            if selected.lane == 0 {
                return None;
            }
            let prev = selected.lane - 1;
            let prev_group = &lanes[prev].name;
            if show_header && collapsed.contains(prev_group) {
                Some(board_group_key(prev_group))
            } else {
                // Land on the bottom of the preferred column in the lane above.
                Some(board_lane_column_entry(
                    data,
                    &lanes[prev],
                    preferred_column,
                    search,
                    true,
                ))
            }
        } else {
            let keys = board_lane_column_keys(data, &lanes[selected.lane], selected.column, search);
            let pos = keys.iter().position(|key| *key == selected.key);
            if let Some(prev) = pos.and_then(|p| p.checked_sub(1)) {
                Some(keys[prev].clone())
            } else if show_header {
                Some(board_group_key(&selected.group))
            } else {
                None
            }
        }
    } else if selected.is_group {
        if show_header && collapsed.contains(&selected.group) {
            (selected.lane + 1 < lanes.len())
                .then(|| board_group_key(&lanes[selected.lane + 1].name))
        } else {
            // Enter the top of the preferred column in this lane.
            Some(board_lane_column_entry(
                data,
                &lanes[selected.lane],
                preferred_column,
                search,
                false,
            ))
        }
    } else {
        let keys = board_lane_column_keys(data, &lanes[selected.lane], selected.column, search);
        let pos = keys.iter().position(|key| *key == selected.key);
        if let Some(next) = pos.filter(|p| p + 1 < keys.len()) {
            Some(keys[next + 1].clone())
        } else if show_header && selected.lane + 1 < lanes.len() {
            Some(board_group_key(&lanes[selected.lane + 1].name))
        } else {
            None
        }
    }
}

/// Half-page vertical jump: repeatedly steps up/down, passing *through* group
/// headers (they aren't landing spots) and counting cards and empty cells, so
/// the jump lands a few rows away without skipping empty columns.
#[allow(clippy::too_many_arguments)]
fn board_page_vertical(
    data: &BoardData,
    lanes: &[BoardSwimlaneSummary],
    cells: &[BoardCell],
    collapsed: &std::collections::BTreeSet<String>,
    grouping: BoardGrouping,
    selected: &BoardCell,
    preferred_column: usize,
    search: &str,
    direction: isize,
) -> Option<String> {
    const PAGE: usize = 4;
    let mut current = selected.key.clone();
    let mut landed = 0usize;
    let mut result = None;
    // Bounded by the number of cells (every step advances to a distinct key).
    for _ in 0..cells.len().saturating_add(PAGE) {
        let Some(cell) = cells.iter().find(|cell| cell.key == current) else {
            break;
        };
        let Some(next) = board_vertical_target(
            data,
            lanes,
            collapsed,
            grouping,
            cell,
            preferred_column,
            search,
            direction,
        ) else {
            break;
        };
        if next == current {
            break;
        }
        current = next;
        let landed_on_group = cells
            .iter()
            .find(|cell| cell.key == current)
            .is_some_and(|cell| cell.is_group);
        if !landed_on_group {
            result = Some(current.clone());
            landed += 1;
            if landed >= PAGE {
                break;
            }
        }
    }
    result
}

fn board_lane_column_keys(
    data: &BoardData,
    lane: &BoardSwimlaneSummary,
    column_index: usize,
    search: &str,
) -> Vec<String> {
    lane.issue_keys
        .iter()
        .filter(|key| {
            data.issues
                .iter()
                .find(|issue| issue.key.as_str() == key.as_str())
                .is_some_and(|issue| {
                    board_issue_column(data, issue) == column_index
                        && board_issue_matches_search(issue, search)
                })
        })
        .cloned()
        .collect()
}

fn board_issue_matches_search(issue: &IssueSummary, search: &str) -> bool {
    let search = search.trim();
    if search.is_empty() {
        return true;
    }
    let search = search.to_ascii_lowercase();
    issue.key.to_ascii_lowercase().contains(&search)
        || issue.summary.to_ascii_lowercase().contains(&search)
        || issue.status.to_ascii_lowercase().contains(&search)
        || issue.issue_type.to_ascii_lowercase().contains(&search)
        || board_displayed_field_matches(issue, "epic_summary", &search)
        || board_displayed_field_matches(issue, "labels", &search)
        || board_displayed_field_matches(issue, "dueDate", &search)
        || board_displayed_field_matches(issue, "priorityName", &search)
        || board_assignee_matches(issue, &search)
}

fn board_assignee_matches(issue: &IssueSummary, search: &str) -> bool {
    issue.field_values.get("assignee").is_some_and(|assignee| {
        let assignee = assignee.to_ascii_lowercase();
        let initials = crate::components::generic::avatar::initials(&assignee).to_ascii_lowercase();
        assignee.contains(search) || initials.contains(search)
    })
}

fn board_displayed_field_matches(issue: &IssueSummary, field: &str, search: &str) -> bool {
    issue
        .field_values
        .get(field)
        .is_some_and(|value| value.to_ascii_lowercase().contains(search))
}

pub fn board_issue_column(data: &BoardData, issue: &IssueSummary) -> usize {
    let status_id = issue.field_values.get("status_id").map(String::as_str);
    data.columns
        .iter()
        .position(|column| board_column_contains_issue(column, issue, status_id))
        .unwrap_or(0)
}

fn board_column_contains_issue(
    column: &BoardColumnSummary,
    issue: &IssueSummary,
    status_id: Option<&str>,
) -> bool {
    if column.statuses.is_empty() {
        return column.name == issue.status;
    }
    column
        .statuses
        .iter()
        .any(|status| Some(status.as_str()) == status_id || status == &issue.status)
}

pub(crate) fn normalize_board_user_fields(
    board: &mut BoardData,
    users: &[UserSummary],
    current_user: Option<&UserSummary>,
) {
    for issue in &mut board.issues {
        for field in ["assignee", "reporter"] {
            let Some(value) = issue.field_values.get_mut(field) else {
                continue;
            };
            if let Some(user) = users
                .iter()
                .chain(current_user)
                .find(|user| user.account_id == *value)
            {
                *value = user.display_name.clone();
            }
        }
    }
}

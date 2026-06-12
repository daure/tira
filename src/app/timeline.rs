use std::cell::Cell;
use std::collections::BTreeMap;
use std::time::Duration;

use crate::components::generic::filter::{FilterAction, FilterEvent, FilterState};
use crate::components::generic::scroll_animator::ScrollAnimator;
use crate::components::generic::tree::{Children, TreeAction, TreeItem, TreeState, fuzzy_matches};
use crate::services::jira::TimelineData;

/// View state for the Timeline tab. The epic/child rows are backed by a generic
/// [`TreeState`] so the tab inherits the List view's behaviour exactly:
/// centered and animated scrolling, every navigation key, expand/collapse by
/// id, and selection that survives reloads. This struct adds the
/// timeline-specific horizontal (date axis) scroll on top.
#[derive(Clone, PartialEq, Eq)]
pub struct TimelineState {
    data: Option<TimelineData>,
    error: Option<String>,
    loaded: bool,
    pending_request_id: Option<u64>,
    filter: FilterState,
    tree: TreeState,
    /// Per-load derived lookups the grid renderer reads each frame. Computed
    /// once at load so the hot render path never rebuilds them.
    sprint_dates: std::collections::BTreeMap<i64, (i64, i64)>,
    item_spans: std::collections::BTreeMap<String, (i64, i64)>,
    item_sprints: std::collections::BTreeMap<String, Vec<i64>>,
    epic_stats: std::collections::BTreeMap<String, crate::domain::models::TimelineEpicStats>,
    /// Desired left cell of the horizontal viewport, before clamping to the
    /// canvas width (known only at render). Adjusted by scroll keys.
    h_offset: Cell<i32>,
    /// Whether the initial centre-on-today offset has been applied yet.
    h_centered: Cell<bool>,
    /// Glides the rendered horizontal offset toward `h_offset`.
    h_scroll: ScrollAnimator,
    /// One-shot request to pan the date axis enough to include the selected
    /// item's range. Set by selection-changing tree navigation, then consumed by
    /// render so manual horizontal scrolling remains free afterwards.
    h_include_selected_on_next_render: Cell<bool>,
}

impl Default for TimelineState {
    fn default() -> Self {
        Self {
            data: None,
            error: None,
            loaded: false,
            pending_request_id: None,
            filter: FilterState::default(),
            tree: TreeState::new(Vec::new()),
            sprint_dates: BTreeMap::new(),
            item_spans: BTreeMap::new(),
            item_sprints: BTreeMap::new(),
            epic_stats: BTreeMap::new(),
            h_offset: Cell::new(0),
            h_centered: Cell::new(false),
            h_scroll: ScrollAnimator::new(),
            h_include_selected_on_next_render: Cell::new(false),
        }
    }
}

impl TimelineState {
    pub fn data(&self) -> Option<&TimelineData> {
        self.data.as_ref()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    pub fn is_loading(&self) -> bool {
        self.pending_request_id.is_some()
    }

    /// The row tree backing the sticky left column and the grid alignment.
    pub fn tree(&self) -> &TreeState {
        &self.tree
    }

    pub fn filter_state(&self) -> &FilterState {
        &self.filter
    }

    pub fn filter(&self) -> &str {
        self.filter.value()
    }

    pub fn filter_cursor(&self) -> usize {
        self.filter.cursor()
    }

    pub fn is_filter_focused(&self) -> bool {
        self.filter.is_focused()
    }

    pub fn sprint_dates(&self) -> &std::collections::BTreeMap<i64, (i64, i64)> {
        &self.sprint_dates
    }

    pub fn item_spans(&self) -> &std::collections::BTreeMap<String, (i64, i64)> {
        &self.item_spans
    }

    pub fn item_sprints(&self) -> &std::collections::BTreeMap<String, Vec<i64>> {
        &self.item_sprints
    }

    pub fn epic_stats(
        &self,
    ) -> &std::collections::BTreeMap<String, crate::domain::models::TimelineEpicStats> {
        &self.epic_stats
    }

    pub(crate) fn begin_load(&mut self, request_id: u64) {
        self.pending_request_id = Some(request_id);
    }

    pub(crate) fn is_pending(&self, request_id: u64) -> bool {
        self.pending_request_id == Some(request_id)
    }

    pub(crate) fn finish_load(&mut self, data: Result<TimelineData, String>) {
        self.pending_request_id = None;
        self.h_centered.set(false);
        match data {
            Ok(data) => {
                self.loaded = true;
                self.tree = TreeState::new(filtered_tree_items(&data, self.filter.value()));
                if !self.filter.value().trim().is_empty() {
                    self.tree.dispatch(TreeAction::ExpandAll);
                }
                self.sprint_dates = sprint_date_map(&data);
                self.item_spans = item_span_map(&data, &self.sprint_dates);
                self.item_sprints = item_sprint_map(&data);
                self.epic_stats = epic_stat_map(&data);
                self.h_include_selected_on_next_render.set(true);
                self.data = Some(data);
                self.error = None;
            }
            Err(message) => self.error = Some(message),
        }
    }

    pub(crate) fn dispatch_tree(&mut self, action: TreeAction) {
        // The whole timeline is loaded up front, so any child-load request the
        // tree emits has nothing to fetch and is safely ignored.
        let selected_before = self.tree.selected_item_id().map(str::to_owned);
        let _ = self.tree.dispatch(action);
        if self.tree.selected_item_id() != selected_before.as_deref() {
            self.h_include_selected_on_next_render.set(true);
        }
    }

    pub(crate) fn focus_filter(&mut self) {
        self.filter.focus();
    }

    pub(crate) fn clear_filter(&mut self) {
        if self.filter.value().is_empty() {
            return;
        }
        self.filter.clear();
        self.refresh_filter();
    }

    pub(crate) fn dispatch_filter(&mut self, action: FilterAction) -> Option<FilterEvent> {
        let event = self.filter.dispatch(action)?;
        if matches!(event, FilterEvent::Changed) {
            self.refresh_filter();
        }
        Some(event)
    }

    fn refresh_filter(&mut self) {
        let Some(data) = &self.data else {
            return;
        };
        let filtering = !self.filter.value().trim().is_empty();
        let expanded = self.tree.expanded_item_ids().clone();
        self.tree
            .set_items(filtered_tree_items(data, self.filter.value()), &expanded);
        if filtering {
            self.tree.dispatch(TreeAction::ExpandAll);
        }
        self.h_include_selected_on_next_render.set(true);
    }

    /// Wheel-scrolls the rows without moving the selection (mouse support).
    pub(crate) fn scroll_viewport(&mut self, delta: isize, height: usize) {
        self.tree.scroll_viewport(delta, height);
    }

    pub fn tick(&self, dt: Duration) {
        self.h_scroll.tick(dt);
    }

    /// Advances the row-scroll glide (requires `&mut` like the List tree).
    pub(crate) fn tick_tree(&mut self, dt: Duration) {
        self.tree.tick(dt);
    }

    pub fn is_animating(&self) -> bool {
        self.h_scroll.is_animating() || self.tree.is_animating()
    }

    /// Nudges the horizontal scroll by `delta` cells and takes the viewport off
    /// auto-centre so it stays where the user put it.
    pub(crate) fn scroll_h(&mut self, delta: i32) {
        self.h_offset.set(self.h_offset.get() + delta);
        self.h_centered.set(true);
        self.h_include_selected_on_next_render.set(false);
    }

    /// Resolves the horizontal offset to render at for this frame's geometry.
    /// First frame for fresh data centres `today_x`; afterwards it clamps the
    /// user's offset to the canvas and glides toward it. Interior-mutable, so it
    /// must run once per frame before reading the animator.
    pub fn resolve_h_offset(
        &self,
        viewport: u16,
        total_width: u16,
        today_x: i32,
        selected_range: Option<(u16, u16)>,
    ) -> u16 {
        let max = i32::from(total_width.saturating_sub(viewport));
        let mut target = if self.h_centered.get() {
            let clamped = self.h_offset.get().clamp(0, max);
            self.h_offset.set(clamped);
            clamped
        } else {
            let centered = (today_x - i32::from(viewport) / 2).clamp(0, max);
            self.h_offset.set(centered);
            self.h_centered.set(true);
            self.h_scroll.snap_to(f64::from(centered));
            centered
        };
        if self.h_include_selected_on_next_render.replace(false)
            && let Some((start, end)) = selected_range
        {
            let adjusted = include_range_in_view(
                target,
                i32::from(viewport),
                i32::from(start),
                i32::from(end),
                max,
            );
            if adjusted != target {
                self.h_scroll.snap_to(f64::from(adjusted));
            }
            target = adjusted;
            self.h_offset.set(target);
        }
        self.h_scroll.set_target(f64::from(target));
        (self.h_scroll.current().round() as i32).clamp(0, max) as u16
    }
}

fn include_range_in_view(offset: i32, viewport: i32, start: i32, end: i32, max: i32) -> i32 {
    if end - start >= viewport {
        return start.clamp(0, max);
    }
    if start < offset {
        start.clamp(0, max)
    } else if end > offset + viewport {
        (end - viewport).clamp(0, max)
    } else {
        offset
    }
}

/// Flattens the timeline's epics and their children into tree items: each epic
/// is a root with its child issues nested beneath, so the generic tree handles
/// expansion, navigation and scrolling. Sprint/percentage data is looked up
/// separately by id at render time.
fn tree_items(data: &TimelineData) -> Vec<TreeItem> {
    let mut items = Vec::new();
    for epic in &data.epics {
        items.push(TreeItem {
            id: epic.key.clone(),
            label: epic.summary.clone(),
            kind: String::from("Epic"),
            status: epic.status.clone(),
            parent_id: None,
            root_order: 0,
            children: Children::Unknown,
            field_values: BTreeMap::new(),
        });
        for child in &epic.children {
            items.push(TreeItem {
                id: child.key.clone(),
                label: child.summary.clone(),
                kind: child.issue_type.clone(),
                status: child.status.clone(),
                parent_id: Some(epic.key.clone()),
                root_order: 0,
                children: Children::Unknown,
                field_values: BTreeMap::new(),
            });
        }
    }
    items
}

fn filtered_tree_items(data: &TimelineData, filter: &str) -> Vec<TreeItem> {
    if filter.trim().is_empty() {
        return tree_items(data);
    }

    let mut items = Vec::new();
    for epic in &data.epics {
        let epic_matches =
            timeline_item_matches_filter(&epic.key, &epic.summary, "Epic", &epic.status, filter);
        let matching_children = epic
            .children
            .iter()
            .filter(|child| {
                epic_matches
                    || timeline_item_matches_filter(
                        &child.key,
                        &child.summary,
                        &child.issue_type,
                        &child.status,
                        filter,
                    )
            })
            .collect::<Vec<_>>();
        if !epic_matches && matching_children.is_empty() {
            continue;
        }

        items.push(TreeItem {
            id: epic.key.clone(),
            label: epic.summary.clone(),
            kind: String::from("Epic"),
            status: epic.status.clone(),
            parent_id: None,
            root_order: 0,
            children: Children::Unknown,
            field_values: BTreeMap::new(),
        });
        for child in matching_children {
            items.push(TreeItem {
                id: child.key.clone(),
                label: child.summary.clone(),
                kind: child.issue_type.clone(),
                status: child.status.clone(),
                parent_id: Some(epic.key.clone()),
                root_order: 0,
                children: Children::Unknown,
                field_values: BTreeMap::new(),
            });
        }
    }
    items
}

fn timeline_item_matches_filter(
    key: &str,
    summary: &str,
    issue_type: &str,
    status: &str,
    filter: &str,
) -> bool {
    fuzzy_matches(key, filter)
        || fuzzy_matches(summary, filter)
        || fuzzy_matches(issue_type, filter)
        || fuzzy_matches(status, filter)
}

/// Sprint id to its `(start, end)` day range, for the dated sprints only.
fn sprint_date_map(data: &TimelineData) -> BTreeMap<i64, (i64, i64)> {
    data.sprints
        .iter()
        .filter_map(|sprint| match (sprint.start_day, sprint.end_day) {
            (Some(start), Some(end)) => Some((sprint.id, (start, end))),
            _ => None,
        })
        .collect()
}

/// Each row's item id to the sprint ids its work occupies: an epic's is the
/// union over its children, a child's is its own.
fn item_sprint_map(data: &TimelineData) -> BTreeMap<String, Vec<i64>> {
    let mut map = BTreeMap::new();
    for epic in &data.epics {
        map.insert(epic.key.clone(), epic.sprint_ids().into_iter().collect());
        for child in &epic.children {
            map.insert(child.key.clone(), child.sprint_ids.clone());
        }
    }
    map
}

fn item_span_map(
    data: &TimelineData,
    sprint_dates: &BTreeMap<i64, (i64, i64)>,
) -> BTreeMap<String, (i64, i64)> {
    let mut map = BTreeMap::new();
    for epic in &data.epics {
        if let Some(span) = epic_span(epic, sprint_dates) {
            map.insert(epic.key.clone(), span);
        }
        for child in &epic.children {
            if let Some(span) = explicit_or_sprint_span(
                child.start_day,
                child.end_day,
                &child.sprint_ids,
                sprint_dates,
            ) {
                map.insert(child.key.clone(), span);
            }
        }
    }
    map
}

fn epic_span(
    epic: &crate::domain::models::TimelineEpic,
    sprint_dates: &BTreeMap<i64, (i64, i64)>,
) -> Option<(i64, i64)> {
    explicit_or_sprint_span(epic.start_day, epic.end_day, &epic.sprint_ids, sprint_dates).or_else(
        || {
            let child_sprints = epic.sprint_ids().into_iter().collect::<Vec<_>>();
            sprint_span(&child_sprints, sprint_dates)
        },
    )
}

fn explicit_or_sprint_span(
    start_day: Option<i64>,
    end_day: Option<i64>,
    sprint_ids: &[i64],
    sprint_dates: &BTreeMap<i64, (i64, i64)>,
) -> Option<(i64, i64)> {
    match (start_day, end_day) {
        (Some(start), Some(end)) if start < end => Some((start, end)),
        _ => sprint_span(sprint_ids, sprint_dates),
    }
}

fn sprint_span(ids: &[i64], sprint_dates: &BTreeMap<i64, (i64, i64)>) -> Option<(i64, i64)> {
    let mut span: Option<(i64, i64)> = None;
    for id in ids {
        if let Some(&(start, end)) = sprint_dates.get(id) {
            span = Some(match span {
                Some((s, e)) => (s.min(start), e.max(end)),
                None => (start, end),
            });
        }
    }
    span
}

fn epic_stat_map(
    data: &TimelineData,
) -> BTreeMap<String, crate::domain::models::TimelineEpicStats> {
    data.epics
        .iter()
        .map(|epic| (epic.key.clone(), epic.stats))
        .collect()
}

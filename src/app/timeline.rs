use std::cell::Cell;
use std::collections::BTreeMap;
use std::time::Duration;

use crate::components::generic::scroll_animator::ScrollAnimator;
use crate::components::generic::tree::{Children, TreeAction, TreeItem, TreeState};
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
    tree: TreeState,
    /// Per-load derived lookups the grid renderer reads each frame. Computed
    /// once at load so the hot render path never rebuilds them.
    sprint_dates: std::collections::BTreeMap<i64, (i64, i64)>,
    item_sprints: std::collections::BTreeMap<String, Vec<i64>>,
    epic_stats: std::collections::BTreeMap<String, crate::domain::models::TimelineEpicStats>,
    /// Desired left cell of the horizontal viewport, before clamping to the
    /// canvas width (known only at render). Adjusted by scroll keys.
    h_offset: Cell<i32>,
    /// Whether the initial centre-on-today offset has been applied yet.
    h_centered: Cell<bool>,
    /// Glides the rendered horizontal offset toward `h_offset`.
    h_scroll: ScrollAnimator,
}

impl Default for TimelineState {
    fn default() -> Self {
        Self {
            data: None,
            error: None,
            loaded: false,
            pending_request_id: None,
            tree: TreeState::new(Vec::new()),
            sprint_dates: BTreeMap::new(),
            item_sprints: BTreeMap::new(),
            epic_stats: BTreeMap::new(),
            h_offset: Cell::new(0),
            h_centered: Cell::new(false),
            h_scroll: ScrollAnimator::new(),
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

    pub fn sprint_dates(&self) -> &std::collections::BTreeMap<i64, (i64, i64)> {
        &self.sprint_dates
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
                self.tree = TreeState::new(tree_items(&data));
                self.sprint_dates = sprint_date_map(&data);
                self.item_sprints = item_sprint_map(&data);
                self.epic_stats = epic_stat_map(&data);
                self.data = Some(data);
                self.error = None;
            }
            Err(message) => self.error = Some(message),
        }
    }

    pub(crate) fn dispatch_tree(&mut self, action: TreeAction) {
        // The whole timeline is loaded up front, so any child-load request the
        // tree emits has nothing to fetch and is safely ignored.
        let _ = self.tree.dispatch(action);
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
    }

    /// Resolves the horizontal offset to render at for this frame's geometry.
    /// First frame for fresh data centres `today_x`; afterwards it clamps the
    /// user's offset to the canvas and glides toward it. Interior-mutable, so it
    /// must run once per frame before reading the animator.
    pub fn resolve_h_offset(&self, viewport: u16, total_width: u16, today_x: i32) -> u16 {
        let max = i32::from(total_width.saturating_sub(viewport));
        let target = if self.h_centered.get() {
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
        self.h_scroll.set_target(f64::from(target));
        (self.h_scroll.current().round() as i32).clamp(0, max) as u16
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

fn epic_stat_map(
    data: &TimelineData,
) -> BTreeMap<String, crate::domain::models::TimelineEpicStats> {
    data.epics
        .iter()
        .map(|epic| (epic.key.clone(), epic.stats))
        .collect()
}

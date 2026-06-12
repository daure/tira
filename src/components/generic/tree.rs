use std::collections::{BTreeMap, HashMap, HashSet};

use super::scroll_animator::ScrollAnimator;
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};

const HALF_PAGE_STEP: isize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeAction {
    MoveUp,
    MoveDown,
    HalfPageUp,
    HalfPageDown,
    Collapse,
    Expand,
    ToggleExpanded,
    CollapseAll,
    ExpandAll,
    GoToStart,
    GoToEnd,
    GotoPrefix,
}

/// Emitted by the tree when an interaction needs work the tree cannot perform
/// itself, such as fetching children that have not been loaded yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeEvent {
    /// The item with this id was expanded but its children are not loaded.
    LoadChildren(String),
}

/// Whether an item's children are known to exist and whether they are loaded.
///
/// `Unknown` defers to in-memory children (the generic tree behavior used by
/// non-lazy callers and tests). The other variants are set explicitly by lazy
/// callers that probe child presence up front and fetch children on demand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Children {
    /// Expandability is derived from children present in the item set. Used for
    /// items with no known children (childless roots, leaves, search results).
    #[default]
    Unknown,
    /// Known to have children that have not been fetched yet.
    NotLoaded,
    /// A child fetch is in flight.
    Loading,
    /// Children have been fetched into the item set.
    Loaded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeItem {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub status: String,
    pub parent_id: Option<String>,
    pub root_order: u8,
    pub children: Children,
    pub field_values: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeRow {
    pub item_index: usize,
    pub depth: usize,
    pub expandable: bool,
    pub expanded: bool,
    pub loading: bool,
    /// True when this row sits under a node whose children are being refreshed
    /// in place (a soft reload). The stale rows stay visible but are rendered
    /// greyed out until the fresh children arrive.
    pub reloading: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct TreeState {
    items: Vec<TreeItem>,
    expanded_item_ids: HashSet<String>,
    selected_row: usize,
    selected_item_id: Option<String>,
    scroll: usize,
    /// The offset actually rendered by the most recent `visible_range` call
    /// (centered on the selection against the live viewport). `scroll` is the
    /// pre-render hint; this is the authoritative value `scroll_offset` reports.
    rendered_scroll: std::cell::Cell<usize>,
    manual_scroll: bool,
    pending_goto_prefix: bool,
    scroll_animator: ScrollAnimator,
    /// When set, rows are rendered as a flat list with no hierarchy or
    /// expansion (used for server-side search results).
    flat: bool,
}

impl TreeState {
    pub fn new(items: Vec<TreeItem>) -> Self {
        let selected_item_id = items.first().map(|item| item.id.clone());
        Self {
            items,
            expanded_item_ids: HashSet::new(),
            selected_row: 0,
            selected_item_id,
            scroll: 0,
            rendered_scroll: std::cell::Cell::new(0),
            manual_scroll: false,
            pending_goto_prefix: false,
            scroll_animator: ScrollAnimator::new(),
            flat: false,
        }
    }

    pub fn items(&self) -> &[TreeItem] {
        &self.items
    }

    pub fn update_item_field(&mut self, item_id: &str, field_id: &str, value: Option<String>) {
        let Some(item) = self.items.iter_mut().find(|item| item.id == item_id) else {
            return;
        };
        match value {
            Some(value) => {
                item.field_values.insert(field_id.to_owned(), value);
            }
            None => {
                item.field_values.remove(field_id);
            }
        }
    }

    pub fn update_item_status(&mut self, item_id: &str, status: String, status_id: Option<String>) {
        let Some(item) = self.items.iter_mut().find(|item| item.id == item_id) else {
            return;
        };
        item.status = status;
        match status_id {
            Some(id) => {
                item.field_values.insert(String::from("status_id"), id);
            }
            None => {
                item.field_values.remove("status_id");
            }
        }
    }

    /// Replaces the item set. `preserve_expanded` is the set of item ids whose
    /// expansion should be kept across the replacement; any other expansion is
    /// dropped. A re-appearing parent that should stay expanded keeps its id in
    /// `expanded_item_ids`, but its children must be re-fetched by the caller
    /// (see [`Self::nodes_needing_child_reload`]). Selection is preserved by id.
    pub fn set_items(&mut self, items: Vec<TreeItem>, preserve_expanded: &HashSet<String>) {
        let previous_selection = self.selected_item_id.clone();
        self.items = items;
        let present = self
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<HashSet<_>>();
        self.expanded_item_ids
            .retain(|id| preserve_expanded.contains(id) && present.contains(id.as_str()));
        // Whether the prior selection survives the swap. If it does, keep the
        // viewport where it is and let `visible_range` glide to the recentered
        // target; only a genuinely fresh item set (selection gone) snaps back to
        // the top.
        let selection_preserved = previous_selection
            .as_ref()
            .is_some_and(|id| self.items.iter().any(|item| item.id == *id));
        self.selected_item_id = if selection_preserved {
            previous_selection
        } else {
            self.items.first().map(|item| item.id.clone())
        };
        self.selected_row = 0;
        self.pending_goto_prefix = false;
        if !selection_preserved {
            self.scroll = 0;
            self.scroll_animator.snap_to(0.0);
        }
        self.clamp_selection();
    }

    pub fn set_flat(&mut self, flat: bool) {
        self.flat = flat;
    }

    /// Appends fetched children of `parent_id` to the item set and marks the
    /// parent as loaded. Preserves the current selection by id.
    pub fn add_children(&mut self, parent_id: &str, children: Vec<TreeItem>) {
        let Some(parent_index) = self.items.iter().position(|item| item.id == parent_id) else {
            return;
        };
        let existing = self
            .items
            .iter()
            .map(|item| item.id.clone())
            .collect::<HashSet<_>>();
        for child in children {
            if !existing.contains(&child.id) {
                self.items.push(child);
            }
        }
        self.items[parent_index].children = Children::Loaded;
        self.clamp_selection();
    }

    /// Appends more top-level items (e.g. a later page of roots) without
    /// resetting selection or scroll. Duplicate ids are skipped.
    pub fn append_items(&mut self, items: Vec<TreeItem>) {
        let mut existing = self
            .items
            .iter()
            .map(|item| item.id.clone())
            .collect::<HashSet<_>>();
        for item in items {
            if existing.insert(item.id.clone()) {
                self.items.push(item);
            }
        }
        self.clamp_selection();
    }

    /// Marks a previously-requested child load as failed so the node can be
    /// retried rather than spinning forever.
    pub fn mark_children_failed(&mut self, parent_id: &str) {
        if let Some(item) = self.items.iter_mut().find(|item| item.id == parent_id) {
            item.children = Children::NotLoaded;
        }
        self.expanded_item_ids.remove(parent_id);
        self.clamp_selection();
    }

    /// Drops every descendant of `parent_id` and marks the node `Loading` so a
    /// fresh child fetch can repopulate it. The node stays expanded. Returns
    /// `true` if the node exists and can be reloaded (i.e. it can have
    /// children); `false` for leaves or unknown ids.
    pub fn reload_children(&mut self, parent_id: &str) -> bool {
        let Some(parent_index) = self.items.iter().position(|item| item.id == parent_id) else {
            return false;
        };
        if matches!(self.items[parent_index].children, Children::Unknown) {
            // A node with no known children (leaf, or in-memory-only hierarchy)
            // has nothing to refetch from the server.
            return false;
        }

        let descendants = self.descendant_ids(parent_id);
        if !descendants.is_empty() {
            self.items
                .retain(|item| item.id == parent_id || !descendants.contains(item.id.as_str()));
            self.expanded_item_ids
                .retain(|id| !descendants.contains(id.as_str()));
        }
        // Re-resolve the index: retain may have shifted it.
        if let Some(item) = self.items.iter_mut().find(|item| item.id == parent_id) {
            item.children = Children::Loading;
        }
        self.expanded_item_ids.insert(parent_id.to_owned());
        self.clamp_selection();
        true
    }

    /// Merges a page of root items into the tree in place without tearing it
    /// down: matching roots are updated (display fields only, keeping any loaded
    /// subtree and expansion) and brand-new roots are appended. Pruning of
    /// vanished roots is deferred to [`Self::retain_roots`] once every page has
    /// arrived, so paginated reloads don't transiently drop later-page roots.
    /// Selection, scroll, and expansion are left untouched.
    pub fn merge_root_items(&mut self, incoming: Vec<TreeItem>) {
        for root in incoming {
            self.merge_item(root);
        }
        self.clamp_selection();
    }

    /// Removes root items (and their descendants) whose ids are not in `keep`.
    /// Used to finish a paginated reload by pruning issues deleted server-side.
    pub fn retain_roots(&mut self, keep: &HashSet<String>) {
        let index_by_id = self.index_by_id();
        let current_root_ids: Vec<String> = self
            .items
            .iter()
            .filter(|item| {
                item.parent_id
                    .as_deref()
                    .is_none_or(|parent| !index_by_id.contains_key(parent))
            })
            .map(|item| item.id.clone())
            .collect();
        drop(index_by_id);

        let mut remove: HashSet<String> = HashSet::new();
        for root_id in &current_root_ids {
            if !keep.contains(root_id) {
                remove.insert(root_id.clone());
                remove.extend(self.descendant_ids(root_id));
            }
        }
        if !remove.is_empty() {
            self.items.retain(|item| !remove.contains(&item.id));
            self.expanded_item_ids.retain(|id| !remove.contains(id));
            self.clamp_selection();
        }
    }

    /// Updates the display fields of an existing item in place (preserving any
    /// loaded subtree and `Loading`/`Loaded` state), or inserts it when new.
    fn merge_item(&mut self, incoming: TreeItem) {
        if let Some(existing) = self.items.iter_mut().find(|item| item.id == incoming.id) {
            existing.label = incoming.label;
            existing.kind = incoming.kind;
            existing.status = incoming.status;
            existing.root_order = incoming.root_order;
            existing.parent_id = incoming.parent_id;
            existing.field_values = incoming.field_values;
            // Keep a subtree we already hold; only adopt the incoming hint when
            // we have nothing loaded for this node yet.
            if !matches!(existing.children, Children::Loaded | Children::Loading) {
                existing.children = incoming.children;
            }
        } else {
            self.items.push(incoming);
        }
    }

    /// Marks every present, expanded node in `expanded` that already holds
    /// children as `Loading` *without* dropping its subtree, so the stale rows
    /// stay visible (greyed) while fresh children are fetched. Returns the ids
    /// to refetch. Unlike [`Self::reload_children`], the existing descendants
    /// are retained until [`Self::replace_children`] swaps them in.
    pub fn begin_soft_reload(&mut self, expanded: &HashSet<String>) -> Vec<String> {
        let children = self.children_by_parent();
        let has_children: HashSet<String> =
            children.keys().map(|parent| (*parent).to_owned()).collect();
        drop(children);

        let mut to_fetch = Vec::new();
        for index in 0..self.items.len() {
            let id = self.items[index].id.clone();
            if !expanded.contains(&id) {
                continue;
            }
            // Only refresh nodes that already hold a loaded subtree: the stale
            // children stay on screen (greyed) until the fresh set arrives.
            if self.items[index].children == Children::Loaded && has_children.contains(&id) {
                self.items[index].children = Children::Loading;
                self.expanded_item_ids.insert(id.clone());
                to_fetch.push(id);
            }
        }
        to_fetch
    }

    /// Reverts a soft-reloading node to `Loaded` without disturbing its stale
    /// subtree. Used when a background child refresh fails so the existing rows
    /// remain instead of collapsing.
    pub fn mark_children_loaded(&mut self, parent_id: &str) {
        if let Some(item) = self.items.iter_mut().find(|item| item.id == parent_id) {
            item.children = Children::Loaded;
        }
    }

    /// Swaps in a freshly-fetched set of direct children for `parent_id`,
    /// merging in place: existing children keep their loaded subtrees and are
    /// updated, new children are appended, and children no longer present are
    /// removed with their descendants. Marks the parent `Loaded`.
    pub fn replace_children(&mut self, parent_id: &str, incoming: Vec<TreeItem>) {
        if !self.items.iter().any(|item| item.id == parent_id) {
            return;
        }
        let incoming_ids: HashSet<String> = incoming.iter().map(|item| item.id.clone()).collect();
        let current_direct: Vec<String> = self
            .items
            .iter()
            .filter(|item| item.parent_id.as_deref() == Some(parent_id))
            .map(|item| item.id.clone())
            .collect();

        let mut remove: HashSet<String> = HashSet::new();
        for child_id in &current_direct {
            if !incoming_ids.contains(child_id) {
                remove.insert(child_id.clone());
                remove.extend(self.descendant_ids(child_id));
            }
        }
        if !remove.is_empty() {
            self.items.retain(|item| !remove.contains(&item.id));
            self.expanded_item_ids.retain(|id| !remove.contains(id));
        }

        for child in incoming {
            self.merge_item(child);
        }
        if let Some(item) = self.items.iter_mut().find(|item| item.id == parent_id) {
            item.children = Children::Loaded;
        }
        self.clamp_selection();
    }

    /// Collects the ids of every transitive child of `parent_id` currently in
    /// the item set.
    fn descendant_ids(&self, parent_id: &str) -> HashSet<String> {
        let children = self.children_by_parent();
        let mut descendants = HashSet::new();
        let mut stack = vec![parent_id];
        while let Some(current) = stack.pop() {
            if let Some(child_indices) = children.get(current) {
                for &child_index in child_indices {
                    let child_id = self.items[child_index].id.as_str();
                    if descendants.insert(child_id.to_owned()) {
                        stack.push(child_id);
                    }
                }
            }
        }
        descendants
    }

    /// The ids currently marked expanded.
    pub fn expanded_item_ids(&self) -> &HashSet<String> {
        &self.expanded_item_ids
    }

    /// The expanded ids strictly below `parent_id` (its transitive descendants),
    /// used to capture which child subtrees to restore on a node reload.
    pub fn expanded_descendant_ids(&self, parent_id: &str) -> HashSet<String> {
        let descendants = self.descendant_ids(parent_id);
        self.expanded_item_ids
            .iter()
            .filter(|id| descendants.contains(id.as_str()))
            .cloned()
            .collect()
    }

    /// Whether an item with `id` is present in the set.
    pub fn contains_item(&self, id: &str) -> bool {
        self.items.iter().any(|item| item.id == id)
    }

    /// For each id in `restore` that is present and has unfetched children
    /// (`NotLoaded`), marks it expanded and `Loading`, and returns those ids so
    /// the caller can fire the child fetches. Ids already loaded or in flight
    /// are skipped. This is the per-step engine of expansion restoration: it is
    /// called after the initial reload and again after each child batch arrives,
    /// walking the tree downward as nested nodes reappear.
    pub fn nodes_needing_child_reload(&mut self, restore: &HashSet<String>) -> Vec<String> {
        let mut to_fetch = Vec::new();
        for index in 0..self.items.len() {
            let item = &self.items[index];
            if !restore.contains(&item.id) {
                continue;
            }
            if item.children == Children::NotLoaded {
                let id = item.id.clone();
                self.items[index].children = Children::Loading;
                self.expanded_item_ids.insert(id.clone());
                to_fetch.push(id);
            } else if item.children == Children::Loaded {
                // Already populated (e.g. a shallow node): just keep it open.
                self.expanded_item_ids.insert(item.id.clone());
            }
        }
        if !to_fetch.is_empty() {
            self.clamp_selection();
        }
        to_fetch
    }

    pub fn selected_item_id(&self) -> Option<&str> {
        self.selected_item_id.as_deref()
    }

    /// The id of the top-level ancestor of the currently-selected item: walk
    /// `parent_id` links upward while the parent is present in the item set.
    /// Returns the selection itself when it is already a root (or has no loaded
    /// parent), and `None` when there is no selection.
    pub fn selected_root_ancestor_id(&self) -> Option<String> {
        let index_by_id = self.index_by_id();
        let mut current = self.selected_item_id.as_deref()?;
        while let Some(&index) = index_by_id.get(current) {
            match self.items[index].parent_id.as_deref() {
                Some(parent) if index_by_id.contains_key(parent) => current = parent,
                _ => break,
            }
        }
        Some(current.to_owned())
    }

    /// Selects the item with `id` if present, preserving it across a reload.
    pub fn select_item_by_id(&mut self, id: &str) {
        if self.items.iter().any(|item| item.id == id) {
            self.selected_item_id = Some(id.to_owned());
            self.clamp_selection();
        }
    }

    pub fn selected_row(&self) -> usize {
        self.selected_row
    }

    pub fn select_row(&mut self, row: usize) {
        let rows = self.rows();
        if rows.is_empty() {
            self.selected_row = 0;
            self.selected_item_id = None;
            return;
        }
        self.selected_row = row.min(rows.len() - 1);
        self.sync_selected_item_id(&rows);
        // `visible_range` re-centers against the real viewport height; clearing
        // manual scroll re-enables auto-centering and lets the change glide.
        self.manual_scroll = false;
    }

    pub fn scroll_offset(&self) -> usize {
        self.rendered_scroll.get()
    }
    pub fn scroll_viewport(&mut self, delta: isize, height: usize) {
        let row_count = self.rows().len();
        if row_count == 0 || height == 0 {
            self.scroll = 0;
            self.rendered_scroll.set(0);
            self.manual_scroll = false;
            self.scroll_animator.snap_to(0.0);
            return;
        }
        let viewport = height.min(row_count);
        let max_scroll = row_count.saturating_sub(viewport);
        // Seed the delta from the offset currently on screen (which, while
        // auto-centering, lives in `rendered_scroll` rather than `scroll`).
        // Reading the stale `scroll` here would snap the viewport back to the
        // top on the first wheel tick before drifting back to the selection.
        let base = self.rendered_scroll.get().min(max_scroll);
        self.scroll = base.saturating_add_signed(delta).min(max_scroll);
        self.rendered_scroll.set(self.scroll);
        self.manual_scroll = true;
        self.scroll_animator.snap_to(self.scroll as f64);
    }

    pub fn tick(&mut self, dt: std::time::Duration) {
        self.scroll_animator.tick(dt);
    }

    pub fn is_animating(&self) -> bool {
        self.scroll_animator.is_animating()
    }

    /// Whether any node currently has a child fetch in flight.
    pub fn any_loading(&self) -> bool {
        self.items
            .iter()
            .any(|item| item.children == Children::Loading)
    }

    pub fn rows(&self) -> Vec<TreeRow> {
        self.build_rows()
    }

    pub fn visible_range(&self, height: usize) -> std::ops::Range<usize> {
        let row_count = self.rows().len();
        if row_count == 0 || height == 0 {
            return 0..0;
        }

        let viewport = height.min(row_count);
        let max_scroll = row_count.saturating_sub(viewport);
        let selected = self.selected_row.min(row_count - 1);
        let mut offset = self.scroll.min(max_scroll);
        if !self.manual_scroll {
            // Keep the selection vertically centered in the viewport (clamped at
            // the list ends), so j/k navigation reads from the middle rather
            // than drifting toward the top.
            offset = selected.saturating_sub(viewport / 2).min(max_scroll);
        }

        // Record the settled (non-animated) target so `scroll_offset` reports
        // where the viewport is heading, independent of glide progress.
        self.rendered_scroll.set(offset);
        self.scroll_animator.set_target(offset as f64);
        let animated_offset = self.scroll_animator.current().round() as usize;
        let animated_offset = animated_offset.min(max_scroll);
        let end = (animated_offset + viewport).min(row_count);
        animated_offset..end
    }

    pub fn dispatch(&mut self, action: TreeAction) -> Option<TreeEvent> {
        match action {
            TreeAction::MoveUp => self.move_by(-1),
            TreeAction::MoveDown => self.move_by(1),
            TreeAction::HalfPageUp => self.move_by(-HALF_PAGE_STEP),
            TreeAction::HalfPageDown => self.move_by(HALF_PAGE_STEP),
            TreeAction::Collapse => self.collapse_or_go_to_parent(),
            TreeAction::Expand => return self.expand_or_go_to_first_child(),
            TreeAction::ToggleExpanded => return self.toggle_selected_expansion(),
            TreeAction::CollapseAll => self.collapse_all_expansion(),
            TreeAction::ExpandAll => self.expand_all_loaded_expansion(),
            TreeAction::GoToStart => self.go_to_start(),
            TreeAction::GoToEnd => self.go_to_end(),
            TreeAction::GotoPrefix => self.handle_goto_prefix(),
        }
        None
    }

    pub fn clear_pending_prefix(&mut self) {
        self.pending_goto_prefix = false;
    }

    pub fn clamp_selection(&mut self) {
        let rows = self.rows();
        if rows.is_empty() {
            self.selected_row = 0;
            self.selected_item_id = None;
            self.scroll = 0;
            self.manual_scroll = false;
            self.scroll_animator.snap_to(0.0);
            return;
        }

        if let Some(selected_id) = self.selected_item_id.as_deref()
            && let Some(row_index) = self.row_index_for_item_id(&rows, selected_id)
        {
            self.selected_row = row_index;
        } else {
            self.selected_row = self.selected_row.min(rows.len() - 1);
            self.sync_selected_item_id(&rows);
        }
        // Sync the manual hint to where the viewport is actually parked and
        // hand control back to auto-centering. Don't snap the animator here:
        // its `current` already holds the on-screen offset, so leaving it lets
        // `visible_range` set the new centered target and glide there. Snapping
        // to the stale `scroll` would jerk the viewport to the top first.
        self.scroll = self.rendered_scroll.get().min(rows.len() - 1);
        self.manual_scroll = false;
    }

    fn move_by(&mut self, delta: isize) {
        self.pending_goto_prefix = false;
        let rows = self.rows();
        if rows.is_empty() {
            self.selected_row = 0;
            self.selected_item_id = None;
            self.scroll = 0;
            return;
        }

        let max_index = rows.len() - 1;
        self.selected_row = self
            .selected_row
            .saturating_add_signed(delta)
            .min(max_index);
        self.sync_selected_item_id(&rows);
        // Leave `scroll` for `visible_range` to recompute (centered) against the
        // real viewport height; clearing manual scroll re-enables auto-centering.
        self.manual_scroll = false;
    }

    fn collapse_or_go_to_parent(&mut self) {
        self.pending_goto_prefix = false;
        let rows = self.rows();
        let Some(row) = rows.get(self.selected_row) else {
            return;
        };
        let selected_id = self.items[row.item_index].id.as_str();

        if row.expandable && row.expanded {
            // A node whose subtree is mid-refresh must not be collapsed: the
            // stale children are still shown (greyed) until the fresh set lands.
            if self.is_soft_reloading(row.item_index) {
                return;
            }
            self.expanded_item_ids.remove(selected_id);
            self.clamp_selection();
            return;
        }

        if let Some(parent_row_index) = self.parent_row_index(&rows, row.depth) {
            self.selected_row = parent_row_index;
            self.sync_selected_item_id(&rows);
        } else {
            self.go_to_start();
        }
    }

    fn expand_or_go_to_first_child(&mut self) -> Option<TreeEvent> {
        self.pending_goto_prefix = false;
        let rows = self.rows();
        let row = rows.get(self.selected_row)?;
        if !row.expandable {
            return None;
        }

        let item_index = row.item_index;
        let depth = row.depth;
        let selected_id = self.items[item_index].id.clone();
        if !self.expanded_item_ids.contains(&selected_id) {
            self.expanded_item_ids.insert(selected_id.clone());
            return self.begin_child_load(item_index);
        }

        if rows
            .get(self.selected_row + 1)
            .is_some_and(|next| next.depth > depth)
        {
            self.selected_row += 1;
            self.sync_selected_item_id(&rows);
        }
        None
    }

    fn toggle_selected_expansion(&mut self) -> Option<TreeEvent> {
        self.pending_goto_prefix = false;
        let rows = self.rows();
        let row = rows.get(self.selected_row)?;
        if !row.expandable {
            return None;
        }

        let item_index = row.item_index;
        let id = self.items[item_index].id.clone();
        // Block toggling while this node's children are being refreshed.
        if self.is_soft_reloading(item_index) {
            return None;
        }
        if self.expanded_item_ids.remove(&id) {
            self.clamp_selection();
            return None;
        }

        self.expanded_item_ids.insert(id);
        let event = self.begin_child_load(item_index);
        self.clamp_selection();
        event
    }

    /// If the item's children are known but not loaded, marks it loading and
    /// returns a load request. Otherwise no work is needed.
    fn begin_child_load(&mut self, item_index: usize) -> Option<TreeEvent> {
        if self.items[item_index].children == Children::NotLoaded {
            self.items[item_index].children = Children::Loading;
            return Some(TreeEvent::LoadChildren(self.items[item_index].id.clone()));
        }
        None
    }

    fn collapse_all_expansion(&mut self) {
        self.pending_goto_prefix = false;
        self.expanded_item_ids.clear();
        self.clamp_selection();
    }

    fn expand_all_loaded_expansion(&mut self) {
        self.pending_goto_prefix = false;
        let children = self.children_by_parent();
        let expandable_ids = self
            .items
            .iter()
            .filter_map(|item| {
                let has_loaded_children = children
                    .get(item.id.as_str())
                    .is_some_and(|children| !children.is_empty());
                (self.is_expandable(item, has_loaded_children) && has_loaded_children)
                    .then(|| item.id.clone())
            })
            .collect::<Vec<_>>();
        self.expanded_item_ids.extend(expandable_ids);
        self.clamp_selection();
    }

    fn go_to_start(&mut self) {
        self.pending_goto_prefix = false;
        self.selected_row = 0;
        self.scroll = 0;
        self.manual_scroll = false;
        let rows = self.rows();
        self.sync_selected_item_id(&rows);
    }

    fn go_to_end(&mut self) {
        self.pending_goto_prefix = false;
        let rows = self.rows();
        if rows.is_empty() {
            self.selected_row = 0;
            self.selected_item_id = None;
            self.scroll = 0;
        } else {
            self.selected_row = rows.len() - 1;
            self.sync_selected_item_id(&rows);
        }
        // `visible_range` re-centers (clamped to the bottom) against the real
        // viewport height.
        self.manual_scroll = false;
    }

    fn handle_goto_prefix(&mut self) {
        if self.pending_goto_prefix {
            self.go_to_start();
        } else {
            self.pending_goto_prefix = true;
        }
    }

    fn parent_row_index(&self, rows: &[TreeRow], selected_depth: usize) -> Option<usize> {
        if selected_depth == 0 || self.selected_row == 0 {
            return None;
        }

        rows[..self.selected_row]
            .iter()
            .rposition(|candidate| candidate.depth + 1 == selected_depth)
    }

    fn row_index_for_item_id(&self, rows: &[TreeRow], item_id: &str) -> Option<usize> {
        rows.iter()
            .position(|row| self.items[row.item_index].id == item_id)
    }

    fn sync_selected_item_id(&mut self, rows: &[TreeRow]) {
        self.selected_item_id = rows
            .get(self.selected_row)
            .map(|row| self.items[row.item_index].id.clone());
    }

    fn build_rows(&self) -> Vec<TreeRow> {
        if self.flat {
            return (0..self.items.len())
                .map(|item_index| TreeRow {
                    item_index,
                    depth: 0,
                    expandable: false,
                    expanded: false,
                    loading: false,
                    reloading: false,
                })
                .collect();
        }

        let children = self.children_by_parent();
        let index_by_id = self.index_by_id();
        let roots = self.root_item_indices(&index_by_id);
        let mut rows = Vec::new();

        for root in roots {
            self.push_item_row(root, 0, &children, &mut rows, false);
        }

        rows
    }

    fn push_item_row(
        &self,
        item_index: usize,
        depth: usize,
        children: &HashMap<&str, Vec<usize>>,
        rows: &mut Vec<TreeRow>,
        reloading: bool,
    ) {
        let item = &self.items[item_index];
        let child_indices = children.get(item.id.as_str());
        let has_loaded_children = child_indices.is_some_and(|children| !children.is_empty());
        let expandable = self.is_expandable(item, has_loaded_children);
        let expanded = expandable && self.expanded_item_ids.contains(&item.id);
        // A node whose children are present but marked `Loading` is being
        // refreshed in place: keep showing the stale subtree, greyed out.
        let soft_reloading = item.children == Children::Loading && has_loaded_children;
        rows.push(TreeRow {
            item_index,
            depth,
            expandable,
            expanded,
            loading: item.children == Children::Loading,
            reloading: reloading || soft_reloading,
        });

        if expanded && let Some(child_indices) = child_indices {
            for child_index in child_indices {
                self.push_item_row(
                    *child_index,
                    depth + 1,
                    children,
                    rows,
                    reloading || soft_reloading,
                );
            }
        }
    }

    fn is_expandable(&self, item: &TreeItem, has_loaded_children: bool) -> bool {
        match item.children {
            Children::Unknown | Children::Loaded => has_loaded_children,
            Children::NotLoaded | Children::Loading => true,
        }
    }

    /// Whether the item at `item_index` is mid soft-reload: marked `Loading`
    /// while still holding the previously-loaded children that stay on screen.
    fn is_soft_reloading(&self, item_index: usize) -> bool {
        let item = &self.items[item_index];
        item.children == Children::Loading
            && self
                .items
                .iter()
                .any(|other| other.parent_id.as_deref() == Some(item.id.as_str()))
    }

    fn root_item_indices(&self, index_by_id: &HashMap<&str, usize>) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                let parent_in_result = item
                    .parent_id
                    .as_deref()
                    .is_some_and(|parent| index_by_id.contains_key(parent));
                (!parent_in_result).then_some(index)
            })
            .collect()
    }

    fn children_by_parent(&self) -> HashMap<&str, Vec<usize>> {
        let index_by_id = self.index_by_id();
        let mut children: HashMap<&str, Vec<usize>> = HashMap::new();
        for (index, item) in self.items.iter().enumerate() {
            if let Some(parent_id) = item.parent_id.as_deref()
                && index_by_id.contains_key(parent_id)
            {
                children.entry(parent_id).or_default().push(index);
            }
        }
        children
    }

    fn index_by_id(&self) -> HashMap<&str, usize> {
        self.items
            .iter()
            .enumerate()
            .map(|(index, item)| (item.id.as_str(), index))
            .collect()
    }
}

pub fn fuzzy_indices(text: &str, filter: &str) -> Vec<usize> {
    if filter.is_empty() {
        return Vec::new();
    }

    SkimMatcherV2::default()
        .smart_case()
        .fuzzy_indices(text, filter)
        .map_or_else(Vec::new, |(_, indices)| indices)
}

pub fn fuzzy_matches(text: &str, filter: &str) -> bool {
    filter.is_empty()
        || SkimMatcherV2::default()
            .smart_case()
            .fuzzy_match(text, filter)
            .is_some()
}

#[cfg(test)]
mod tests {
    use super::{Children, TreeAction, TreeEvent, TreeItem, TreeState};
    use std::collections::HashSet;

    #[test]
    fn rows_preserve_loaded_root_order_and_indent_children() {
        let tree = TreeState::new(vec![
            item("EPIC", "Parent", "Epic", None, 1),
            item("ONE", "Unparented", "Task", None, 0),
            item("STORY", "Child", "Story", Some("EPIC"), 0),
        ]);

        let rows = tree.rows();

        assert_eq!(tree.items()[rows[0].item_index].id, "EPIC");
        assert_eq!(tree.items()[rows[1].item_index].id, "ONE");
        assert_eq!(rows.len(), 2);
        assert!(rows[0].expandable);
    }

    #[test]
    fn expand_moves_to_first_child_when_already_expanded() {
        let mut tree = TreeState::new(vec![
            item("EPIC", "Parent", "Epic", None, 0),
            item("STORY", "Child", "Story", Some("EPIC"), 0),
        ]);

        tree.dispatch(TreeAction::Expand);
        assert_eq!(tree.rows().len(), 2);
        assert_eq!(tree.selected_row(), 0);

        tree.dispatch(TreeAction::Expand);
        assert_eq!(tree.selected_row(), 1);
    }

    #[test]
    fn collapse_moves_to_parent_when_already_collapsed() {
        let mut tree = TreeState::new(vec![
            item("EPIC", "Parent", "Epic", None, 0),
            item("STORY", "Child", "Story", Some("EPIC"), 0),
        ]);

        tree.dispatch(TreeAction::Expand);
        tree.dispatch(TreeAction::Expand);
        tree.dispatch(TreeAction::Collapse);

        assert_eq!(tree.selected_row(), 0);
    }

    #[test]
    fn g_g_and_uppercase_g_jump_to_start_and_end() {
        let mut tree = TreeState::new(vec![
            item("ONE", "One", "Task", None, 0),
            item("TWO", "Two", "Task", None, 0),
            item("THREE", "Three", "Task", None, 0),
        ]);

        tree.dispatch(TreeAction::GoToEnd);
        assert_eq!(tree.selected_row(), 2);

        tree.dispatch(TreeAction::GotoPrefix);
        tree.dispatch(TreeAction::GotoPrefix);
        assert_eq!(tree.selected_row(), 0);
    }

    #[test]
    fn not_loaded_children_make_a_row_expandable_and_request_a_load() {
        let mut root = item("EPIC", "Parent", "Epic", None, 0);
        root.children = Children::NotLoaded;
        let mut tree = TreeState::new(vec![root]);

        assert!(tree.rows()[0].expandable);
        assert!(!tree.rows()[0].expanded);

        let event = tree.dispatch(TreeAction::ToggleExpanded);
        assert_eq!(event, Some(TreeEvent::LoadChildren(String::from("EPIC"))));
        assert!(tree.rows()[0].loading);
    }

    #[test]
    fn add_children_splices_under_parent_and_marks_loaded() {
        let mut root = item("EPIC", "Parent", "Epic", None, 0);
        root.children = Children::NotLoaded;
        let mut tree = TreeState::new(vec![root]);

        tree.dispatch(TreeAction::ToggleExpanded);
        tree.add_children(
            "EPIC",
            vec![item("STORY", "Child", "Story", Some("EPIC"), 0)],
        );

        let rows = tree.rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(tree.items()[rows[1].item_index].id, "STORY");
        assert_eq!(rows[1].depth, 1);
        assert!(!rows[0].loading);
    }

    #[test]
    fn reload_children_drops_subtree_marks_loading_and_keeps_expanded() {
        let mut root = item("EPIC", "Parent", "Epic", None, 0);
        root.children = Children::NotLoaded;
        let mut tree = TreeState::new(vec![root]);
        tree.dispatch(TreeAction::ToggleExpanded);
        tree.add_children(
            "EPIC",
            vec![item("STORY", "Child", "Story", Some("EPIC"), 0)],
        );
        assert_eq!(tree.rows().len(), 2);

        let reloaded = tree.reload_children("EPIC");

        assert!(reloaded);
        assert!(tree.any_loading());
        let rows = tree.rows();
        // Child dropped, parent still present, expanded, and showing the spinner.
        assert_eq!(rows.len(), 1);
        assert_eq!(tree.items()[rows[0].item_index].id, "EPIC");
        assert!(rows[0].loading);
        assert!(rows[0].expandable);
        assert_eq!(tree.items().len(), 1);
    }

    #[test]
    fn set_items_preserves_only_listed_expansion() {
        let mut a = item("A", "A", "Epic", None, 0);
        a.children = Children::NotLoaded;
        let mut b = item("B", "B", "Epic", None, 0);
        b.children = Children::NotLoaded;
        let mut tree = TreeState::new(vec![a.clone(), b.clone()]);
        // Expand both, then replace items preserving only A's expansion.
        tree.dispatch(TreeAction::ToggleExpanded); // A
        tree.dispatch(TreeAction::MoveDown);
        tree.dispatch(TreeAction::ToggleExpanded); // B

        let preserve: HashSet<String> = ["A".to_owned()].into_iter().collect();
        tree.set_items(vec![a, b], &preserve);

        assert!(tree.expanded_item_ids().contains("A"));
        assert!(!tree.expanded_item_ids().contains("B"));
    }

    #[test]
    fn nodes_needing_child_reload_marks_loading_and_returns_unloaded_targets() {
        let mut a = item("A", "A", "Epic", None, 0);
        a.children = Children::NotLoaded;
        let mut tree = TreeState::new(vec![a, item("LEAF", "Leaf", "Task", None, 0)]);

        let restore: HashSet<String> = ["A".to_owned(), "LEAF".to_owned(), "MISSING".to_owned()]
            .into_iter()
            .collect();
        let to_fetch = tree.nodes_needing_child_reload(&restore);

        // Only A (NotLoaded) is fetched; LEAF (Unknown) and MISSING are skipped.
        assert_eq!(to_fetch, vec![String::from("A")]);
        assert!(tree.any_loading());
        assert!(tree.expanded_item_ids().contains("A"));
    }

    #[test]
    fn expanded_descendant_ids_returns_open_nodes_below_parent() {
        let mut epic = item("EPIC", "Epic", "Epic", None, 0);
        epic.children = Children::NotLoaded;
        let mut tree = TreeState::new(vec![epic]);
        tree.dispatch(TreeAction::ToggleExpanded);
        let mut sub = item("SUB", "Sub", "Story", Some("EPIC"), 0);
        sub.children = Children::NotLoaded;
        tree.add_children("EPIC", vec![sub]);
        // Expand the nested node SUB.
        tree.dispatch(TreeAction::MoveDown);
        tree.dispatch(TreeAction::ToggleExpanded);

        let descendants = tree.expanded_descendant_ids("EPIC");
        assert!(descendants.contains("SUB"));
        assert!(!descendants.contains("EPIC"), "parent itself is excluded");
    }

    #[test]
    fn reload_children_is_noop_for_unknown_or_leaf_nodes() {
        // A node with no known children (Children::Unknown) cannot be reloaded.
        let mut tree = TreeState::new(vec![item("TASK", "Leaf", "Task", None, 0)]);
        assert!(!tree.reload_children("TASK"));
        assert!(!tree.any_loading());
        // An unknown id is also rejected.
        assert!(!tree.reload_children("MISSING"));
    }

    #[test]
    fn childless_unknown_row_without_loaded_children_is_not_expandable() {
        let root = item("EPIC", "Parent", "Epic", None, 0);
        let tree = TreeState::new(vec![root]);

        assert!(!tree.rows()[0].expandable);
    }

    #[test]
    fn flat_mode_lists_every_item_at_depth_zero_without_expansion() {
        let mut tree = TreeState::new(vec![
            item("EPIC", "Parent", "Epic", None, 0),
            item("STORY", "Child", "Story", Some("EPIC"), 0),
        ]);
        tree.set_flat(true);

        let rows = tree.rows();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row.depth == 0 && !row.expandable));
    }

    #[test]
    fn selection_survives_child_splice_by_id() {
        let mut root = item("EPIC", "Parent", "Epic", None, 0);
        root.children = Children::NotLoaded;
        let mut tree = TreeState::new(vec![root, item("OTHER", "Other", "Task", None, 0)]);

        tree.dispatch(TreeAction::MoveDown);
        assert_eq!(tree.selected_item_id(), Some("OTHER"));

        tree.add_children(
            "EPIC",
            vec![item("STORY", "Child", "Story", Some("EPIC"), 0)],
        );
        assert_eq!(tree.selected_item_id(), Some("OTHER"));
    }

    fn item(
        id: &str,
        label: &str,
        kind: &str,
        parent_id: Option<&str>,
        root_order: u8,
    ) -> TreeItem {
        TreeItem {
            id: id.to_owned(),
            label: label.to_owned(),
            status: String::from("To Do"),
            kind: kind.to_owned(),
            parent_id: parent_id.map(str::to_owned),
            root_order,
            children: Children::Unknown,
            field_values: std::collections::BTreeMap::new(),
        }
    }

    #[test]
    fn scroll_glides_smoothly_toward_target() {
        let mut items = Vec::new();
        for i in 0..50 {
            items.push(item(
                &format!("ITEM_{i}"),
                &format!("Item {i}"),
                "Task",
                None,
                0,
            ));
        }
        let mut tree = TreeState::new(items);

        assert!(!tree.is_animating());

        tree.dispatch(TreeAction::GoToEnd);
        let range = tree.visible_range(10);

        assert_eq!(tree.scroll_animator.target(), 40.0);
        assert!(tree.is_animating());
        assert_eq!(range.start, 0);

        tree.tick(std::time::Duration::from_millis(50));
        assert!(tree.is_animating());
        assert!(tree.scroll_animator.current() > 0.0);
        assert!(tree.scroll_animator.current() < 40.0);

        tree.tick(std::time::Duration::from_secs(2));
        assert!(!tree.is_animating());
        assert_eq!(tree.scroll_animator.current(), 40.0);

        let range_end = tree.visible_range(10);
        assert_eq!(range_end.start, 40);
    }

    #[test]
    fn root_rows_preserve_loaded_order_instead_of_type_sorting() {
        let tree = TreeState::new(vec![
            item("EPIC", "Epic Parent", "Epic", None, 0),
            item("STORY_WITH_CHILDREN", "Story Parent", "Story", None, 0),
            item("STORY_NO_CHILDREN", "Story Leaf", "Story", None, 0),
            item("BUG", "Bug Report", "Bug", None, 0),
            item("TASK", "Task Item", "Task", None, 0),
        ]);

        let rows = tree.rows();
        let root_ids: Vec<String> = rows
            .iter()
            .filter(|row| row.depth == 0)
            .map(|row| tree.items()[row.item_index].id.clone())
            .collect();

        assert_eq!(
            root_ids,
            vec![
                String::from("EPIC"),
                String::from("STORY_WITH_CHILDREN"),
                String::from("STORY_NO_CHILDREN"),
                String::from("BUG"),
                String::from("TASK")
            ]
        );
    }
}

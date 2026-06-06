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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeItem {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub status: String,
    pub parent_id: Option<String>,
    pub root_order: u8,
    pub field_values: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeRow {
    pub item_index: usize,
    pub depth: usize,
    pub expandable: bool,
    pub expanded: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct TreeState {
    items: Vec<TreeItem>,
    expanded_item_ids: HashSet<String>,
    selected_row: usize,
    scroll: usize,
    pending_goto_prefix: bool,
    scroll_animator: ScrollAnimator,
    searchable_field_ids: Option<Vec<String>>,
}

impl TreeState {
    pub fn new(items: Vec<TreeItem>) -> Self {
        Self {
            items,
            expanded_item_ids: HashSet::new(),
            selected_row: 0,
            scroll: 0,
            pending_goto_prefix: false,
            scroll_animator: ScrollAnimator::new(),
            searchable_field_ids: None,
        }
    }

    pub fn items(&self) -> &[TreeItem] {
        &self.items
    }

    pub fn set_items(&mut self, items: Vec<TreeItem>) {
        self.items = items;
        self.expanded_item_ids.clear();
        self.selected_row = 0;
        self.scroll = 0;
        self.pending_goto_prefix = false;
        self.scroll_animator.snap_to(0.0);
    }

    pub fn set_searchable_field_ids(&mut self, field_ids: Option<Vec<String>>) {
        self.searchable_field_ids = field_ids;
    }

    pub fn selected_row(&self) -> usize {
        self.selected_row
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll
    }

    pub fn tick(&mut self, dt: std::time::Duration) {
        self.scroll_animator.tick(dt);
    }

    pub fn is_animating(&self) -> bool {
        self.scroll_animator.is_animating()
    }

    pub fn rows(&self, filter: &str) -> Vec<TreeRow> {
        self.build_rows(filter)
    }

    pub fn visible_range(&self, filter: &str, height: usize) -> std::ops::Range<usize> {
        let row_count = self.rows(filter).len();
        if row_count == 0 || height == 0 {
            return 0..0;
        }

        let viewport = height.min(row_count);
        let max_scroll = row_count.saturating_sub(viewport);
        let selected = self.selected_row.min(row_count - 1);
        let mut offset = self.scroll.min(max_scroll);

        if selected < offset {
            offset = selected;
        } else if selected >= offset + viewport {
            offset = selected + 1 - viewport;
        }

        self.scroll_animator.set_target(offset as f64);

        let animated_offset = self.scroll_animator.current().round() as usize;
        let animated_offset = animated_offset.min(max_scroll);

        let end = (animated_offset + viewport).min(row_count);
        animated_offset..end
    }

    pub fn dispatch(&mut self, action: TreeAction, filter: &str) {
        match action {
            TreeAction::MoveUp => self.move_by(-1, filter),
            TreeAction::MoveDown => self.move_by(1, filter),
            TreeAction::HalfPageUp => self.move_by(-HALF_PAGE_STEP, filter),
            TreeAction::HalfPageDown => self.move_by(HALF_PAGE_STEP, filter),
            TreeAction::Collapse => self.collapse_or_go_to_parent(filter),
            TreeAction::Expand => self.expand_or_go_to_first_child(filter),
            TreeAction::ToggleExpanded => self.toggle_selected_expansion(filter),
            TreeAction::CollapseAll => self.collapse_all_expansion(filter),
            TreeAction::ExpandAll => self.expand_all_expansion(filter),
            TreeAction::GoToStart => self.go_to_start(),
            TreeAction::GoToEnd => self.go_to_end(filter),
            TreeAction::GotoPrefix => self.handle_goto_prefix(),
        }
    }

    pub fn clear_pending_prefix(&mut self) {
        self.pending_goto_prefix = false;
    }

    pub fn clamp_selection(&mut self, filter: &str) {
        let row_count = self.rows(filter).len();
        if row_count == 0 {
            self.selected_row = 0;
            self.scroll = 0;
            self.scroll_animator.snap_to(0.0);
        } else {
            self.selected_row = self.selected_row.min(row_count - 1);
            self.scroll = self.scroll.min(row_count - 1);
            self.scroll_animator.snap_to(self.scroll as f64);
        }
    }

    fn move_by(&mut self, delta: isize, filter: &str) {
        self.pending_goto_prefix = false;
        let rows = self.rows(filter);
        if rows.is_empty() {
            self.selected_row = 0;
            self.scroll = 0;
            return;
        }

        let max_index = rows.len() - 1;
        self.selected_row = self
            .selected_row
            .saturating_add_signed(delta)
            .min(max_index);
        self.scroll = self.selected_row.saturating_sub(HALF_PAGE_STEP as usize);
    }

    fn collapse_or_go_to_parent(&mut self, filter: &str) {
        self.pending_goto_prefix = false;
        let rows = self.rows(filter);
        let Some(row) = rows.get(self.selected_row) else {
            return;
        };
        let selected_id = self.items[row.item_index].id.as_str();

        if row.expandable && row.expanded {
            self.expanded_item_ids.remove(selected_id);
            self.clamp_selection(filter);
            return;
        }

        if let Some(parent_row_index) = self.parent_row_index(&rows, row.depth) {
            self.selected_row = parent_row_index;
        } else {
            self.go_to_start();
        }
    }

    fn expand_or_go_to_first_child(&mut self, filter: &str) {
        self.pending_goto_prefix = false;
        let rows = self.rows(filter);
        let Some(row) = rows.get(self.selected_row) else {
            return;
        };
        if !row.expandable {
            return;
        }

        let selected_id = self.items[row.item_index].id.clone();
        if !row.expanded {
            self.expanded_item_ids.insert(selected_id);
            return;
        }

        if rows
            .get(self.selected_row + 1)
            .is_some_and(|next| next.depth > row.depth)
        {
            self.selected_row += 1;
        }
    }

    fn toggle_selected_expansion(&mut self, filter: &str) {
        self.pending_goto_prefix = false;
        let rows = self.rows(filter);
        let Some(row) = rows.get(self.selected_row) else {
            return;
        };
        if !row.expandable {
            return;
        }

        let id = &self.items[row.item_index].id;
        if !self.expanded_item_ids.remove(id) {
            self.expanded_item_ids.insert(id.clone());
        }
        self.clamp_selection(filter);
    }

    fn collapse_all_expansion(&mut self, filter: &str) {
        self.pending_goto_prefix = false;
        self.expanded_item_ids.clear();
        self.clamp_selection(filter);
    }

    fn expand_all_expansion(&mut self, filter: &str) {
        self.pending_goto_prefix = false;
        self.expanded_item_ids.extend(self.expandable_item_ids());
        self.clamp_selection(filter);
    }

    fn go_to_start(&mut self) {
        self.pending_goto_prefix = false;
        self.selected_row = 0;
        self.scroll = 0;
    }

    fn go_to_end(&mut self, filter: &str) {
        self.pending_goto_prefix = false;
        let row_count = self.rows(filter).len();
        if row_count == 0 {
            self.selected_row = 0;
            self.scroll = 0;
        } else {
            self.selected_row = row_count - 1;
            self.scroll = self.selected_row.saturating_sub(HALF_PAGE_STEP as usize);
        }
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

    fn build_rows(&self, filter: &str) -> Vec<TreeRow> {
        let children = self.children_by_parent();
        let index_by_id = self.index_by_id();
        let roots = self.root_item_indices(&index_by_id);
        let mut rows = Vec::new();

        for root in roots {
            self.push_item_row(root, 0, &children, filter, &mut rows);
        }

        rows
    }

    fn push_item_row(
        &self,
        item_index: usize,
        depth: usize,
        children: &HashMap<&str, Vec<usize>>,
        filter: &str,
        rows: &mut Vec<TreeRow>,
    ) -> bool {
        let item = &self.items[item_index];
        let child_indices = children.get(item.id.as_str());
        let self_matches = self.item_matches_filter(item, filter);
        let mut child_rows = Vec::new();

        if let Some(child_indices) = child_indices {
            for child_index in child_indices {
                self.push_item_row(*child_index, depth + 1, children, filter, &mut child_rows);
            }
        }

        let include = filter.is_empty() || self_matches || !child_rows.is_empty();
        if include {
            let has_children = child_indices.is_some_and(|children| !children.is_empty());
            let expanded = !filter.is_empty() || self.expanded_item_ids.contains(&item.id);
            rows.push(TreeRow {
                item_index,
                depth,
                expandable: has_children,
                expanded,
            });
            if expanded {
                rows.extend(child_rows);
            }
        }

        include
    }

    fn item_matches_filter(&self, item: &TreeItem, filter: &str) -> bool {
        if filter.is_empty() {
            return true;
        }

        let matcher = SkimMatcherV2::default().smart_case();
        match self.searchable_field_ids.as_deref() {
            Some(field_ids) => searchable_fields_for_ids(item, field_ids)
                .iter()
                .any(|field| matcher.fuzzy_match(field, filter).is_some()),
            None => searchable_fields(item)
                .iter()
                .any(|field| matcher.fuzzy_match(field, filter).is_some()),
        }
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

    fn expandable_item_ids(&self) -> Vec<String> {
        self.children_by_parent()
            .into_iter()
            .filter_map(|(id, children)| (!children.is_empty()).then_some(id.to_owned()))
            .collect()
    }
}

pub fn item_matches_filter(item: &TreeItem, filter: &str) -> bool {
    if filter.is_empty() {
        return true;
    }

    let matcher = SkimMatcherV2::default().smart_case();
    searchable_fields(item)
        .iter()
        .any(|field| matcher.fuzzy_match(field, filter).is_some())
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

fn searchable_fields(item: &TreeItem) -> Vec<&str> {
    let mut fields = Vec::with_capacity(4 + item.field_values.len());
    fields.push(item.id.as_str());
    fields.push(item.label.as_str());
    fields.push(item.kind.as_str());
    fields.push(item.status.as_str());
    fields.extend(item.field_values.values().map(String::as_str));
    fields
}

fn searchable_fields_for_ids<'a>(item: &'a TreeItem, field_ids: &[String]) -> Vec<&'a str> {
    let mut fields = Vec::with_capacity(field_ids.len());
    for field_id in field_ids {
        match field_id.as_str() {
            "key" => fields.push(item.id.as_str()),
            "summary" => fields.push(item.label.as_str()),
            "issuetype" => fields.push(item.kind.as_str()),
            "status" => fields.push(item.status.as_str()),
            id => {
                if let Some(value) = item.field_values.get(id) {
                    fields.push(value.as_str());
                }
            }
        }
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::{TreeAction, TreeItem, TreeState, item_matches_filter};

    #[test]
    fn rows_preserve_loaded_root_order_and_indent_children() {
        let tree = TreeState::new(vec![
            item("EPIC", "Parent", "Epic", None, 1),
            item("ONE", "Unparented", "Task", None, 0),
            item("STORY", "Child", "Story", Some("EPIC"), 0),
        ]);

        let rows = tree.rows("");

        assert_eq!(tree.items()[rows[0].item_index].id, "EPIC");
        assert_eq!(tree.items()[rows[1].item_index].id, "ONE");
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn expand_moves_to_first_child_when_already_expanded() {
        let mut tree = TreeState::new(vec![
            item("EPIC", "Parent", "Epic", None, 0),
            item("STORY", "Child", "Story", Some("EPIC"), 0),
        ]);

        tree.dispatch(TreeAction::Expand, "");
        assert_eq!(tree.rows("").len(), 2);
        assert_eq!(tree.selected_row(), 0);

        tree.dispatch(TreeAction::Expand, "");
        assert_eq!(tree.selected_row(), 1);
    }

    #[test]
    fn collapse_moves_to_parent_when_already_collapsed() {
        let mut tree = TreeState::new(vec![
            item("EPIC", "Parent", "Epic", None, 0),
            item("STORY", "Child", "Story", Some("EPIC"), 0),
        ]);

        tree.dispatch(TreeAction::Expand, "");
        tree.dispatch(TreeAction::Expand, "");
        tree.dispatch(TreeAction::Collapse, "");

        assert_eq!(tree.selected_row(), 0);
    }

    #[test]
    fn g_g_and_uppercase_g_jump_to_start_and_end() {
        let mut tree = TreeState::new(vec![
            item("ONE", "One", "Task", None, 0),
            item("TWO", "Two", "Task", None, 0),
            item("THREE", "Three", "Task", None, 0),
        ]);

        tree.dispatch(TreeAction::GoToEnd, "");
        assert_eq!(tree.selected_row(), 2);

        tree.dispatch(TreeAction::GotoPrefix, "");
        tree.dispatch(TreeAction::GotoPrefix, "");
        assert_eq!(tree.selected_row(), 0);
    }

    #[test]
    fn fuzzy_filter_matches_non_contiguous_letters() {
        let item = item("ONE", "Checkout payments", "Task", None, 0);

        assert!(item_matches_filter(&item, "copay"));
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

        tree.dispatch(TreeAction::GoToEnd, "");
        let range = tree.visible_range("", 10);

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

        let range_end = tree.visible_range("", 10);
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

        let rows = tree.rows("");
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

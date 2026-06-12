use std::collections::HashSet;

use super::{
    filter::{FilterAction, FilterEvent, FilterState},
    tree::{TreeAction, TreeEvent, TreeItem, TreeRow, TreeState},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilteredTreeViewMode {
    List,
    Table,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilteredTreeAction {
    Tree(TreeAction),
    FocusFilter,
    ClearFilter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilteredTreeEvent {
    Quit,
    /// The selected item was expanded but its children are not loaded yet.
    LoadChildren(String),
    /// The filter text changed; the new value should drive a server search.
    FilterChanged(String),
    /// The filter was cleared; browse state should be restored.
    FilterCleared,
}

#[derive(Clone, PartialEq, Eq)]
pub struct FilteredTreeState {
    tree: TreeState,
    filter: FilterState,
    view_mode: FilteredTreeViewMode,
}

impl FilteredTreeState {
    pub fn new(items: Vec<TreeItem>) -> Self {
        Self {
            tree: TreeState::new(items),
            filter: FilterState::default(),
            view_mode: FilteredTreeViewMode::List,
        }
    }

    pub fn items(&self) -> &[TreeItem] {
        self.tree.items()
    }

    /// Number of currently-loaded root items (those without a parent).
    pub fn root_count(&self) -> usize {
        self.tree
            .items()
            .iter()
            .filter(|item| item.parent_id.is_none())
            .count()
    }

    pub fn update_item_field(&mut self, item_id: &str, field_id: &str, value: Option<String>) {
        self.tree.update_item_field(item_id, field_id, value);
        self.tree.clamp_selection();
    }

    pub fn update_item_status(&mut self, item_id: &str, status: String, status_id: Option<String>) {
        self.tree.update_item_status(item_id, status, status_id);
        self.tree.clamp_selection();
    }

    pub fn set_items(&mut self, items: Vec<TreeItem>, preserve_expanded: &HashSet<String>) {
        self.tree.set_items(items, preserve_expanded);
        self.tree.clamp_selection();
    }

    pub fn set_flat(&mut self, flat: bool) {
        self.tree.set_flat(flat);
    }

    pub fn add_children(&mut self, parent_id: &str, children: Vec<TreeItem>) {
        self.tree.add_children(parent_id, children);
    }

    pub fn append_items(&mut self, items: Vec<TreeItem>) {
        self.tree.append_items(items);
    }

    pub fn mark_children_failed(&mut self, parent_id: &str) {
        self.tree.mark_children_failed(parent_id);
    }

    pub fn reload_children(&mut self, parent_id: &str) -> bool {
        self.tree.reload_children(parent_id)
    }

    pub fn merge_root_items(&mut self, incoming: Vec<TreeItem>) {
        self.tree.merge_root_items(incoming);
    }

    pub fn retain_roots(&mut self, keep: &HashSet<String>) {
        self.tree.retain_roots(keep);
    }

    pub fn begin_soft_reload(&mut self, expanded: &HashSet<String>) -> Vec<String> {
        self.tree.begin_soft_reload(expanded)
    }

    pub fn replace_children(&mut self, parent_id: &str, incoming: Vec<TreeItem>) {
        self.tree.replace_children(parent_id, incoming);
    }

    pub fn mark_children_loaded(&mut self, parent_id: &str) {
        self.tree.mark_children_loaded(parent_id);
    }

    pub fn expanded_item_ids(&self) -> &HashSet<String> {
        self.tree.expanded_item_ids()
    }

    pub fn expanded_descendant_ids(&self, parent_id: &str) -> HashSet<String> {
        self.tree.expanded_descendant_ids(parent_id)
    }

    pub fn contains_item(&self, id: &str) -> bool {
        self.tree.contains_item(id)
    }

    pub fn nodes_needing_child_reload(&mut self, restore: &HashSet<String>) -> Vec<String> {
        self.tree.nodes_needing_child_reload(restore)
    }

    pub fn tick(&mut self, dt: std::time::Duration) {
        self.tree.tick(dt);
    }

    pub fn is_animating(&self) -> bool {
        self.tree.is_animating()
    }

    pub fn any_loading(&self) -> bool {
        self.tree.any_loading()
    }

    pub fn view_mode(&self) -> FilteredTreeViewMode {
        self.view_mode
    }

    pub fn set_view_mode(&mut self, view_mode: FilteredTreeViewMode) {
        self.view_mode = view_mode;
    }

    pub fn selected_item_index(&self) -> usize {
        self.tree.selected_row()
    }

    pub fn select_item_index(&mut self, index: usize) {
        self.tree.select_row(index);
    }

    pub fn selected_item_id(&self) -> Option<&str> {
        self.tree.selected_item_id()
    }

    pub fn selected_root_ancestor_id(&self) -> Option<String> {
        self.tree.selected_root_ancestor_id()
    }

    pub fn select_item_by_id(&mut self, id: &str) {
        self.tree.select_item_by_id(id);
    }

    pub fn scroll_offset(&self) -> usize {
        self.tree.scroll_offset()
    }

    pub fn scroll_viewport(&mut self, delta: isize, height: usize) {
        self.tree.scroll_viewport(delta, height);
    }

    pub fn filter(&self) -> &str {
        self.filter.value()
    }

    pub fn filter_cursor(&self) -> usize {
        self.filter.cursor()
    }

    pub fn filter_state(&self) -> &FilterState {
        &self.filter
    }

    pub fn is_filter_focused(&self) -> bool {
        self.filter.is_focused()
    }

    pub fn visible_rows(&self) -> Vec<TreeRow> {
        self.tree.rows()
    }

    pub fn visible_range(&self, height: usize) -> std::ops::Range<usize> {
        self.tree.visible_range(height)
    }

    pub fn dispatch(&mut self, action: FilteredTreeAction) -> Option<FilteredTreeEvent> {
        match action {
            FilteredTreeAction::Tree(action) => {
                let event = self.tree.dispatch(action)?;
                match event {
                    TreeEvent::LoadChildren(id) => Some(FilteredTreeEvent::LoadChildren(id)),
                }
            }
            FilteredTreeAction::FocusFilter => {
                self.filter.focus();
                self.tree.clear_pending_prefix();
                None
            }
            FilteredTreeAction::ClearFilter => {
                let had_filter = !self.filter.value().is_empty();
                self.filter.clear();
                had_filter.then_some(FilteredTreeEvent::FilterCleared)
            }
        }
    }

    pub fn dispatch_filter(&mut self, action: FilterAction) -> Option<FilteredTreeEvent> {
        if matches!(action, FilterAction::Quit) {
            return Some(FilteredTreeEvent::Quit);
        }

        match self.filter.dispatch(action) {
            Some(FilterEvent::Changed) => {
                let value = self.filter.value();
                if value.is_empty() {
                    Some(FilteredTreeEvent::FilterCleared)
                } else {
                    Some(FilteredTreeEvent::FilterChanged(value.to_owned()))
                }
            }
            Some(FilterEvent::Blurred) | None => None,
        }
    }

    pub fn clear_transient_input(&mut self) {
        self.filter.clear_focus();
        self.tree.clear_pending_prefix();
        self.tree.clamp_selection();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::super::{
        filter::FilterAction,
        tree::{Children, TreeAction, TreeItem},
    };
    use super::{FilteredTreeAction, FilteredTreeEvent, FilteredTreeState, FilteredTreeViewMode};

    #[test]
    fn supports_switching_between_list_and_table_view() {
        let mut filtered_tree = FilteredTreeState::new(Vec::new());

        filtered_tree.set_view_mode(FilteredTreeViewMode::Table);

        assert_eq!(filtered_tree.view_mode(), FilteredTreeViewMode::Table);
    }

    #[test]
    fn typing_filter_emits_filter_changed_with_current_value() {
        let mut filtered_tree = FilteredTreeState::new(vec![
            item("ONE", "Checkout", None),
            item("TWO", "Catalog", None),
        ]);

        filtered_tree.dispatch(FilteredTreeAction::FocusFilter);
        filtered_tree.dispatch_filter(FilterAction::Text('c'));
        let event = filtered_tree.dispatch_filter(FilterAction::Text('a'));

        assert_eq!(
            event,
            Some(FilteredTreeEvent::FilterChanged(String::from("ca")))
        );
        assert_eq!(filtered_tree.filter(), "ca");
    }

    #[test]
    fn emptying_filter_by_backspace_emits_filter_cleared() {
        let mut filtered_tree = FilteredTreeState::new(vec![item("ONE", "Checkout", None)]);

        filtered_tree.dispatch(FilteredTreeAction::FocusFilter);
        filtered_tree.dispatch_filter(FilterAction::Text('c'));
        let event = filtered_tree.dispatch_filter(FilterAction::Backspace);

        assert_eq!(event, Some(FilteredTreeEvent::FilterCleared));
        assert_eq!(filtered_tree.filter(), "");
    }

    #[test]
    fn clear_filter_action_emits_cleared_only_when_filter_was_set() {
        let mut filtered_tree = FilteredTreeState::new(vec![item("ONE", "Checkout", None)]);

        assert_eq!(
            filtered_tree.dispatch(FilteredTreeAction::ClearFilter),
            None
        );

        filtered_tree.dispatch(FilteredTreeAction::FocusFilter);
        filtered_tree.dispatch_filter(FilterAction::Text('c'));

        assert_eq!(
            filtered_tree.dispatch(FilteredTreeAction::ClearFilter),
            Some(FilteredTreeEvent::FilterCleared)
        );
        assert_eq!(filtered_tree.filter(), "");
    }

    #[test]
    fn tree_action_is_scoped_through_filtered_tree() {
        let mut filtered_tree =
            FilteredTreeState::new(vec![item("ONE", "One", None), item("TWO", "Two", None)]);

        filtered_tree.dispatch(FilteredTreeAction::Tree(TreeAction::MoveDown));

        assert_eq!(filtered_tree.selected_item_index(), 1);
    }

    #[test]
    fn expanding_not_loaded_item_emits_load_children() {
        let mut root = item("EPIC", "Parent", None);
        root.children = Children::NotLoaded;
        let mut filtered_tree = FilteredTreeState::new(vec![root]);

        let event = filtered_tree.dispatch(FilteredTreeAction::Tree(TreeAction::ToggleExpanded));

        assert_eq!(
            event,
            Some(FilteredTreeEvent::LoadChildren(String::from("EPIC")))
        );
    }

    #[test]
    fn refreshed_items_preserve_selected_id() {
        let mut filtered_tree = FilteredTreeState::new(vec![
            item("ONE", "Checkout", None),
            item("TWO", "Catalog", None),
            item("THREE", "Cart", None),
        ]);
        filtered_tree.dispatch(FilteredTreeAction::Tree(TreeAction::MoveDown));
        filtered_tree.dispatch(FilteredTreeAction::Tree(TreeAction::MoveDown));
        assert_eq!(filtered_tree.selected_item_id(), Some("THREE"));

        filtered_tree.set_items(
            vec![
                item("THREE", "Cart refreshed", None),
                item("TWO", "Catalog refreshed", None),
                item("ONE", "Checkout refreshed", None),
            ],
            &HashSet::new(),
        );

        assert_eq!(filtered_tree.selected_item_id(), Some("THREE"));
        assert_eq!(filtered_tree.selected_item_index(), 0);
    }

    fn item(id: &str, label: &str, parent_id: Option<&str>) -> TreeItem {
        TreeItem {
            id: id.to_owned(),
            label: label.to_owned(),
            status: String::from("To Do"),
            kind: String::from("Task"),
            parent_id: parent_id.map(str::to_owned),
            field_values: std::collections::BTreeMap::new(),
            root_order: 0,
            children: Children::Unknown,
        }
    }
}

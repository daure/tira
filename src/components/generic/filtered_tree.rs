use super::{
    filter::{FilterAction, FilterEvent, FilterState},
    tree::{TreeAction, TreeItem, TreeRow, TreeState},
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilteredTreeEvent {
    Quit,
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

    pub fn set_items(&mut self, items: Vec<TreeItem>) {
        self.tree.set_items(items);
        self.tree.clamp_selection(self.filter.value());
    }

    pub fn set_searchable_field_ids(&mut self, field_ids: Option<Vec<String>>) {
        self.tree.set_searchable_field_ids(field_ids);
        self.tree.clamp_selection(self.filter.value());
    }

    pub fn tick(&mut self, dt: std::time::Duration) {
        self.tree.tick(dt);
    }

    pub fn is_animating(&self) -> bool {
        self.tree.is_animating()
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

    pub fn selected_item_id(&self) -> Option<&str> {
        self.tree.selected_item_id()
    }

    pub fn scroll_offset(&self) -> usize {
        self.tree.scroll_offset()
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
        self.tree.rows(self.filter.value())
    }

    pub fn visible_range(&self, height: usize) -> std::ops::Range<usize> {
        self.tree.visible_range(self.filter.value(), height)
    }

    pub fn dispatch(&mut self, action: FilteredTreeAction) {
        match action {
            FilteredTreeAction::Tree(action) => self.tree.dispatch(action, self.filter.value()),
            FilteredTreeAction::FocusFilter => {
                self.filter.focus();
                self.tree.clear_pending_prefix();
            }
            FilteredTreeAction::ClearFilter => {
                self.filter.clear();
                self.tree.clamp_selection(self.filter.value());
            }
        }
    }

    pub fn dispatch_filter(&mut self, action: FilterAction) -> Option<FilteredTreeEvent> {
        if matches!(action, FilterAction::Quit) {
            return Some(FilteredTreeEvent::Quit);
        }

        let event = self.filter.dispatch(action);
        if matches!(event, Some(FilterEvent::Changed | FilterEvent::Blurred)) {
            self.tree.clamp_selection(self.filter.value());
        }
        None
    }

    pub fn clear_transient_input(&mut self) {
        self.filter.clear_focus();
        self.tree.clear_pending_prefix();
        self.tree.clamp_selection(self.filter.value());
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        filter::FilterAction,
        tree::{TreeAction, TreeItem},
    };
    use super::{FilteredTreeAction, FilteredTreeState, FilteredTreeViewMode};

    #[test]
    fn supports_switching_between_list_and_table_view() {
        let mut filtered_tree = FilteredTreeState::new(Vec::new());

        filtered_tree.set_view_mode(FilteredTreeViewMode::Table);

        assert_eq!(filtered_tree.view_mode(), FilteredTreeViewMode::Table);
    }

    #[test]
    fn filter_blur_keeps_filter_text_and_current_clamped_selection() {
        let mut filtered_tree = FilteredTreeState::new(vec![
            item("ONE", "Cart", None),
            item("TWO", "Catalog", None),
            item("THREE", "Other", None),
        ]);

        filtered_tree.dispatch(FilteredTreeAction::Tree(TreeAction::MoveDown));
        filtered_tree.dispatch(FilteredTreeAction::Tree(TreeAction::MoveDown));
        filtered_tree.dispatch(FilteredTreeAction::FocusFilter);
        filtered_tree.dispatch_filter(FilterAction::Text('c'));
        filtered_tree.dispatch_filter(FilterAction::Text('a'));
        filtered_tree.dispatch_filter(FilterAction::Exit);

        assert_eq!(filtered_tree.filter(), "ca");
        assert!(!filtered_tree.is_filter_focused());
        assert_eq!(filtered_tree.selected_item_index(), 1);
    }

    #[test]
    fn typing_filter_clamps_selection_without_jumping_to_first_row() {
        let mut filtered_tree = FilteredTreeState::new(vec![
            item("ONE", "Checkout", None),
            item("TWO", "Catalog", None),
            item("THREE", "Other", None),
        ]);

        filtered_tree.dispatch(FilteredTreeAction::Tree(TreeAction::MoveDown));
        filtered_tree.dispatch(FilteredTreeAction::Tree(TreeAction::MoveDown));
        filtered_tree.dispatch(FilteredTreeAction::FocusFilter);
        filtered_tree.dispatch_filter(FilterAction::Text('c'));

        assert_eq!(filtered_tree.visible_rows().len(), 2);
        assert_eq!(filtered_tree.selected_item_index(), 1);
    }

    #[test]
    fn clear_filter_restores_all_rows_without_resetting_selection() {
        let mut filtered_tree = FilteredTreeState::new(vec![
            item("ONE", "Checkout", None),
            item("TWO", "Catalog", None),
        ]);

        filtered_tree.dispatch(FilteredTreeAction::Tree(TreeAction::MoveDown));
        filtered_tree.dispatch(FilteredTreeAction::FocusFilter);
        filtered_tree.dispatch_filter(FilterAction::Text('c'));
        assert_eq!(filtered_tree.visible_rows().len(), 2);
        assert_eq!(filtered_tree.selected_item_index(), 1);

        filtered_tree.dispatch(FilteredTreeAction::ClearFilter);

        assert_eq!(filtered_tree.filter(), "");
        assert!(!filtered_tree.is_filter_focused());
        assert_eq!(filtered_tree.visible_rows().len(), 2);
        assert_eq!(filtered_tree.selected_item_index(), 1);
    }

    #[test]
    fn tree_action_is_scoped_through_filtered_tree() {
        let mut filtered_tree =
            FilteredTreeState::new(vec![item("ONE", "One", None), item("TWO", "Two", None)]);

        filtered_tree.dispatch(FilteredTreeAction::Tree(TreeAction::MoveDown));

        assert_eq!(filtered_tree.selected_item_index(), 1);
    }

    #[test]
    fn refreshed_items_preserve_filter_and_selected_id() {
        let mut filtered_tree = FilteredTreeState::new(vec![
            item("ONE", "Checkout", None),
            item("TWO", "Catalog", None),
            item("THREE", "Cart", None),
        ]);
        filtered_tree.dispatch(FilteredTreeAction::FocusFilter);
        filtered_tree.dispatch_filter(FilterAction::Text('c'));
        filtered_tree.dispatch_filter(FilterAction::Text('a'));
        filtered_tree.dispatch_filter(FilterAction::Exit);
        filtered_tree.dispatch(FilteredTreeAction::Tree(TreeAction::MoveDown));
        assert_eq!(filtered_tree.selected_item_id(), Some("THREE"));

        filtered_tree.set_items(vec![
            item("THREE", "Cart refreshed", None),
            item("TWO", "Catalog refreshed", None),
            item("ONE", "Checkout refreshed", None),
        ]);

        assert_eq!(filtered_tree.filter(), "ca");
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
        }
    }
}

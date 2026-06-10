use std::collections::HashSet;

use crate::components::generic::{
    dropdown::{
        DropdownAction, DropdownEvent, DropdownOption, DropdownVisibleOption,
        MultiSelectDropdownState,
    },
    filtered_tree::{
        FilteredTreeAction, FilteredTreeEvent, FilteredTreeState, FilteredTreeViewMode,
    },
    tree::{TreeItem, TreeRow},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JiraIssueColumn {
    IssueKey,
    Summary,
    IssueType,
    Status,
    Field { id: String, label: String },
}

impl JiraIssueColumn {
    pub fn default_columns() -> Vec<Self> {
        let mut columns = Self::fixed_columns();
        columns.push(Self::Field {
            id: String::from("assignee"),
            label: String::from("Assignee"),
        });
        columns.push(Self::Status);
        columns.push(Self::labels_column());
        columns
    }

    pub fn fixed_columns() -> Vec<Self> {
        vec![
            Self::IssueKey,
            Self::Field {
                id: String::from("priority"),
                label: String::from("Priority"),
            },
            Self::Summary,
        ]
    }

    pub fn labels_column() -> Self {
        Self::Field {
            id: String::from("labels"),
            label: String::from("Labels"),
        }
    }

    pub fn is_fixed(&self) -> bool {
        matches!(self, Self::IssueKey | Self::Summary)
            || matches!(self, Self::Field { id, .. } if id == "priority")
    }
    pub fn label(&self) -> &str {
        match self {
            Self::IssueKey => "Work",
            Self::Summary => "Summary",
            Self::IssueType => "Work type",
            Self::Status => "Status",
            Self::Field { id, label } if id == "priority" => "",
            Self::Field { label, .. } => label,
        }
    }

    /// The Jira field id this column reads from a search response, if any.
    /// `IssueKey`/`Summary`/`IssueType`/`Status` are always requested as base
    /// fields, so they report `None` here.
    pub fn field_id(&self) -> Option<&str> {
        match self {
            Self::Field { id, .. } => Some(id.as_str()),
            Self::IssueKey | Self::Summary | Self::IssueType | Self::Status => None,
        }
    }

    /// Builds the comma-separated `fields` query value for `/search/jql` that
    /// covers the base fields the tree always needs plus every dynamic field the
    /// given columns render. Order is stable and de-duplicated.
    pub fn fields_param<'a>(columns: impl IntoIterator<Item = &'a Self>) -> String {
        let mut fields = vec!["summary", "status", "issuetype", "parent"];
        for column in columns {
            if let Some(id) = column.field_id()
                && !fields.contains(&id)
            {
                fields.push(id);
            }
        }
        fields.join(",")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JiraFilteredTreeAction {
    FilteredTree(FilteredTreeAction),
    Dropdown(DropdownAction),
    OpenColumns,
    YankIssueUrlPrefix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JiraFilteredTreeEvent {
    Quit,
    IssueUrlCopyRequested(String),
    IssueUrlCopyUnavailable(String),
    ColumnsChanged(Vec<JiraIssueColumn>),
    /// The selected issue was expanded but its children are not loaded yet.
    LoadChildren(String),
    /// The search filter changed; drive a server-side search with this term.
    FilterChanged(String),
    /// The search filter was cleared; restore the browse view.
    FilterCleared,
}

#[derive(Clone, PartialEq, Eq)]
pub struct JiraFilteredTreeState {
    filtered_tree: FilteredTreeState,
    jira_site: Option<String>,
    column_dropdown: Option<MultiSelectDropdownState<JiraIssueColumn>>,
    visible_columns: Vec<JiraIssueColumn>,
    available_columns: Vec<JiraIssueColumn>,
    pending_yank: bool,
}

impl JiraFilteredTreeState {
    pub fn new(items: Vec<TreeItem>) -> Self {
        let default_columns = JiraIssueColumn::default_columns();
        let available_columns = default_columns
            .iter()
            .filter(|column| !column.is_fixed())
            .cloned()
            .collect::<Vec<_>>();
        Self {
            filtered_tree: FilteredTreeState::new(items),
            jira_site: None,
            column_dropdown: None,
            visible_columns: default_columns,
            available_columns,
            pending_yank: false,
        }
    }

    pub fn set_jira_site(&mut self, site: impl Into<String>) {
        self.jira_site = Some(site.into());
    }

    pub fn set_items(&mut self, items: Vec<TreeItem>, preserve_expanded: &HashSet<String>) {
        self.filtered_tree.set_items(items, preserve_expanded);
    }

    pub fn set_flat(&mut self, flat: bool) {
        self.filtered_tree.set_flat(flat);
    }

    pub fn add_children(&mut self, parent_id: &str, children: Vec<TreeItem>) {
        self.filtered_tree.add_children(parent_id, children);
    }

    pub fn append_items(&mut self, items: Vec<TreeItem>) {
        self.filtered_tree.append_items(items);
    }

    pub fn mark_children_failed(&mut self, parent_id: &str) {
        self.filtered_tree.mark_children_failed(parent_id);
    }

    pub fn reload_children(&mut self, parent_id: &str) -> bool {
        self.filtered_tree.reload_children(parent_id)
    }

    pub fn merge_root_items(&mut self, incoming: Vec<TreeItem>) {
        self.filtered_tree.merge_root_items(incoming);
    }

    pub fn retain_roots(&mut self, keep: &HashSet<String>) {
        self.filtered_tree.retain_roots(keep);
    }

    pub fn begin_soft_reload(&mut self, expanded: &HashSet<String>) -> Vec<String> {
        self.filtered_tree.begin_soft_reload(expanded)
    }

    pub fn replace_children(&mut self, parent_id: &str, incoming: Vec<TreeItem>) {
        self.filtered_tree.replace_children(parent_id, incoming);
    }

    pub fn mark_children_loaded(&mut self, parent_id: &str) {
        self.filtered_tree.mark_children_loaded(parent_id);
    }

    pub fn expanded_item_ids(&self) -> &HashSet<String> {
        self.filtered_tree.expanded_item_ids()
    }

    pub fn expanded_descendant_ids(&self, parent_id: &str) -> HashSet<String> {
        self.filtered_tree.expanded_descendant_ids(parent_id)
    }

    pub fn contains_item(&self, id: &str) -> bool {
        self.filtered_tree.contains_item(id)
    }

    pub fn nodes_needing_child_reload(&mut self, restore: &HashSet<String>) -> Vec<String> {
        self.filtered_tree.nodes_needing_child_reload(restore)
    }

    pub fn update_assignee(&mut self, issue_key: &str, assignee_name: Option<String>) {
        self.filtered_tree
            .update_item_field(issue_key, "assignee", assignee_name);
    }

    pub fn set_available_columns(&mut self, columns: Vec<JiraIssueColumn>) {
        if !columns.is_empty() {
            self.available_columns = columns
                .into_iter()
                .filter(|column| !column.is_fixed())
                .collect();
        }
    }

    pub fn tick(&mut self, dt: std::time::Duration) {
        self.filtered_tree.tick(dt);
        if let Some(dropdown) = &mut self.column_dropdown {
            dropdown.tick(dt);
        }
    }

    pub fn is_animating(&self) -> bool {
        self.filtered_tree.is_animating()
            || self
                .column_dropdown
                .as_ref()
                .is_some_and(MultiSelectDropdownState::is_animating)
    }

    pub fn any_loading(&self) -> bool {
        self.filtered_tree.any_loading()
    }

    pub fn view_mode(&self) -> FilteredTreeViewMode {
        self.filtered_tree.view_mode()
    }

    pub fn set_view_mode(&mut self, view_mode: FilteredTreeViewMode) {
        self.filtered_tree.set_view_mode(view_mode);
    }

    pub fn items(&self) -> &[TreeItem] {
        self.filtered_tree.items()
    }

    /// Number of currently-loaded root items (those without a parent).
    pub fn root_count(&self) -> usize {
        self.filtered_tree.root_count()
    }

    pub fn selected_item_index(&self) -> usize {
        self.filtered_tree.selected_item_index()
    }

    pub fn select_item_index(&mut self, index: usize) {
        self.filtered_tree.select_item_index(index);
    }

    pub fn selected_item_id(&self) -> Option<&str> {
        self.filtered_tree.selected_item_id()
    }

    pub fn selected_root_ancestor_id(&self) -> Option<String> {
        self.filtered_tree.selected_root_ancestor_id()
    }

    pub fn select_item_by_id(&mut self, id: &str) {
        self.filtered_tree.select_item_by_id(id);
    }

    pub fn scroll_offset(&self) -> usize {
        self.filtered_tree.scroll_offset()
    }

    pub fn scroll_viewport(&mut self, delta: isize, height: usize) {
        self.filtered_tree.scroll_viewport(delta, height);
    }

    pub fn filter(&self) -> &str {
        self.filtered_tree.filter()
    }

    pub fn filter_cursor(&self) -> usize {
        self.filtered_tree.filter_cursor()
    }

    pub fn filter_state(&self) -> &crate::FilterState {
        self.filtered_tree.filter_state()
    }

    pub fn is_filter_focused(&self) -> bool {
        self.filtered_tree.is_filter_focused()
    }

    pub fn visible_rows(&self) -> Vec<TreeRow> {
        self.filtered_tree.visible_rows()
    }

    pub fn visible_range(&self, height: usize) -> std::ops::Range<usize> {
        self.filtered_tree.visible_range(height)
    }

    pub fn visible_columns(&self) -> &[JiraIssueColumn] {
        &self.visible_columns
    }

    pub fn column_dropdown(&self) -> Option<&MultiSelectDropdownState<JiraIssueColumn>> {
        self.column_dropdown.as_ref()
    }

    pub fn column_dropdown_scroll_offset(&self) -> Option<usize> {
        self.column_dropdown
            .as_ref()
            .map(MultiSelectDropdownState::scroll_offset)
    }

    pub fn is_column_dropdown_open(&self) -> bool {
        self.column_dropdown.is_some()
    }

    pub fn close_column_dropdown(&mut self) {
        self.column_dropdown = None;
    }
    pub fn is_column_dropdown_filter_focused(&self) -> bool {
        self.column_dropdown
            .as_ref()
            .is_some_and(MultiSelectDropdownState::is_filter_focused)
    }

    pub fn click_column_dropdown_row(&mut self, row: usize, height: usize) {
        let Some(dropdown) = &mut self.column_dropdown else {
            return;
        };
        let Some(entry) = dropdown.visible_window(height).into_iter().nth(row) else {
            return;
        };
        let DropdownVisibleOption::Option { index, .. } = entry else {
            return;
        };
        dropdown.set_selected_index(index);
        if matches!(
            dropdown.dispatch(DropdownAction::ToggleSelected),
            Some(DropdownEvent::Toggled(_))
        ) {
            self.sync_visible_columns();
        }
    }
    pub fn scroll_column_dropdown(&mut self, delta: isize) {
        if let Some(dropdown) = &mut self.column_dropdown {
            dropdown.scroll_viewport(delta);
        }
    }

    pub fn dispatch(&mut self, action: JiraFilteredTreeAction) -> Option<JiraFilteredTreeEvent> {
        match action {
            JiraFilteredTreeAction::FilteredTree(action) => {
                self.pending_yank = false;
                self.filtered_tree
                    .dispatch(action)
                    .map(Self::map_tree_event)
            }
            JiraFilteredTreeAction::Dropdown(action) => self.dispatch_dropdown(action),
            JiraFilteredTreeAction::OpenColumns => {
                self.pending_yank = false;
                self.toggle_columns();
                None
            }
            JiraFilteredTreeAction::YankIssueUrlPrefix => self.dispatch_yank_prefix(),
        }
    }

    pub fn dispatch_filter(
        &mut self,
        action: crate::FilterAction,
    ) -> Option<JiraFilteredTreeEvent> {
        self.filtered_tree
            .dispatch_filter(action)
            .map(Self::map_tree_event)
    }

    /// Maps a generic filtered-tree event onto the Jira-specific event surface.
    fn map_tree_event(event: FilteredTreeEvent) -> JiraFilteredTreeEvent {
        match event {
            FilteredTreeEvent::Quit => JiraFilteredTreeEvent::Quit,
            FilteredTreeEvent::LoadChildren(id) => JiraFilteredTreeEvent::LoadChildren(id),
            FilteredTreeEvent::FilterChanged(term) => JiraFilteredTreeEvent::FilterChanged(term),
            FilteredTreeEvent::FilterCleared => JiraFilteredTreeEvent::FilterCleared,
        }
    }

    pub fn clear_transient_input(&mut self) {
        self.filtered_tree.clear_transient_input();
    }

    fn dispatch_dropdown(&mut self, action: DropdownAction) -> Option<JiraFilteredTreeEvent> {
        let dropdown = self.column_dropdown.as_mut()?;
        match dropdown.dispatch(action) {
            Some(DropdownEvent::Closed) => self.column_dropdown = None,
            Some(DropdownEvent::Toggled(_)) => {
                self.sync_visible_columns();
                return Some(JiraFilteredTreeEvent::ColumnsChanged(
                    self.visible_columns.clone(),
                ));
            }
            None => {}
        }
        None
    }

    pub fn open_column_dropdown(&mut self) {
        self.open_columns();
    }

    fn toggle_columns(&mut self) {
        if self.column_dropdown.is_some() {
            self.column_dropdown = None;
        } else {
            self.open_columns();
        }
    }
    fn open_columns(&mut self) {
        let options = self
            .available_columns
            .iter()
            .cloned()
            .map(|column| DropdownOption {
                selected: self.visible_columns.contains(&column),
                label: Self::selector_label(&column),
                value: column,
            })
            .collect();
        self.column_dropdown = Some(
            MultiSelectDropdownState::new(options)
                .require_at_least_one_selection()
                .with_filter_focused(),
        );
    }

    fn sync_visible_columns(&mut self) {
        let Some(dropdown) = &self.column_dropdown else {
            return;
        };
        let selected = dropdown
            .options()
            .iter()
            .filter_map(|option| option.selected.then_some(option.value.clone()))
            .collect::<Vec<_>>();
        self.visible_columns = JiraIssueColumn::fixed_columns();
        self.visible_columns.extend(selected);
    }

    fn selector_label(column: &JiraIssueColumn) -> String {
        match column {
            JiraIssueColumn::Field { id, .. } if id == "priority" => String::from("Priority"),
            JiraIssueColumn::Field { id, .. } if id == "assignee" => String::from("Assignee"),
            _ => column.label().to_owned(),
        }
    }

    fn dispatch_yank_prefix(&mut self) -> Option<JiraFilteredTreeEvent> {
        if self.pending_yank {
            self.pending_yank = false;
            match self.selected_issue_url() {
                Some(url) => Some(JiraFilteredTreeEvent::IssueUrlCopyRequested(url)),
                None => Some(JiraFilteredTreeEvent::IssueUrlCopyUnavailable(
                    String::from("No selected issue or Jira site is available."),
                )),
            }
        } else {
            self.pending_yank = true;
            None
        }
    }

    fn selected_issue_url(&self) -> Option<String> {
        let site = self.jira_site.as_ref()?.trim_end_matches('/');
        let rows = self.filtered_tree.visible_rows();
        let row = rows.get(self.filtered_tree.selected_item_index())?;
        let issue = &self.filtered_tree.items()[row.item_index];

        Some(format!("{site}/browse/{}", issue.id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str) -> TreeItem {
        TreeItem {
            id: id.to_owned(),
            label: String::from("Issue"),
            status: String::from("To Do"),
            kind: String::from("Task"),
            parent_id: None,
            field_values: std::collections::BTreeMap::new(),
            root_order: 0,
            children: crate::components::generic::tree::Children::Unknown,
        }
    }

    #[test]
    fn selected_issue_url_uses_configured_jira_site_and_issue_key() {
        let mut tree = JiraFilteredTreeState::new(vec![item("KAN-20")]);
        tree.set_jira_site("https://example.atlassian.net/");

        assert_eq!(
            tree.selected_issue_url().as_deref(),
            Some("https://example.atlassian.net/browse/KAN-20")
        );
    }

    #[test]
    fn filter_input_emits_filter_changed_event_for_server_search() {
        let mut tree = JiraFilteredTreeState::new(vec![item("KAN-1"), item("KAN-2")]);

        let mut last = None;
        for ch in "price".chars() {
            last = tree.dispatch_filter(crate::FilterAction::Text(ch));
        }

        assert_eq!(
            last,
            Some(JiraFilteredTreeEvent::FilterChanged(String::from("price")))
        );
    }

    #[test]
    fn expanding_not_loaded_issue_emits_load_children_event() {
        let mut root = item("KAN-1");
        root.children = crate::components::generic::tree::Children::NotLoaded;
        let mut tree = JiraFilteredTreeState::new(vec![root]);

        let event = tree.dispatch(JiraFilteredTreeAction::FilteredTree(
            crate::components::generic::filtered_tree::FilteredTreeAction::Tree(
                crate::components::generic::tree::TreeAction::ToggleExpanded,
            ),
        ));

        assert_eq!(
            event,
            Some(JiraFilteredTreeEvent::LoadChildren(String::from("KAN-1")))
        );
    }

    #[test]
    fn fields_param_includes_base_fields_and_dynamic_columns_without_duplicates() {
        let columns = vec![
            JiraIssueColumn::IssueKey,
            JiraIssueColumn::Summary,
            JiraIssueColumn::Status,
            JiraIssueColumn::Field {
                id: String::from("assignee"),
                label: String::from("Assignee"),
            },
            JiraIssueColumn::Field {
                id: String::from("priority"),
                label: String::from("Priority"),
            },
        ];

        assert_eq!(
            JiraIssueColumn::fields_param(&columns),
            "summary,status,issuetype,parent,assignee,priority"
        );
    }
}

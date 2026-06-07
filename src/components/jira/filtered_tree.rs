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
        let mut state = Self {
            filtered_tree: FilteredTreeState::new(items),
            jira_site: None,
            column_dropdown: None,
            visible_columns: default_columns,
            available_columns,
            pending_yank: false,
        };
        state.sync_searchable_fields();
        state
    }

    pub fn set_jira_site(&mut self, site: impl Into<String>) {
        self.jira_site = Some(site.into());
    }

    pub fn set_items(&mut self, items: Vec<TreeItem>) {
        self.filtered_tree.set_items(items);
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

    pub fn view_mode(&self) -> FilteredTreeViewMode {
        self.filtered_tree.view_mode()
    }

    pub fn set_view_mode(&mut self, view_mode: FilteredTreeViewMode) {
        self.filtered_tree.set_view_mode(view_mode);
    }

    pub fn items(&self) -> &[TreeItem] {
        self.filtered_tree.items()
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
                self.filtered_tree.dispatch(action);
                None
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
            .map(|FilteredTreeEvent::Quit| JiraFilteredTreeEvent::Quit)
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
        self.sync_searchable_fields();
    }

    fn selector_label(column: &JiraIssueColumn) -> String {
        match column {
            JiraIssueColumn::Field { id, .. } if id == "priority" => String::from("Priority"),
            JiraIssueColumn::Field { id, .. } if id == "assignee" => String::from("Assignee"),
            _ => column.label().to_owned(),
        }
    }
    fn sync_searchable_fields(&mut self) {
        let field_ids = self
            .visible_columns
            .iter()
            .map(|column| match column {
                JiraIssueColumn::IssueKey => String::from("key"),
                JiraIssueColumn::Summary => String::from("summary"),
                JiraIssueColumn::IssueType => String::from("issuetype"),
                JiraIssueColumn::Status => String::from("status"),
                JiraIssueColumn::Field { id, .. } => id.clone(),
            })
            .collect();
        self.filtered_tree.set_searchable_field_ids(Some(field_ids));
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
    fn filter_uses_only_visible_jira_columns() {
        let mut hidden_only = item("KAN-1");
        hidden_only
            .field_values
            .insert(String::from("reporter"), String::from("Marlo Vlietstra"));
        let mut assignee = item("KAN-2");
        assignee
            .field_values
            .insert(String::from("assignee"), String::from("Marlo Vlietstra"));
        let mut tree = JiraFilteredTreeState::new(vec![hidden_only, assignee]);
        tree.set_available_columns(vec![
            JiraIssueColumn::IssueKey,
            JiraIssueColumn::Summary,
            JiraIssueColumn::IssueType,
            JiraIssueColumn::Status,
            JiraIssueColumn::Field {
                id: String::from("assignee"),
                label: String::from("Assignee"),
            },
        ]);

        for ch in "marlo vlietstra".chars() {
            tree.dispatch_filter(crate::FilterAction::Text(ch));
        }

        let rows = tree.visible_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(tree.items()[rows[0].item_index].id, "KAN-2");
    }
}

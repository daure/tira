use crate::{
    components::{
        generic::{
            filter::FilterAction,
            filtered_tree::FilteredTreeViewMode,
            notification::Notification,
            tabs::{TabAction, TabsState},
            tree::{TreeItem, TreeRow},
        },
        jira::filtered_tree::{
            JiraFilteredTreeAction, JiraFilteredTreeEvent, JiraFilteredTreeState, JiraIssueColumn,
        },
    },
    config::{self, JiraCredentials},
    keymap::KeyBindings,
    services::jira::{self, CommandLogEntry, IssueSummary, JiraLoadResult, ProjectSummary},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub const APP_TABS: &[&str] = &["Board", "List", "Timeline", "Filters"];
const DEFAULT_TAB_INDEX: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Tabs(TabAction),
    JiraFilteredTree(JiraFilteredTreeAction),
    ReloadList,
    ToggleCommandLog,
    CloseCommandLog,
    ToggleProjectDropdown,
    ProjectDropdown(crate::components::generic::dropdown::DropdownAction),
    Quit,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DialogKind {
    CommandLog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DropdownKind {
    JiraColumns,
    ProjectSwitcher,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupAction {
    NextField,
    PreviousField,
    Submit,
    Backspace,
    Quit,
    Text(char),
    None,
    MoveCursorStart,
    MoveCursorEnd,
    Clear,
    MoveCursorWordLeft,
    MoveCursorWordRight,
    DeleteWordLeft,
    DeleteWordRight,
    MoveCursorLeft,
    MoveCursorRight,
    DeleteToEnd,
    DeleteToStart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Setup,
    Main,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialField {
    Site,
    Email,
    ApiKey,
    DefaultProject,
}

impl CredentialField {
    const ALL: [CredentialField; 4] = [
        CredentialField::Site,
        CredentialField::Email,
        CredentialField::ApiKey,
        CredentialField::DefaultProject,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CredentialField::Site => "Jira site",
            CredentialField::Email => "Email",
            CredentialField::ApiKey => "API token",
            CredentialField::DefaultProject => "Project key",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialForm {
    site: String,
    email: String,
    api_key: String,
    default_project: String,
    active_field: usize,
    cursors: [usize; 4],
}

impl Default for CredentialForm {
    fn default() -> Self {
        Self {
            site: String::new(),
            email: String::new(),
            api_key: String::new(),
            default_project: String::new(),
            active_field: 0,
            cursors: [0; 4],
        }
    }
}

impl CredentialForm {
    pub fn active_field(&self) -> CredentialField {
        CredentialField::ALL[self.active_field]
    }

    pub fn fields(&self) -> [(CredentialField, &str); 4] {
        [
            (CredentialField::Site, &self.site),
            (CredentialField::Email, &self.email),
            (CredentialField::ApiKey, &self.api_key),
            (CredentialField::DefaultProject, &self.default_project),
        ]
    }

    pub fn cursors(&self) -> [usize; 4] {
        self.cursors
    }

    pub fn active_field_idx(&self) -> usize {
        self.active_field
    }

    fn next_field(&mut self) {
        self.active_field = (self.active_field + 1) % CredentialField::ALL.len();
    }

    fn previous_field(&mut self) {
        self.active_field = self
            .active_field
            .checked_sub(1)
            .unwrap_or(CredentialField::ALL.len() - 1);
    }

    fn push(&mut self, c: char) {
        let field_idx = self.active_field;
        let mut cursor = self.cursors[field_idx];
        let val = self.active_value_mut();
        crate::components::generic::input::insert_char(val, &mut cursor, c);
        self.cursors[field_idx] = cursor;
    }

    fn backspace(&mut self) {
        let field_idx = self.active_field;
        let mut cursor = self.cursors[field_idx];
        let val = self.active_value_mut();
        crate::components::generic::input::delete_backwards(val, &mut cursor);
        self.cursors[field_idx] = cursor;
    }

    fn active_value_mut(&mut self) -> &mut String {
        match self.active_field() {
            CredentialField::Site => &mut self.site,
            CredentialField::Email => &mut self.email,
            CredentialField::ApiKey => &mut self.api_key,
            CredentialField::DefaultProject => &mut self.default_project,
        }
    }

    fn credentials(&self) -> Option<JiraCredentials> {
        let credentials = JiraCredentials {
            site: self.site.trim().to_owned(),
            email: self.email.trim().to_owned(),
            api_key: self.api_key.trim().to_owned(),
            default_project: self.default_project.trim().to_owned(),
        };

        credentials.is_complete().then_some(credentials)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct App {
    tabs: TabsState,
    running: bool,
    screen: Screen,
    setup: CredentialForm,
    filtered_tree: JiraFilteredTreeState,
    credentials: Option<JiraCredentials>,
    command_log: Vec<CommandLogEntry>,
    command_log_open: bool,
    status: String,
    notifications: Vec<Notification>,
    projects: Vec<ProjectSummary>,
    project_dropdown:
        Option<crate::components::generic::dropdown::MultiSelectDropdownState<ProjectSummary>>,
}

impl Default for App {
    fn default() -> Self {
        Self::setup("No Jira credentials found. Enter them to save config and load Jira issues.")
    }
}

impl App {
    pub fn setup(status: impl Into<String>) -> Self {
        let mut filtered_tree = JiraFilteredTreeState::new(Vec::new());
        filtered_tree.set_view_mode(FilteredTreeViewMode::Table);

        Self {
            tabs: TabsState::new(DEFAULT_TAB_INDEX),
            running: true,
            screen: Screen::Setup,
            setup: CredentialForm::default(),
            filtered_tree,
            credentials: None,
            command_log: Vec::new(),
            command_log_open: false,
            status: status.into(),
            notifications: Vec::new(),
            projects: Vec::new(),
            project_dropdown: None,
        }
    }

    pub fn with_issues(issues: Vec<IssueSummary>) -> Self {
        let mut filtered_tree = JiraFilteredTreeState::new(tree_items_from_issues(issues));
        filtered_tree.set_view_mode(FilteredTreeViewMode::Table);

        Self {
            tabs: TabsState::new(DEFAULT_TAB_INDEX),
            running: true,
            screen: Screen::Main,
            setup: CredentialForm::default(),
            filtered_tree,
            credentials: None,
            command_log: Vec::new(),
            command_log_open: false,
            status: String::from("Jira issues loaded"),
            notifications: Vec::new(),
            projects: Vec::new(),
            project_dropdown: None,
        }
    }

    pub fn with_issues_and_projects(
        issues: Vec<IssueSummary>,
        projects: Vec<ProjectSummary>,
        current_project: impl Into<String>,
    ) -> Self {
        let current_project = current_project.into();
        let mut app = Self::with_issues(issues);
        app.credentials = Some(JiraCredentials {
            site: String::from("https://example.atlassian.net"),
            email: String::from("test@example.com"),
            api_key: String::from("test"),
            default_project: current_project,
        });
        app.projects = projects;
        app
    }

    pub fn from_credentials(credentials: JiraCredentials) -> Self {
        let load_result = jira::load_project_issues(&credentials);
        Self::from_load_result(credentials, load_result)
    }

    fn from_load_result(credentials: JiraCredentials, load_result: JiraLoadResult) -> Self {
        let mut app = Self::setup("Loading Jira issues...");
        app.credentials = Some(credentials);
        app.filtered_tree.set_jira_site(
            app.credentials
                .as_ref()
                .expect("credentials set")
                .site
                .clone(),
        );
        load_available_columns(&mut app);
        load_available_projects(&mut app);
        app.command_log.push(load_result.log);
        match load_result.issues {
            Ok(issues) => {
                app.filtered_tree.set_items(tree_items_from_issues(issues));
                app.screen = Screen::Main;
                app.status = String::from("Jira issues loaded");
            }
            Err(error) => {
                app.status = error.0;
            }
        }
        app
    }

    pub fn screen(&self) -> Screen {
        self.screen
    }

    pub fn setup_form(&self) -> &CredentialForm {
        &self.setup
    }

    pub fn current_project(&self) -> &str {
        self.credentials
            .as_ref()
            .map(|c| c.default_project.as_str())
            .unwrap_or("")
    }

    pub fn tick(&mut self, dt: std::time::Duration) {
        self.filtered_tree.tick(dt);
        if let Some(dropdown) = &mut self.project_dropdown {
            dropdown.tick(dt);
        }
        for notification in &mut self.notifications {
            notification.tick(dt);
        }
        self.notifications
            .retain(|notification| !notification.is_expired());
    }

    pub fn is_animating(&self) -> bool {
        self.filtered_tree.is_animating()
            || self.project_dropdown.as_ref().is_some_and(
                crate::components::generic::dropdown::MultiSelectDropdownState::is_animating,
            )
            || self.notifications.iter().any(Notification::is_animating)
    }

    pub fn issues(&self) -> &[TreeItem] {
        self.filtered_tree.items()
    }

    pub fn selected_issue_index(&self) -> usize {
        self.filtered_tree.selected_item_index()
    }

    pub fn issue_scroll_offset(&self) -> usize {
        self.filtered_tree.scroll_offset()
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

    pub fn visible_issue_rows(&self) -> Vec<TreeRow> {
        self.filtered_tree.visible_rows()
    }

    pub fn visible_issue_range(&self, height: usize) -> std::ops::Range<usize> {
        self.filtered_tree.visible_range(height)
    }

    pub fn filtered_tree_view_mode(&self) -> FilteredTreeViewMode {
        self.filtered_tree.view_mode()
    }

    pub fn visible_issue_columns(&self) -> &[crate::JiraIssueColumn] {
        self.filtered_tree.visible_columns()
    }

    pub fn column_dropdown(
        &self,
    ) -> Option<
        &crate::components::generic::dropdown::MultiSelectDropdownState<crate::JiraIssueColumn>,
    > {
        self.filtered_tree.column_dropdown()
    }

    pub fn is_column_dropdown_open(&self) -> bool {
        self.filtered_tree.is_column_dropdown_open()
    }

    pub fn is_column_dropdown_filter_focused(&self) -> bool {
        self.filtered_tree.is_column_dropdown_filter_focused()
    }

    pub fn project_dropdown(
        &self,
    ) -> Option<&crate::components::generic::dropdown::MultiSelectDropdownState<ProjectSummary>>
    {
        self.project_dropdown.as_ref()
    }

    pub fn is_project_dropdown_open(&self) -> bool {
        self.project_dropdown.is_some()
    }

    pub fn is_project_dropdown_filter_focused(&self) -> bool {
        self.project_dropdown.as_ref().is_some_and(
            crate::components::generic::dropdown::MultiSelectDropdownState::is_filter_focused,
        )
    }

    pub fn is_input_focused(&self) -> bool {
        self.is_filter_focused()
            || self.is_column_dropdown_filter_focused()
            || self.is_project_dropdown_filter_focused()
    }

    pub fn command_log_entries(&self) -> &[CommandLogEntry] {
        &self.command_log
    }

    pub fn notifications(&self) -> &[Notification] {
        &self.notifications
    }

    pub fn is_command_log_open(&self) -> bool {
        self.command_log_open
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn active_tab(&self) -> &'static str {
        self.tabs.active_title(APP_TABS).unwrap_or("")
    }

    pub fn active_tab_index(&self) -> usize {
        self.tabs.selected_index()
    }

    pub fn tabs_view_mode(&self) -> crate::components::generic::tabs::TabsViewMode {
        self.tabs.view_mode()
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn handle_key(&mut self, key: KeyEvent, keybindings: &KeyBindings) {
        let typing = match self.screen {
            Screen::Setup => true,
            Screen::Main if self.project_dropdown.is_some() => true,
            Screen::Main if self.filtered_tree.is_column_dropdown_open() => true,
            Screen::Main if self.filtered_tree.is_filter_focused() => true,
            _ => false,
        };

        #[allow(clippy::collapsible_if)]
        if let Some(action) = keybindings.global_action_for(key) {
            let is_ctrl_q =
                key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL);
            if !(typing && matches!(action, Action::Quit) && !is_ctrl_q) {
                self.dispatch(action);
                return;
            }
        }

        match self.screen {
            Screen::Setup => self.dispatch_setup(keybindings.setup_action_for(key)),
            Screen::Main if self.command_log_open => {
                self.dispatch(keybindings.command_log_action_for(key))
            }
            Screen::Main if self.project_dropdown.is_some() => {
                let action = if self.is_project_dropdown_filter_focused() {
                    if key.code == KeyCode::Esc
                        || key.code == KeyCode::Char('[')
                            && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        crate::components::generic::dropdown::DropdownAction::Close
                    } else {
                        crate::components::generic::dropdown::DropdownAction::Filter(
                            keybindings.filter_action_for(key),
                        )
                    }
                } else if key.code == KeyCode::Esc
                    || key.code == KeyCode::Char('[')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    crate::components::generic::dropdown::DropdownAction::Close
                } else {
                    keybindings.project_dropdown_action_for(key)
                };
                self.dispatch(Action::ProjectDropdown(action));
            }
            Screen::Main if self.filtered_tree.is_column_dropdown_open() => {
                let action = if self.filtered_tree.is_column_dropdown_filter_focused() {
                    if key.code == KeyCode::Esc
                        || key.code == KeyCode::Char('[')
                            && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        JiraFilteredTreeAction::Dropdown(
                            crate::components::generic::dropdown::DropdownAction::Close,
                        )
                    } else {
                        JiraFilteredTreeAction::Dropdown(
                            crate::components::generic::dropdown::DropdownAction::Filter(
                                keybindings.filter_action_for(key),
                            ),
                        )
                    }
                } else if let Some(action) = keybindings.column_dropdown_context_action_for(key) {
                    action
                } else {
                    keybindings.dropdown_action_for(key)
                };
                self.dispatch(Action::JiraFilteredTree(action));
            }
            Screen::Main if self.filtered_tree.is_filter_focused() => {
                self.dispatch_filter(keybindings.filter_action_for(key))
            }
            Screen::Main => self.dispatch(keybindings.jira_filtered_tree_action_for(key)),
        }
    }

    pub fn dispatch(&mut self, action: Action) {
        match action {
            Action::Tabs(action) => self.dispatch_tabs(action),
            Action::JiraFilteredTree(action) => self.dispatch_jira_filtered_tree(action),
            Action::ReloadList => self.reload_list(),
            Action::ToggleCommandLog => self.toggle_dialog(DialogKind::CommandLog),
            Action::ToggleProjectDropdown => self.toggle_dropdown(DropdownKind::ProjectSwitcher),
            Action::ProjectDropdown(action) => self.dispatch_project_dropdown(action),
            Action::CloseCommandLog => self.close_dialog(DialogKind::CommandLog),
            Action::Quit => self.running = false,
            Action::None => self.filtered_tree.clear_transient_input(),
        }
    }

    pub fn dispatch_filter(&mut self, action: FilterAction) {
        if let Some(event) = self.filtered_tree.dispatch_filter(action) {
            self.handle_jira_filtered_tree_event(event);
        }
    }

    pub fn dispatch_setup(&mut self, action: SetupAction) {
        use crate::components::generic::input;
        let field_idx = self.setup.active_field;
        match action {
            SetupAction::NextField => self.setup.next_field(),
            SetupAction::PreviousField => self.setup.previous_field(),
            SetupAction::Submit => self.submit_setup(),
            SetupAction::Backspace => self.setup.backspace(),
            SetupAction::Quit => self.running = false,
            SetupAction::Text(c) => self.setup.push(c),
            SetupAction::None => {}
            SetupAction::MoveCursorStart => {
                self.setup.cursors[field_idx] = 0;
            }
            SetupAction::MoveCursorEnd => {
                let val = self.setup.active_value_mut();
                self.setup.cursors[field_idx] = val.chars().count();
            }
            SetupAction::Clear => {
                let val = self.setup.active_value_mut();
                val.clear();
                self.setup.cursors[field_idx] = 0;
            }
            SetupAction::MoveCursorWordLeft => {
                let mut cursor = self.setup.cursors[field_idx];
                let val = self.setup.active_value_mut();
                input::move_word_left(val, &mut cursor);
                self.setup.cursors[field_idx] = cursor;
            }
            SetupAction::MoveCursorWordRight => {
                let mut cursor = self.setup.cursors[field_idx];
                let val = self.setup.active_value_mut();
                input::move_word_right(val, &mut cursor);
                self.setup.cursors[field_idx] = cursor;
            }
            SetupAction::DeleteWordLeft => {
                let mut cursor = self.setup.cursors[field_idx];
                let val = self.setup.active_value_mut();
                input::delete_word_left(val, &mut cursor);
                self.setup.cursors[field_idx] = cursor;
            }
            SetupAction::DeleteWordRight => {
                let cursor = self.setup.cursors[field_idx];
                let val = self.setup.active_value_mut();
                input::delete_word_right(val, cursor);
            }
            SetupAction::MoveCursorLeft => {
                let mut cursor = self.setup.cursors[field_idx];
                input::move_left(&mut cursor);
                self.setup.cursors[field_idx] = cursor;
            }
            SetupAction::MoveCursorRight => {
                let mut cursor = self.setup.cursors[field_idx];
                let val = self.setup.active_value_mut();
                input::move_right(val, &mut cursor);
                self.setup.cursors[field_idx] = cursor;
            }
            SetupAction::DeleteToEnd => {
                let cursor = self.setup.cursors[field_idx];
                let val = self.setup.active_value_mut();
                input::delete_to_end(val, cursor);
            }
            SetupAction::DeleteToStart => {
                let mut cursor = self.setup.cursors[field_idx];
                let val = self.setup.active_value_mut();
                input::delete_to_start(val, &mut cursor);
                self.setup.cursors[field_idx] = cursor;
            }
        }
    }

    fn dispatch_tabs(&mut self, action: TabAction) {
        self.tabs.dispatch(action, APP_TABS);
        self.filtered_tree.clear_transient_input();
        self.close_overlays();
    }

    fn dispatch_jira_filtered_tree(&mut self, action: JiraFilteredTreeAction) {
        if matches!(action, JiraFilteredTreeAction::OpenColumns) {
            self.toggle_dropdown(DropdownKind::JiraColumns);
            return;
        }
        if self.tabs.is_active(APP_TABS, "List")
            && let Some(event) = self.filtered_tree.dispatch(action)
        {
            self.handle_jira_filtered_tree_event(event);
        }
    }

    fn handle_jira_filtered_tree_event(&mut self, event: JiraFilteredTreeEvent) {
        match event {
            JiraFilteredTreeEvent::Quit => self.running = false,
            JiraFilteredTreeEvent::IssueUrlCopied(url) => self
                .notifications
                .push(Notification::success("Issue URL copied", url)),
            JiraFilteredTreeEvent::IssueUrlCopyFailed(message) => self
                .notifications
                .push(Notification::error("Issue URL not copied", message)),
            JiraFilteredTreeEvent::ColumnsChanged(_) => {}
        }
    }

    fn toggle_dialog(&mut self, dialog: DialogKind) {
        if self.is_dialog_open(dialog) {
            self.close_dialog(dialog);
        } else {
            self.open_dialog(dialog);
        }
    }

    fn open_dialog(&mut self, dialog: DialogKind) {
        self.close_overlays();
        match dialog {
            DialogKind::CommandLog => self.command_log_open = true,
        }
    }

    fn close_dialog(&mut self, dialog: DialogKind) {
        match dialog {
            DialogKind::CommandLog => self.command_log_open = false,
        }
    }

    fn is_dialog_open(&self, dialog: DialogKind) -> bool {
        match dialog {
            DialogKind::CommandLog => self.command_log_open,
        }
    }

    fn toggle_dropdown(&mut self, dropdown: DropdownKind) {
        if self.is_dropdown_open(dropdown) {
            self.close_dropdown(dropdown);
        } else {
            self.open_dropdown(dropdown);
        }
    }

    fn open_dropdown(&mut self, dropdown: DropdownKind) {
        match dropdown {
            DropdownKind::JiraColumns => {
                self.close_overlays();
                self.filtered_tree.open_column_dropdown();
            }
            DropdownKind::ProjectSwitcher => self.open_project_dropdown(),
        }
    }

    fn close_dropdown(&mut self, dropdown: DropdownKind) {
        match dropdown {
            DropdownKind::JiraColumns => self.filtered_tree.close_column_dropdown(),
            DropdownKind::ProjectSwitcher => self.project_dropdown = None,
        }
    }

    fn is_dropdown_open(&self, dropdown: DropdownKind) -> bool {
        match dropdown {
            DropdownKind::JiraColumns => self.filtered_tree.is_column_dropdown_open(),
            DropdownKind::ProjectSwitcher => self.project_dropdown.is_some(),
        }
    }

    fn close_dropdowns(&mut self) {
        self.project_dropdown = None;
        self.filtered_tree.close_column_dropdown();
    }

    fn close_dialogs(&mut self) {
        self.command_log_open = false;
    }

    fn close_overlays(&mut self) {
        self.close_dropdowns();
        self.close_dialogs();
    }

    fn open_project_dropdown(&mut self) {
        if self.projects.is_empty() {
            self.status = String::from("No Jira projects available.");
            return;
        }

        self.close_overlays();
        let current_project = self.current_project();
        let options = self
            .projects
            .iter()
            .cloned()
            .map(
                |project| crate::components::generic::dropdown::DropdownOption {
                    selected: project.key == current_project,
                    label: format!("{}  {}", project.key, project.name),
                    value: project,
                },
            )
            .collect();
        self.project_dropdown = Some(
            crate::components::generic::dropdown::MultiSelectDropdownState::new(options)
                .single_select(),
        );
    }

    fn dispatch_project_dropdown(
        &mut self,
        action: crate::components::generic::dropdown::DropdownAction,
    ) {
        let Some(dropdown) = &mut self.project_dropdown else {
            return;
        };
        match dropdown.dispatch(action) {
            Some(crate::components::generic::dropdown::DropdownEvent::Closed) => {
                self.project_dropdown = None;
            }
            Some(crate::components::generic::dropdown::DropdownEvent::Toggled(index)) => {
                let Some(project) = dropdown
                    .options()
                    .get(index)
                    .map(|option| option.value.clone())
                else {
                    return;
                };
                self.project_dropdown = None;
                self.switch_project(project);
            }
            None => {}
        }
    }

    fn switch_project(&mut self, project: ProjectSummary) {
        let Some(mut credentials) = self.credentials.clone() else {
            self.status = String::from("No Jira credentials available for project switch.");
            return;
        };
        if credentials.default_project == project.key {
            return;
        }
        credentials.default_project = project.key.clone();
        if let Err(error) = config::save_jira_credentials(&credentials) {
            self.status = format!("Could not save selected Jira project: {error}");
            return;
        }

        self.status = format!("Loading Jira project {}...", project.key);
        let load_result = jira::load_project_issues(&credentials);
        self.credentials = Some(credentials);
        self.command_log.push(load_result.log);
        match load_result.issues {
            Ok(issues) => {
                self.filtered_tree.set_items(tree_items_from_issues(issues));
                self.status = format!("Jira project {} loaded.", project.key);
            }
            Err(error) => self.status = error.0,
        }
    }

    fn submit_setup(&mut self) {
        let Some(credentials) = self.setup.credentials() else {
            self.status = String::from("All Jira credential fields are required.");
            return;
        };

        if let Err(error) = config::save_jira_credentials(&credentials) {
            self.status = format!("Could not save Jira credentials: {error}");
            return;
        }

        self.status = String::from("Loading Jira issues...");
        let load_result = jira::load_project_issues(&credentials);
        self.credentials = Some(credentials);
        self.filtered_tree.set_jira_site(
            self.credentials
                .as_ref()
                .expect("credentials set")
                .site
                .clone(),
        );
        load_available_columns(self);
        load_available_projects(self);
        self.command_log.push(load_result.log);
        match load_result.issues {
            Ok(issues) => {
                self.filtered_tree.set_items(tree_items_from_issues(issues));
                self.screen = Screen::Main;
                self.status = String::from("Jira credentials saved and issues loaded.");
            }
            Err(error) => self.status = error.0,
        }
    }

    fn reload_list(&mut self) {
        if !self.tabs.is_active(APP_TABS, "List") {
            return;
        }

        let Some(credentials) = self.credentials.clone() else {
            self.status = String::from("No Jira credentials available for reload.");
            return;
        };

        self.status = String::from("Reloading Jira issues...");
        let load_result = jira::load_project_issues(&credentials);
        self.command_log.push(load_result.log);
        match load_result.issues {
            Ok(issues) => {
                self.filtered_tree.set_items(tree_items_from_issues(issues));
                self.status = String::from("Jira issues reloaded.");
            }
            Err(error) => self.status = error.0,
        }
    }
}

fn load_available_projects(app: &mut App) {
    let Some(credentials) = &app.credentials else {
        return;
    };
    let project_result = jira::load_projects(credentials);
    app.command_log.push(project_result.log);
    if let Ok(projects) = project_result.projects {
        app.projects = projects;
    }
}

fn load_available_columns(app: &mut App) {
    let Some(credentials) = &app.credentials else {
        return;
    };
    let field_result = jira::load_issue_fields(credentials);
    app.command_log.push(field_result.log);
    if let Ok(fields) = field_result.fields {
        let mut columns = JiraIssueColumn::default_columns();
        columns.extend(fields.into_iter().filter_map(|field| {
            let is_known = matches!(
                field.id.as_str(),
                "key" | "summary" | "issuetype" | "status"
            );
            (!is_known).then_some(JiraIssueColumn::Field {
                id: field.id,
                label: field.name,
            })
        }));
        app.filtered_tree.set_available_columns(columns);
    }
}

fn tree_items_from_issues(issues: Vec<IssueSummary>) -> Vec<TreeItem> {
    issues
        .into_iter()
        .map(|issue| TreeItem {
            id: issue.key,
            label: issue.summary,
            status: issue.status,
            kind: issue.issue_type.clone(),
            parent_id: issue.parent_key,
            field_values: issue.field_values,
            root_order: 0,
        })
        .collect()
}

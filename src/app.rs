use crate::{
    components::{
        generic::{
            dropdown::{
                DropdownEvent, DropdownOption, DropdownVisibleOption, MultiSelectDropdownState,
            },
            filter::FilterAction,
            filtered_tree::FilteredTreeViewMode,
            notification::Notification,
            tabs::{TabAction, TabsState},
            tree::{Children, TreeItem, TreeRow},
        },
        jira::filtered_tree::{
            JiraFilteredTreeAction, JiraFilteredTreeEvent, JiraFilteredTreeState, JiraIssueColumn,
        },
    },
    config::JiraCredentials,
    keymap::KeyBindings,
    services::jira::{
        BoardData, CommandLogEntry, FieldSummary, IssueSummary, JiraError, ProjectSummary,
        UserSummary,
    },
    ui::theme::{Theme, ThemeChoice},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

/// The tab titles in display order, derived from [`ApplicationTab`] so the enum
/// is the single source of truth. Passed to the generic, string/index-based
/// `TabsState`.
pub fn app_tabs() -> [&'static str; 3] {
    ApplicationTab::all().map(|tab| tab.title())
}
const DEFAULT_TAB_INDEX: usize = 1;

mod action;
mod board;
mod command_log;
mod dropdown;
mod effect;
mod event;
mod focus;
mod keys;
mod list;
mod modal;
mod mouse;
mod setup;
mod tab;

pub use action::{Action, BoardAction};
pub use tab::ApplicationTab;
pub use board::{BoardGrouping, BoardState, board_issue_column};
pub(crate) use board::{
    board_assignee_value_matches, board_empty_cell_key, board_group_key, board_grouped_lanes,
    board_issue_matches_search, board_value_matches, normalize_board_user_fields,
};
use command_log::CommandLogView;
pub use dropdown::QuickAction;
use dropdown::{DropdownKind, Overlay};
pub use effect::{AppEffect, AppEvent, JiraLoadPurpose, JiraProjectLoadResult};
use modal::{DialogKind, ModalState};
pub use setup::{CredentialField, CredentialForm, SetupAction};
use setup::Spinner;

/// Which content the List screen is showing.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ListView {
    /// Browsing the paginated root issue tree.
    Browse,
    /// Showing flat server-side search results for this term.
    Search(String),
}

impl ListView {
    /// Whether this is a search view for exactly `term` (no allocation).
    fn is_searching_for(&self, term: &str) -> bool {
        matches!(self, Self::Search(current) if current == term)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Setup,
    Main,
}

#[derive(Clone, PartialEq, Eq)]
pub struct App {
    tabs: TabsState,
    running: bool,
    screen: Screen,
    setup: CredentialForm,
    filtered_tree: JiraFilteredTreeState,
    board: BoardState,
    board_go_to_start_pending: bool,
    board_grouping: BoardGrouping,
    credentials: Option<JiraCredentials>,
    command_log: Vec<CommandLogEntry>,
    modal: Option<ModalState>,
    board_filter: crate::FilterState,
    status: String,
    notifications: Vec<Notification>,
    projects: Vec<ProjectSummary>,
    users: Vec<UserSummary>,
    current_user: Option<UserSummary>,
    overlay: Option<Overlay>,
    theme_preview_origin: Option<Theme>,
    leader_pending: bool,
    pending_effects: Vec<AppEffect>,
    theme: Theme,
    active_load_request_id: Option<u64>,
    next_request_id: u64,
    pending_assignment_requests: std::collections::BTreeMap<String, u64>,
    /// Whether the list is browsing the project tree or showing search results.
    view: ListView,
    /// Cursor for the next page of root issues while browsing; `None` when the
    /// last page has been loaded or a load is not in progress.
    next_root_page_token: Option<String>,
    /// Request id of the in-flight root page load, if any.
    pending_roots_request_id: Option<u64>,
    /// Request id of the most recent search; only its result is applied.
    search_request_id: Option<u64>,
    /// The search term that produced the issues currently displayed. Highlights
    /// match this, not the live filter input, so they only change when results
    /// arrive. `None` while browsing.
    applied_search_term: Option<String>,
    /// In-flight child-load request ids keyed by parent issue key.
    pending_child_requests: std::collections::BTreeMap<String, u64>,
    /// Issue ids whose expansion should be restored as the tree reloads. Seeded
    /// when a full or node reload starts; consumed incrementally as nodes
    /// reappear and their children arrive, so nested expanded subtrees are
    /// re-fetched in parallel and re-opened.
    expansion_to_restore: std::collections::HashSet<String>,
    /// Parent ids whose children are being refreshed in place by a seamless
    /// reload. Their `ChildrenLoaded` results are merged (not appended) so the
    /// stale subtree is swapped without collapsing or moving the view.
    soft_reload_parents: std::collections::HashSet<String>,
    /// While a paginated seamless reload collects root pages, accumulates every
    /// root id seen so far. Once paging finishes, roots absent from this set
    /// (deleted server-side) are pruned. `None` outside a reload.
    reload_root_ids: Option<std::collections::HashSet<String>>,
    /// Set when the next project load should be applied as a seamless in-place
    /// reload (`Shift+R`) — merging into the current browse tree rather than
    /// rebuilding it. Other project loads (initial, project switch, returning
    /// from search) rebuild from scratch and leave this `false`.
    pending_reload_seamless: bool,
    /// Children results that arrive before the root query during a seamless
    /// reload. Held here so the whole reload settles as one unit when the roots
    /// land, instead of un-dimming nodes mid-reload (which looks like a flicker).
    reload_children_buffer: Vec<(String, Result<Vec<IssueSummary>, JiraError>)>,
    /// Animated spinner for in-flight node loads.
    spinner: Spinner,
    /// Scroll/follow state for the command-log dialog.
    command_log_view: CommandLogView,
    /// Monotonic wall-clock accumulator driving idle UI animations (e.g. the
    /// splash logo).
    anim_clock: std::time::Duration,
    /// True from launch (when credentials already exist) until the first
    /// `Initial` project load resolves. While set, the UI shows only the
    /// animated splash logo instead of the main view.
    awaiting_initial_load: bool,
    /// A search term waiting out its debounce window, with the time left before
    /// it fires. Reset on each keystroke so only a pause in typing triggers the
    /// server search; `None` when no search is pending.
    pending_search: Option<(String, std::time::Duration)>,
}

impl Default for App {
    fn default() -> Self {
        Self::setup("No Jira credentials found. Enter them to save config and load Jira issues.")
    }
}

impl App {
    fn base(
        screen: Screen,
        filtered_tree: JiraFilteredTreeState,
        board: BoardState,
        status: String,
    ) -> Self {
        Self {
            tabs: TabsState::new(DEFAULT_TAB_INDEX),
            running: true,
            screen,
            setup: CredentialForm::default(),
            filtered_tree,
            board,
            board_go_to_start_pending: false,
            board_grouping: BoardGrouping::None,
            board_filter: crate::FilterState::default(),
            credentials: None,
            command_log: Vec::new(),
            modal: None,
            status,
            notifications: Vec::new(),
            projects: Vec::new(),
            users: Vec::new(),
            current_user: None,
            overlay: None,
            theme_preview_origin: None,
            leader_pending: false,
            pending_effects: Vec::new(),
            theme: Theme::default(),
            active_load_request_id: None,
            next_request_id: 1,
            pending_assignment_requests: std::collections::BTreeMap::new(),
            view: ListView::Browse,
            next_root_page_token: None,
            pending_roots_request_id: None,
            search_request_id: None,
            applied_search_term: None,
            pending_child_requests: std::collections::BTreeMap::new(),
            expansion_to_restore: std::collections::HashSet::new(),
            soft_reload_parents: std::collections::HashSet::new(),
            reload_root_ids: None,
            pending_reload_seamless: false,
            reload_children_buffer: Vec::new(),
            spinner: Spinner::default(),
            command_log_view: CommandLogView::default(),
            anim_clock: std::time::Duration::ZERO,
            awaiting_initial_load: false,
            pending_search: None,
        }
    }

    pub fn setup(status: impl Into<String>) -> Self {
        let mut filtered_tree = JiraFilteredTreeState::new(Vec::new());
        filtered_tree.set_view_mode(FilteredTreeViewMode::Table);

        Self::base(
            Screen::Setup,
            filtered_tree,
            BoardState::empty(),
            status.into(),
        )
    }

    pub fn with_issues(issues: Vec<IssueSummary>) -> Self {
        let board = BoardState::from_issues(issues.clone());
        let mut filtered_tree = JiraFilteredTreeState::new(tree_items_from_issues(issues));
        filtered_tree.set_view_mode(FilteredTreeViewMode::Table);

        Self::base(
            Screen::Main,
            filtered_tree,
            board,
            String::from("Jira issues loaded"),
        )
    }

    pub fn with_board_data(board: BoardData) -> Self {
        let mut app = Self::with_issues(board.issues.clone());
        app.board.set_data(board);
        app
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

    pub fn with_issues_projects_and_users(
        issues: Vec<IssueSummary>,
        projects: Vec<ProjectSummary>,
        users: Vec<UserSummary>,
        current_project: impl Into<String>,
    ) -> Self {
        let mut app = Self::with_issues_and_projects(issues, projects, current_project);
        app.current_user = users.first().cloned();
        app.users = users;
        app
    }

    pub fn from_credentials(credentials: JiraCredentials) -> Self {
        let mut app = Self::setup("Loading Jira issues");
        app.screen = Screen::Main;
        app.awaiting_initial_load = true;
        app.credentials = Some(credentials.clone());
        app.filtered_tree.set_jira_site(credentials.site.clone());
        app.queue_jira_load(
            JiraLoadPurpose::Initial,
            credentials,
            crate::services::jira::ROOT_PAGE_SIZE,
        );
        app
    }

    pub fn screen(&self) -> Screen {
        self.screen
    }

    pub fn help_selected(&self) -> usize {
        match &self.modal {
            Some(ModalState::Help { selected }) => *selected,
            _ => 0,
        }
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
        self.anim_clock += dt;
        self.advance_pending_search(dt);
        self.filtered_tree.tick(dt);
        if let Some(overlay) = &mut self.overlay {
            overlay.tick(dt);
        }
        if self.is_loading() {
            self.spinner.tick(dt);
        }
        self.board.v_scroll.tick(dt);
        self.board.h_scroll.tick(dt);
        for notification in &mut self.notifications {
            notification.tick(dt);
        }
        self.notifications
            .retain(|notification| !notification.is_expired());
    }

    /// The current spinner glyph for rendering in-flight loads.
    pub fn spinner_glyph(&self) -> &'static str {
        self.spinner.glyph()
    }

    /// Monotonic wall-clock elapsed since launch, driving idle UI animations.
    pub fn anim_elapsed(&self) -> std::time::Duration {
        self.anim_clock
    }

    /// Whether the initial project load is still in flight at launch, so the UI
    /// should show only the animated splash logo instead of the main view.
    pub fn is_loading_splash(&self) -> bool {
        self.awaiting_initial_load
    }

    /// Whether a Jira request is in flight that the user is waiting on: the
    /// initial/reload/project load, a server search, a node child fetch, or a
    /// lazy root-pagination page. The lazy page is included so the footer
    /// spinner runs while "load more on scroll" fetches the next page.
    pub fn is_loading(&self) -> bool {
        self.active_load_request_id.is_some()
            || self.search_request_id.is_some()
            || self.pending_roots_request_id.is_some()
            || self.filtered_tree.any_loading()
    }

    pub fn is_animating(&self) -> bool {
        self.filtered_tree.is_animating()
            || self.overlay.as_ref().is_some_and(Overlay::is_animating)
            || self.notifications.iter().any(Notification::is_animating)
            || self.board.v_scroll.is_animating()
            || self.board.h_scroll.is_animating()
            || self.pending_search.is_some()
            || self.is_loading()
    }

    pub fn take_effects(&mut self) -> Vec<AppEffect> {
        std::mem::take(&mut self.pending_effects)
    }

    pub fn issues(&self) -> &[TreeItem] {
        self.filtered_tree.items()
    }

    pub fn selected_issue_index(&self) -> usize {
        self.filtered_tree.selected_item_index()
    }

    pub fn board_grouping(&self) -> BoardGrouping {
        self.board_grouping
    }

    pub fn board_group_dropdown(&self) -> Option<&MultiSelectDropdownState<BoardGrouping>> {
        match &self.overlay {
            Some(Overlay::BoardGroup(dropdown)) => Some(dropdown),
            _ => None,
        }
    }

    pub fn selected_issue_key(&self) -> Option<&str> {
        self.filtered_tree.selected_item_id()
    }

    pub fn board(&self) -> &BoardState {
        &self.board
    }

    pub fn selected_board_issue_key(&self) -> Option<&str> {
        self.board.selected_issue_key()
    }

    pub fn selected_board_issue_index(&self) -> usize {
        self.board
            .selected_issue_index(self.board_filter(), self.board_grouping)
    }

    pub fn selected_board_group(&self) -> Option<&str> {
        self.board.selected_group()
    }

    /// The raw selected board key (issue, group, or empty cell) for scrolling.
    pub fn selected_board_raw_key(&self) -> Option<&str> {
        self.board.selected_raw_key()
    }

    /// The `(group, column)` of a focused empty column, if one is selected.
    pub fn selected_board_empty_cell(&self) -> Option<(&str, usize)> {
        self.board.selected_empty_cell()
    }

    pub fn is_board_group_collapsed(&self, group: &str) -> bool {
        self.board.is_group_collapsed(group)
    }

    /// The term highlights should match: the search term that produced the
    /// currently displayed results, not the live filter input. Empty while
    /// browsing or before the first search result arrives, so stale rows are
    /// never highlighted against a not-yet-applied query.
    pub fn highlight_term(&self) -> &str {
        self.applied_search_term.as_deref().unwrap_or("")
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

    pub fn board_filter(&self) -> &str {
        self.board_filter.value()
    }

    pub fn board_filter_cursor(&self) -> usize {
        self.board_filter.cursor()
    }

    pub fn board_filter_state(&self) -> &crate::FilterState {
        &self.board_filter
    }

    pub fn is_board_filter_focused(&self) -> bool {
        self.board_filter.is_focused()
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

    pub(crate) fn selected_assignment_issue_key(&self) -> Option<&str> {
        if self.active_tab() == ApplicationTab::Board {
            self.selected_board_issue_key()
        } else {
            self.selected_issue_key()
        }
    }

    pub(crate) fn selected_assignment_assignee(&self, issue_key: &str) -> Option<String> {
        if self.active_tab() == ApplicationTab::Board {
            return self
                .board
                .data()
                .and_then(|data| data.issues.iter().find(|issue| issue.key == issue_key))
                .and_then(|issue| issue.field_values.get("assignee"))
                .cloned();
        }
        self.issues()
            .iter()
            .find(|item| item.id == issue_key)
            .and_then(|item| item.field_values.get("assignee"))
            .cloned()
    }

    pub fn filtered_tree_view_mode(&self) -> FilteredTreeViewMode {
        self.filtered_tree.view_mode()
    }

    pub fn visible_issue_columns(&self) -> &[crate::JiraIssueColumn] {
        self.filtered_tree.visible_columns()
    }

    /// Pans the issue table horizontally by `delta` cells (shift/horizontal
    /// wheel). No-op effect until the next render advances the glide.
    pub fn scroll_table_horizontal(&self, delta: i32) {
        self.filtered_tree.scroll_table_horizontal(delta);
    }

    /// Resolves the table's animated horizontal offset for this frame, clamped
    /// to `max_offset` cells. Called once per render before slicing rows.
    pub fn resolve_table_h_offset(&self, max_offset: u16) -> u16 {
        self.filtered_tree.resolve_table_h_offset(max_offset)
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

    pub fn assignee_dropdown(&self) -> Option<&MultiSelectDropdownState<Option<UserSummary>>> {
        match &self.overlay {
            Some(Overlay::Assignee(dropdown)) => Some(dropdown),
            _ => None,
        }
    }

    fn overlay_filter_focused(&self) -> bool {
        self.overlay.as_ref().is_some_and(Overlay::is_filter_focused)
    }

    pub fn is_assignee_dropdown_filter_focused(&self) -> bool {
        self.overlay_filter_focused()
    }

    pub fn project_dropdown(&self) -> Option<&MultiSelectDropdownState<ProjectSummary>> {
        match &self.overlay {
            Some(Overlay::Project(dropdown)) => Some(dropdown),
            _ => None,
        }
    }

    pub fn theme_dropdown(&self) -> Option<&MultiSelectDropdownState<ThemeChoice>> {
        match &self.overlay {
            Some(Overlay::Theme(dropdown)) => Some(dropdown),
            _ => None,
        }
    }

    pub fn quick_switcher(&self) -> Option<&MultiSelectDropdownState<QuickAction>> {
        match &self.overlay {
            Some(Overlay::Quick(dropdown)) => Some(dropdown),
            _ => None,
        }
    }

    pub fn is_board_group_dropdown_filter_focused(&self) -> bool {
        self.overlay_filter_focused()
    }

    pub fn is_quick_switcher_filter_focused(&self) -> bool {
        self.overlay_filter_focused()
    }

    pub fn is_help_open(&self) -> bool {
        matches!(self.modal, Some(ModalState::Help { .. }))
    }

    /// Whether an open dropdown should render its filter cursor: hidden while the
    /// help or command-log dialog is up, since those capture input focus.
    pub fn dropdown_cursor_visible(&self) -> bool {
        !self.is_help_open() && !self.is_command_log_open()
    }

    pub fn is_project_dropdown_filter_focused(&self) -> bool {
        self.overlay_filter_focused()
    }

    pub fn is_theme_dropdown_filter_focused(&self) -> bool {
        self.overlay_filter_focused()
    }

    pub fn is_input_focused(&self) -> bool {
        matches!(self.input_mode(), focus::InputMode::Input)
    }

    pub fn command_log_entries(&self) -> &[CommandLogEntry] {
        &self.command_log
    }

    pub fn notifications(&self) -> &[Notification] {
        &self.notifications
    }

    pub fn is_command_log_open(&self) -> bool {
        matches!(self.modal, Some(ModalState::CommandLog))
    }

    pub fn is_sprint_details_open(&self) -> bool {
        matches!(self.modal, Some(ModalState::SprintDetails))
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn active_tab(&self) -> ApplicationTab {
        ApplicationTab::from_index(self.tabs.selected_index()).unwrap_or(ApplicationTab::List)
    }

    pub fn active_tab_index(&self) -> usize {
        self.tabs.selected_index()
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    pub fn tabs_view_mode(&self) -> crate::components::generic::tabs::TabsViewMode {
        self.tabs.view_mode()
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn dispatch(&mut self, action: Action) {
        match action {
            Action::Tabs(action) => self.dispatch_tabs(action),
            Action::JiraFilteredTree(action) => self.dispatch_jira_filtered_tree(action),
            Action::ScrollListHorizontal(delta) => {
                if self.filtered_tree.view_mode() == FilteredTreeViewMode::Table {
                    self.scroll_table_horizontal(delta);
                }
            }
            Action::Board(BoardAction::GoToStartPrefix) => {
                if self.board_go_to_start_pending {
                    let search = self.board_filter.value().to_owned();
                    self.board
                        .dispatch(BoardAction::GoToStart, &search, self.board_grouping);
                    self.board_go_to_start_pending = false;
                } else {
                    self.board_go_to_start_pending = true;
                }
            }
            Action::Board(action) => {
                self.board_go_to_start_pending = false;
                let search = self.board_filter.value().to_owned();
                self.board.dispatch(action, &search, self.board_grouping);
            }
            Action::FocusBoardFilter => self.board_filter.focus(),
            Action::ClearBoardFilter => {
                if !self.board_filter.value().is_empty() {
                    self.board_filter.clear();
                    self.board.select_first("", self.board_grouping);
                }
            }
            Action::ReloadList => self.reload_list(),
            Action::ReloadBoard => self.reload_board(),
            Action::ReloadNode => self.reload_node(),
            Action::Leader => self.leader_pending = true,
            Action::ToggleCommandLog => self.toggle_dialog(DialogKind::CommandLog),
            Action::ToggleSprintDetails => self.toggle_dialog(DialogKind::SprintDetails),
            Action::CloseSprintDetails => self.close_dialog(DialogKind::SprintDetails),
            Action::ToggleQuickSwitcher => self.toggle_dropdown(DropdownKind::QuickSwitcher),
            Action::ToggleProjectDropdown => self.toggle_dropdown(DropdownKind::ProjectSwitcher),
            Action::ToggleThemeDropdown => self.toggle_dropdown(DropdownKind::ThemePicker),
            Action::ToggleAssigneeDropdown => self.toggle_dropdown(DropdownKind::AssigneePicker),
            Action::ToggleBoardGrouping => self.toggle_dropdown(DropdownKind::BoardGroup),
            Action::BoardGroupDropdown(action) => self.dispatch_board_group_dropdown(action),
            Action::AssignSelectedToMe => self.assign_selected_to_me(),
            Action::UnassignSelected => self.queue_selected_assignment(None),
            Action::GoToBoard => self.select_tab(ApplicationTab::Board),
            Action::GoToList => self.select_tab(ApplicationTab::List),
            Action::GoToTimeline => self.select_tab(ApplicationTab::Timeline),
            Action::OpenHelp => self.open_dialog(DialogKind::Help),
            Action::CloseHelp => self.close_dialog(DialogKind::Help),
            Action::QuickSwitcher(action) => self.dispatch_quick_switcher(action),
            Action::ProjectDropdown(action) => self.dispatch_project_dropdown(action),
            Action::ThemeDropdown(action) => self.dispatch_theme_dropdown(action),
            Action::AssigneeDropdown(action) => self.dispatch_assignee_dropdown(action),
            Action::CloseCommandLog => self.close_dialog(DialogKind::CommandLog),
            Action::ScrollCommandLog(delta) => self.scroll_command_log(delta),
            Action::PageCommandLog(direction) => self.page_command_log(direction),
            Action::HalfPageCommandLog(direction) => self.half_page_command_log(direction),
            Action::CommandLogToStart => self.command_log_to_start(),
            Action::CommandLogToStartPrefix => self.command_log_arm_go_to_start(),
            Action::CommandLogToEnd => self.command_log_to_end(),
            Action::Quit => self.running = false,
            Action::None => self.filtered_tree.clear_transient_input(),
        }
    }

    pub fn dispatch_filter(&mut self, action: FilterAction) {
        if let Some(event) = self.filtered_tree.dispatch_filter(action) {
            self.handle_jira_filtered_tree_event(event);
        }
    }

    fn dispatch_tabs(&mut self, action: TabAction) {
        self.tabs.dispatch(action, &app_tabs());
        self.filtered_tree.clear_transient_input();
        self.close_overlays();
    }

    fn dispatch_jira_filtered_tree(&mut self, action: JiraFilteredTreeAction) {
        if matches!(action, JiraFilteredTreeAction::OpenColumns) {
            self.toggle_dropdown(DropdownKind::JiraColumns);
            return;
        }
        let can_dispatch = self.active_tab() == ApplicationTab::List
            || self.active_tab() == ApplicationTab::Board
                && matches!(action, JiraFilteredTreeAction::FilteredTree(_));
        if can_dispatch && let Some(event) = self.filtered_tree.dispatch(action) {
            self.handle_jira_filtered_tree_event(event);
        }
        // Navigating toward the bottom of the list lazily pulls the next page.
        // Height 0: keyboard nav moves the selection, which is the bottom signal.
        self.maybe_prefetch_more_roots(0);
    }

    fn handle_jira_filtered_tree_event(&mut self, event: JiraFilteredTreeEvent) {
        match event {
            JiraFilteredTreeEvent::Quit => self.running = false,
            JiraFilteredTreeEvent::IssueUrlCopyRequested(url) => {
                self.pending_effects.push(AppEffect::CopyToClipboard(url));
            }
            JiraFilteredTreeEvent::IssueUrlCopyUnavailable(message) => self
                .notifications
                .push(Notification::error("Issue URL not copied", message)),
            JiraFilteredTreeEvent::ColumnsChanged(_) => self.reload_current_view_fields(),
            JiraFilteredTreeEvent::LoadChildren(parent_key) => self.request_children(parent_key),
            JiraFilteredTreeEvent::FilterChanged(term) => self.queue_search(term),
            JiraFilteredTreeEvent::FilterCleared => self.restore_browse_view(),
        }
    }

    fn toggle_dialog(&mut self, dialog: DialogKind) {
        if self.is_dialog_open(dialog) {
            self.close_dialog(dialog);
        } else {
            self.open_dialog(dialog);
        }
    }

    pub(crate) fn open_dialog(&mut self, dialog: DialogKind) {
        // Opening a non-help dialog also dismisses any open dropdown; help opens
        // on top of the current view (dropdowns aside) like before. Replacing the
        // single `modal` slot dismisses any other dialog automatically.
        if !matches!(dialog, DialogKind::Help) {
            self.close_dropdowns();
        }
        self.modal = Some(match dialog {
            DialogKind::CommandLog => ModalState::CommandLog,
            DialogKind::SprintDetails => ModalState::SprintDetails,
            DialogKind::Help => ModalState::Help { selected: 0 },
        });
        if matches!(dialog, DialogKind::CommandLog) {
            // Open scrolled to the latest entry at the bottom.
            self.command_log_view.follow.set(true);
            self.command_log_view.offset.set(0);
            self.command_log_view.go_to_start_pending.set(false);
        }
    }

    fn close_dialog(&mut self, dialog: DialogKind) {
        if self.is_dialog_open(dialog) {
            self.modal = None;
        }
    }

    fn is_dialog_open(&self, dialog: DialogKind) -> bool {
        self.modal.as_ref().map(ModalState::kind) == Some(dialog)
    }

    fn close_dialogs(&mut self) {
        self.modal = None;
    }

    pub(crate) fn close_overlays(&mut self) {
        self.close_dropdowns();
        self.close_dialogs();
    }

    fn assign_selected_to_me(&mut self) {
        let Some(current_user) = self.current_user.clone() else {
            self.status = String::from("Current Jira user is not loaded.");
            return;
        };
        self.queue_selected_assignment(Some(current_user));
    }

    pub(crate) fn queue_selected_assignment(&mut self, assignee: Option<UserSummary>) {
        let Some(issue_key) = self.selected_assignment_issue_key().map(str::to_owned) else {
            self.status = String::from("No issue selected.");
            return;
        };
        let Some(credentials) = self.credentials.clone() else {
            self.status = String::from("No Jira credentials available for assignment.");
            return;
        };

        self.status = match assignee.as_ref() {
            Some(user) => format!("Assigning {issue_key} to {}", user.display_name),
            None => format!("Unassigning {issue_key}"),
        };
        let request_id = self.next_request_id();
        self.pending_assignment_requests
            .insert(issue_key.clone(), request_id);
        self.pending_effects.push(AppEffect::AssignIssue {
            request_id,
            issue_key,
            assignee,
            credentials,
        });
    }
}

fn tree_items_from_issues(issues: Vec<IssueSummary>) -> Vec<TreeItem> {
    issues.into_iter().map(tree_item_from_issue).collect()
}

fn tree_item_from_issue(issue: IssueSummary) -> TreeItem {
    TreeItem {
        id: issue.key,
        label: issue.summary,
        status: issue.status,
        kind: issue.issue_type,
        parent_id: issue.parent_key,
        field_values: issue.field_values,
        root_order: 0,
        children: if issue.has_children {
            Children::NotLoaded
        } else {
            Children::Unknown
        },
    }
}

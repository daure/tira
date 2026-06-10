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
use std::fmt;

pub const APP_TABS: &[&str] = &["Board", "List", "Timeline", "Filters"];
const DEFAULT_TAB_INDEX: usize = 1;

mod action;
mod board;
mod effect;
mod list;
mod modal;

pub use action::{Action, BoardAction};
pub use board::{BoardGrouping, BoardState, board_issue_column};
pub(crate) use board::{
    board_empty_cell_key, board_group_key, board_grouped_lanes, normalize_board_user_fields,
};
pub use effect::{AppEffect, AppEvent, JiraLoadPurpose, JiraProjectLoadResult};
use modal::{DialogKind, ModalState};

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

/// Braille dot frames for the loading spinner, cycling clockwise.
const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
/// Plain-icon fallback frames (no Nerd/Unicode braille).
const SPINNER_FRAMES_PLAIN: [&str; 4] = ["|", "/", "-", "\\"];
/// Wall-clock time each spinner frame is shown.
const SPINNER_FRAME_INTERVAL: std::time::Duration = std::time::Duration::from_millis(80);

/// Tracks the current animated-spinner frame, advanced by elapsed wall time.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Spinner {
    frame: usize,
    elapsed: std::time::Duration,
}

impl Spinner {
    /// Advances the frame by the number of whole intervals in `dt`.
    fn tick(&mut self, dt: std::time::Duration) {
        self.elapsed += dt;
        while self.elapsed >= SPINNER_FRAME_INTERVAL {
            self.elapsed -= SPINNER_FRAME_INTERVAL;
            self.frame = self.frame.wrapping_add(1);
        }
    }

    /// The glyph for the current frame, honoring the plain-icon preference.
    fn glyph(&self) -> &'static str {
        if crate::ui::theme::prefers_plain_icons() {
            SPINNER_FRAMES_PLAIN[self.frame % SPINNER_FRAMES_PLAIN.len()]
        } else {
            SPINNER_FRAMES[self.frame % SPINNER_FRAMES.len()]
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DropdownKind {
    JiraColumns,
    QuickSwitcher,
    ProjectSwitcher,
    ThemePicker,
    AssigneePicker,
    BoardGroup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickAction {
    CommandLog,
    ThemePicker,
    ProjectPicker,
    ReloadList,
    ReloadBoard,
    Board,
    List,
    Timeline,
    Filters,
}

impl crate::ui::selector::HasShortcut for QuickAction {
    fn shortcut(&self, keybindings: &crate::KeyBindings) -> Option<String> {
        Some(keybindings.quick_action_shortcut_label(*self))
    }
}

impl crate::ui::selector::HasShortcut for Option<UserSummary> {
    fn shortcut(&self, _keybindings: &crate::KeyBindings) -> Option<String> {
        None
    }
}

impl QuickAction {
    pub fn label(self) -> String {
        match self {
            Self::CommandLog => "Command log",
            Self::ThemePicker => "Theme picker",
            Self::ProjectPicker => "Project picker",
            Self::ReloadList => "Reload list",
            Self::ReloadBoard => "Reload board",
            Self::Board => "Go to Board",
            Self::List => "Go to List",
            Self::Timeline => "Go to Timeline",
            Self::Filters => "Go to Filters",
        }
        .to_owned()
    }
}

fn merge_board_issue_fields(board: &mut BoardData, list_issues: &[IssueSummary]) {
    for board_issue in &mut board.issues {
        let Some(list_issue) = list_issues
            .iter()
            .find(|list_issue| list_issue.key == board_issue.key)
        else {
            continue;
        };
        for (key, value) in &list_issue.field_values {
            if matches!(key.as_str(), "assignee" | "reporter") {
                board_issue.field_values.insert(key.clone(), value.clone());
            } else {
                board_issue
                    .field_values
                    .entry(key.clone())
                    .or_insert_with(|| value.clone());
            }
        }
    }
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
    Delete,
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

#[derive(Clone, PartialEq, Eq)]
pub struct CredentialForm {
    site: String,
    email: String,
    api_key: String,
    default_project: String,
    active_field: usize,
    cursors: [usize; 4],
}

impl fmt::Debug for CredentialForm {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialForm")
            .field("site", &self.site)
            .field("email", &self.email)
            .field("api_key", &"<redacted>")
            .field("default_project", &self.default_project)
            .field("active_field", &self.active_field)
            .field("cursors", &self.cursors)
            .finish()
    }
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

/// View state for the scrollable command-log dialog. Scroll position is kept
/// as a wrapped-line offset from the top; `follow` pins the viewport to the
/// latest entry (the bottom). The `last_*` metrics are written by the renderer
/// so keyboard/mouse scrolling can clamp without re-deriving the dialog width.
#[derive(Clone, PartialEq, Eq, Default)]
struct CommandLogView {
    offset: std::cell::Cell<usize>,
    follow: std::cell::Cell<bool>,
    last_total: std::cell::Cell<usize>,
    last_viewport: std::cell::Cell<usize>,
    /// Set after a single `g`, so the next `g` jumps to the top (`gg`).
    go_to_start_pending: std::cell::Cell<bool>,
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
    board_group_dropdown: Option<MultiSelectDropdownState<BoardGrouping>>,
    credentials: Option<JiraCredentials>,
    command_log: Vec<CommandLogEntry>,
    modal: Option<ModalState>,
    board_filter: crate::FilterState,
    status: String,
    notifications: Vec<Notification>,
    projects: Vec<ProjectSummary>,
    users: Vec<UserSummary>,
    current_user: Option<UserSummary>,
    assignee_dropdown: Option<MultiSelectDropdownState<Option<UserSummary>>>,
    project_dropdown: Option<MultiSelectDropdownState<ProjectSummary>>,
    theme_dropdown: Option<MultiSelectDropdownState<ThemeChoice>>,
    quick_switcher: Option<MultiSelectDropdownState<QuickAction>>,
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
            board: BoardState::empty(),
            board_go_to_start_pending: false,
            board_grouping: BoardGrouping::None,
            board_group_dropdown: None,
            board_filter: crate::FilterState::default(),
            credentials: None,
            command_log: Vec::new(),
            modal: None,
            status: status.into(),
            notifications: Vec::new(),
            projects: Vec::new(),
            users: Vec::new(),
            current_user: None,
            assignee_dropdown: None,
            project_dropdown: None,
            theme_dropdown: None,
            quick_switcher: None,
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
        }
    }

    pub fn with_issues(issues: Vec<IssueSummary>) -> Self {
        let board = BoardState::from_issues(issues.clone());
        let mut filtered_tree = JiraFilteredTreeState::new(tree_items_from_issues(issues));
        filtered_tree.set_view_mode(FilteredTreeViewMode::Table);

        Self {
            tabs: TabsState::new(DEFAULT_TAB_INDEX),
            running: true,
            screen: Screen::Main,
            setup: CredentialForm::default(),
            filtered_tree,
            board,
            board_go_to_start_pending: false,
            board_grouping: BoardGrouping::None,
            board_group_dropdown: None,
            board_filter: crate::FilterState::default(),
            credentials: None,
            command_log: Vec::new(),
            modal: None,
            status: String::from("Jira issues loaded"),
            notifications: Vec::new(),
            projects: Vec::new(),
            users: Vec::new(),
            current_user: None,
            assignee_dropdown: None,
            project_dropdown: None,
            theme_dropdown: None,
            quick_switcher: None,
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
        }
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
        self.filtered_tree.tick(dt);
        if let Some(dropdown) = &mut self.project_dropdown {
            dropdown.tick(dt);
        }
        if let Some(dropdown) = &mut self.theme_dropdown {
            dropdown.tick(dt);
        }
        if let Some(dropdown) = &mut self.quick_switcher {
            dropdown.tick(dt);
        }
        if let Some(dropdown) = &mut self.assignee_dropdown {
            dropdown.tick(dt);
        }
        if let Some(dropdown) = &mut self.board_group_dropdown {
            dropdown.tick(dt);
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
            || self
                .project_dropdown
                .as_ref()
                .is_some_and(MultiSelectDropdownState::is_animating)
            || self
                .theme_dropdown
                .as_ref()
                .is_some_and(MultiSelectDropdownState::is_animating)
            || self
                .quick_switcher
                .as_ref()
                .is_some_and(MultiSelectDropdownState::is_animating)
            || self
                .assignee_dropdown
                .as_ref()
                .is_some_and(MultiSelectDropdownState::is_animating)
            || self
                .board_group_dropdown
                .as_ref()
                .is_some_and(MultiSelectDropdownState::is_animating)
            || self.notifications.iter().any(Notification::is_animating)
            || self.board.v_scroll.is_animating()
            || self.board.h_scroll.is_animating()
            || self.is_loading()
            || matches!(self.active_tab(), "Timeline" | "Filters")
    }

    pub fn take_effects(&mut self) -> Vec<AppEffect> {
        std::mem::take(&mut self.pending_effects)
    }

    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::JiraProjectLoaded {
                request_id,
                purpose,
                credentials,
                result,
            } => self.apply_jira_project_result(request_id, purpose, credentials, result),
            AppEvent::RootsPageLoaded { request_id, result } => {
                self.apply_roots_page_result(request_id, result)
            }
            AppEvent::ChildrenLoaded {
                request_id,
                parent_key,
                result,
            } => self.apply_children_result(request_id, parent_key, result),
            AppEvent::ChildrenBatchLoaded { results, logs } => {
                self.apply_children_batch_result(results, logs)
            }
            AppEvent::BoardReloaded {
                request_id,
                board,
                logs,
            } => self.apply_board_reloaded(request_id, board, logs),
            AppEvent::SearchLoaded {
                request_id,
                term,
                result,
            } => self.apply_search_result(request_id, term, result),
            AppEvent::CredentialsSaveFailed {
                request_id,
                purpose,
                error,
            } => {
                if self.is_current_load(request_id) {
                    self.active_load_request_id = None;
                    self.status = match purpose {
                        JiraLoadPurpose::Setup => {
                            format!("Could not save Jira credentials: {error}")
                        }
                        JiraLoadPurpose::SwitchProject => {
                            format!("Could not save selected Jira project: {error}")
                        }
                        JiraLoadPurpose::Initial
                        | JiraLoadPurpose::Reload
                        | JiraLoadPurpose::ReloadBoard => {
                            format!("Could not save Jira config: {error}")
                        }
                    };
                }
            }
            AppEvent::ThemeSaveFailed(error) => self.notifications.push(Notification::error(
                "Theme not saved",
                format!("The theme changed for this session, but could not be saved: {error}"),
            )),
            AppEvent::IssueUrlCopied(url) => self
                .notifications
                .push(Notification::success("Issue URL copied", url)),
            AppEvent::IssueUrlCopyFailed { url, error } => {
                self.notifications.push(Notification::error(
                    "Issue URL not copied",
                    format!("Could not copy {url}: {error}"),
                ))
            }
            AppEvent::IssueAssigned {
                request_id,
                issue_key,
                assignee,
                result,
            } => self.apply_issue_assignment(request_id, issue_key, assignee, result),
        }
    }

    fn apply_issue_assignment(
        &mut self,
        request_id: u64,
        issue_key: String,
        assignee: Option<UserSummary>,
        result: Result<CommandLogEntry, (JiraError, CommandLogEntry)>,
    ) {
        if self.pending_assignment_requests.get(issue_key.as_str()) != Some(&request_id) {
            match result {
                Ok(log) => self.command_log.push(log),
                Err((_error, log)) => self.command_log.push(log),
            }
            return;
        }

        self.pending_assignment_requests.remove(issue_key.as_str());
        match result {
            Ok(log) => {
                self.command_log.push(log);
                let assignee_name = assignee.as_ref().map(|user| user.display_name.clone());
                let status = match assignee_name.as_ref() {
                    Some(name) => format!("{issue_key} assigned to {name}."),
                    None => format!("{issue_key} unassigned."),
                };
                self.filtered_tree
                    .update_assignee(issue_key.as_str(), assignee_name.clone());
                self.board
                    .update_assignee(issue_key.as_str(), assignee_name.clone());
                self.status = status.clone();
                self.notifications
                    .push(Notification::success("Assignee updated", status));
            }
            Err((error, log)) => {
                self.command_log.push(log);
                self.status = format!("Could not update {issue_key}: {}", error.0);
                self.notifications.push(Notification::error(
                    "Assignee not updated",
                    format!("Could not update {issue_key}: {}", error.0),
                ));
            }
        }
    }

    fn queue_jira_load(
        &mut self,
        purpose: JiraLoadPurpose,
        credentials: JiraCredentials,
        root_max_results: u32,
    ) {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        self.active_load_request_id = Some(request_id);
        // A fresh project load resets browse/search/pagination state.
        self.view = ListView::Browse;
        self.next_root_page_token = None;
        self.pending_roots_request_id = None;
        self.search_request_id = None;
        self.applied_search_term = None;
        self.pending_child_requests.clear();
        // Default to no expansion restore; reload paths re-seed this afterwards.
        self.expansion_to_restore.clear();
        self.soft_reload_parents.clear();
        self.reload_root_ids = None;
        self.pending_reload_seamless = false;
        self.reload_children_buffer.clear();
        self.filtered_tree.set_flat(false);
        self.pending_effects.push(AppEffect::LoadJiraProject {
            request_id,
            purpose,
            credentials,
            fields: self.current_fields_param(),
            root_max_results,
        });
    }

    /// The `fields` query value for the current visible columns.
    fn current_fields_param(&self) -> String {
        JiraIssueColumn::fields_param(self.filtered_tree.visible_columns())
    }

    fn next_request_id(&mut self) -> u64 {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        request_id
    }

    fn is_current_load(&self, request_id: u64) -> bool {
        self.active_load_request_id == Some(request_id)
    }

    fn apply_jira_project_result(
        &mut self,
        request_id: u64,
        purpose: JiraLoadPurpose,
        credentials: JiraCredentials,
        result: JiraProjectLoadResult,
    ) {
        if !self.is_current_load(request_id) {
            return;
        }
        self.active_load_request_id = None;
        if matches!(purpose, JiraLoadPurpose::Initial) {
            self.awaiting_initial_load = false;
        }

        self.command_log.extend(result.logs);

        // A list reload (Shift+R on the List tab) refreshes only the issue tree;
        // the board, projects, users, and field metadata are not refetched, so
        // we must not apply (empty) placeholders for them here.
        let list_only = matches!(purpose, JiraLoadPurpose::Reload);

        if !list_only {
            if let Ok(fields) = result.fields {
                self.apply_available_columns(fields);
            } else {
                self.notifications.push(Notification::error(
                    "Jira fields not loaded",
                    "Issue list is using built-in columns.",
                ));
            }

            if let Ok(projects) = result.projects {
                self.projects = projects;
            } else {
                self.notifications.push(Notification::error(
                    "Jira projects not loaded",
                    "Project switcher is unavailable until reload succeeds.",
                ));
            }
        }

        let users = result.users;
        let current_user = result.current_user;
        let board = result.board;
        match result.issues {
            Ok(issues) => {
                let fallback_board_issues = issues.clone();
                if !list_only {
                    if let Ok(users) = users {
                        self.users = users;
                    } else {
                        self.users.clear();
                        self.notifications.push(Notification::error(
                            "Jira users not loaded",
                            "Assignee selector is unavailable until reload succeeds.",
                        ));
                    }

                    if let Ok(current_user) = current_user {
                        self.current_user = Some(current_user);
                    } else {
                        self.current_user = None;
                        self.notifications.push(Notification::error(
                            "Current Jira user not loaded",
                            "Assign-to-me shortcut is unavailable until reload succeeds.",
                        ));
                    }
                }

                self.credentials = Some(credentials);
                if let Some(credentials) = &self.credentials {
                    self.filtered_tree.set_jira_site(credentials.site.clone());
                }
                self.filtered_tree.set_flat(false);
                self.view = ListView::Browse;
                let roots = tree_items_from_issues(issues);
                if std::mem::take(&mut self.pending_reload_seamless) {
                    self.begin_seamless_reload(roots, result.next_page_token);
                } else {
                    self.filtered_tree
                        .set_items(roots, &self.expansion_to_restore);
                    self.next_root_page_token = result.next_page_token;
                    // Lazy paging: the next page is pulled in on scroll
                    // (`maybe_prefetch_more_roots`), not eagerly up front.
                    // Re-open and re-fetch any roots whose expansion is being
                    // restored; nested levels follow as their children arrive.
                    self.drive_expansion_restore();
                }
                if !list_only {
                    match board {
                        Ok(mut board) => {
                            normalize_board_user_fields(
                                &mut board,
                                &self.users,
                                self.current_user.as_ref(),
                            );
                            merge_board_issue_fields(&mut board, &fallback_board_issues);
                            self.board.set_data(board);
                        }
                        Err(error) => {
                            let message = error.0;
                            self.board
                                .set_data(BoardData::from_issues(fallback_board_issues));
                            self.board.set_error(message.clone());
                            self.notifications.push(Notification::error(
                                "Jira board not loaded",
                                "Board tab is grouped by issue status until the board endpoint succeeds.",
                            ));
                        }
                    }
                }
                self.screen = Screen::Main;
                self.status = match purpose {
                    JiraLoadPurpose::Initial | JiraLoadPurpose::Reload => {
                        String::from("Jira issues loaded.")
                    }
                    JiraLoadPurpose::ReloadBoard => String::from("Jira board loaded."),
                    JiraLoadPurpose::Setup => {
                        String::from("Jira credentials saved and issues loaded.")
                    }
                    JiraLoadPurpose::SwitchProject => {
                        format!("Jira project {} loaded.", self.current_project())
                    }
                };
            }
            Err(error) => {
                if !list_only && let Ok(board) = board {
                    self.board.set_data(board);
                }
                self.status = error.0;
            }
        }
    }

    /// Applies a board-only reload: refreshes the board without touching the
    /// list view, its selection, or its paging state.
    fn apply_board_reloaded(
        &mut self,
        request_id: u64,
        board: Result<BoardData, JiraError>,
        logs: Vec<CommandLogEntry>,
    ) {
        if !self.is_current_load(request_id) {
            return;
        }
        self.active_load_request_id = None;
        self.command_log.extend(logs);
        match board {
            Ok(mut board) => {
                normalize_board_user_fields(&mut board, &self.users, self.current_user.as_ref());
                self.board.set_data(board);
                self.status = String::from("Jira board loaded.");
            }
            Err(error) => {
                self.board.set_error(error.0);
                self.notifications.push(Notification::error(
                    "Jira board not loaded",
                    "Board endpoint failed; the previous board is still shown.",
                ));
            }
        }
    }

    /// After the visible columns change, the required `fields` set changes too.
    /// Re-fetch the current view so newly-shown columns get populated.
    fn reload_current_view_fields(&mut self) {
        match &self.view {
            ListView::Browse => {
                let Some(credentials) = self.credentials.clone() else {
                    return;
                };
                self.queue_jira_load(
                    JiraLoadPurpose::Reload,
                    credentials,
                    crate::services::jira::ROOT_PAGE_SIZE,
                );
            }
            ListView::Search(term) => {
                let term = term.clone();
                self.run_search(term);
            }
        }
    }

    fn apply_available_columns(&mut self, fields: Vec<FieldSummary>) {
        let mut columns = vec![
            JiraIssueColumn::Field {
                id: String::from("assignee"),
                label: String::from("Assignee"),
            },
            JiraIssueColumn::Status,
            JiraIssueColumn::labels_column(),
            JiraIssueColumn::IssueType,
        ];
        let mut name_counts = std::collections::HashMap::new();
        name_counts.insert(String::from("Assignee"), 1);
        name_counts.insert(String::from("Status"), 1);
        name_counts.insert(String::from("Labels"), 1);
        name_counts.insert(String::from("Work type"), 1);

        let mut candidate_fields = Vec::new();
        for field in fields {
            let is_known = matches!(
                field.id.as_str(),
                "key" | "summary" | "issuetype" | "status" | "priority" | "labels"
            );
            if !is_known {
                *name_counts.entry(field.name.clone()).or_insert(0) += 1;
                candidate_fields.push(field);
            }
        }

        columns.extend(candidate_fields.into_iter().map(|field| {
            let label = if name_counts.get(&field.name).copied().unwrap_or(0) > 1 {
                format!("{} ({})", field.name, field.id)
            } else {
                field.name
            };
            JiraIssueColumn::Field {
                id: field.id,
                label,
            }
        }));
        self.filtered_tree.set_available_columns(columns);
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
        self.board_group_dropdown.as_ref()
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

    fn selected_assignment_issue_key(&self) -> Option<&str> {
        if self.active_tab() == "Board" {
            self.selected_board_issue_key()
        } else {
            self.selected_issue_key()
        }
    }

    fn selected_assignment_assignee(&self, issue_key: &str) -> Option<String> {
        if self.active_tab() == "Board" {
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
        self.assignee_dropdown.as_ref()
    }

    pub fn is_assignee_dropdown_open(&self) -> bool {
        self.assignee_dropdown.is_some()
    }

    pub fn is_assignee_dropdown_filter_focused(&self) -> bool {
        self.assignee_dropdown
            .as_ref()
            .is_some_and(MultiSelectDropdownState::is_filter_focused)
    }

    pub fn project_dropdown(&self) -> Option<&MultiSelectDropdownState<ProjectSummary>> {
        self.project_dropdown.as_ref()
    }

    pub fn is_project_dropdown_open(&self) -> bool {
        self.project_dropdown.is_some()
    }

    pub fn theme_dropdown(&self) -> Option<&MultiSelectDropdownState<ThemeChoice>> {
        self.theme_dropdown.as_ref()
    }

    pub fn is_theme_dropdown_open(&self) -> bool {
        self.theme_dropdown.is_some()
    }

    pub fn quick_switcher(&self) -> Option<&MultiSelectDropdownState<QuickAction>> {
        self.quick_switcher.as_ref()
    }

    pub fn is_quick_switcher_open(&self) -> bool {
        self.quick_switcher.is_some()
    }

    pub fn is_board_group_dropdown_open(&self) -> bool {
        self.board_group_dropdown.is_some()
    }

    pub fn is_board_group_dropdown_filter_focused(&self) -> bool {
        self.board_group_dropdown
            .as_ref()
            .is_some_and(MultiSelectDropdownState::is_filter_focused)
    }

    pub fn is_any_dropdown_open(&self) -> bool {
        self.is_column_dropdown_open()
            || self.is_assignee_dropdown_open()
            || self.is_project_dropdown_open()
            || self.is_theme_dropdown_open()
            || self.is_quick_switcher_open()
            || self.is_board_group_dropdown_open()
    }

    pub fn is_quick_switcher_filter_focused(&self) -> bool {
        self.quick_switcher
            .as_ref()
            .is_some_and(MultiSelectDropdownState::is_filter_focused)
    }

    pub fn is_help_open(&self) -> bool {
        matches!(self.modal, Some(ModalState::Help { .. }))
    }

    pub fn is_project_dropdown_filter_focused(&self) -> bool {
        self.project_dropdown.as_ref().is_some_and(
            crate::components::generic::dropdown::MultiSelectDropdownState::is_filter_focused,
        )
    }

    pub fn is_theme_dropdown_filter_focused(&self) -> bool {
        self.theme_dropdown.as_ref().is_some_and(
            crate::components::generic::dropdown::MultiSelectDropdownState::is_filter_focused,
        )
    }

    pub fn is_input_focused(&self) -> bool {
        self.screen == Screen::Setup
            || self.is_filter_focused()
            || self.is_column_dropdown_filter_focused()
            || self.is_assignee_dropdown_filter_focused()
            || self.is_project_dropdown_filter_focused()
            || self.is_theme_dropdown_filter_focused()
            || self.is_quick_switcher_filter_focused()
            || self.is_board_group_dropdown_filter_focused()
    }

    pub fn command_log_entries(&self) -> &[CommandLogEntry] {
        &self.command_log
    }

    pub fn command_log_follows_tail(&self) -> bool {
        self.command_log_view.follow.get()
    }

    pub fn command_log_scroll(&self) -> usize {
        self.command_log_view.offset.get()
    }

    /// Records the wrapped-line total, viewport height, and clamped scroll
    /// offset from the latest render so keyboard/mouse scrolling can clamp
    /// without knowing the dialog width. Leaves `follow` untouched.
    pub fn cache_command_log_layout(&self, scroll: usize, total: usize, viewport: usize) {
        self.command_log_view.offset.set(scroll);
        self.command_log_view.last_total.set(total);
        self.command_log_view.last_viewport.set(viewport);
    }

    fn command_log_max_scroll(&self) -> usize {
        let viewport = self.command_log_view.last_viewport.get().max(1);
        self.command_log_view
            .last_total
            .get()
            .saturating_sub(viewport)
    }

    /// Scrolls the command log by `delta` wrapped lines, leaving follow mode
    /// when scrolling up and re-entering it once the bottom is reached again.
    pub(crate) fn scroll_command_log(&mut self, delta: isize) {
        self.command_log_view.go_to_start_pending.set(false);
        let max_scroll = self.command_log_max_scroll();
        let current = if self.command_log_view.follow.get() {
            max_scroll
        } else {
            self.command_log_view.offset.get().min(max_scroll)
        };
        let next = (current as isize + delta).clamp(0, max_scroll as isize) as usize;
        self.command_log_view.offset.set(next);
        self.command_log_view.follow.set(next >= max_scroll);
    }

    pub(crate) fn page_command_log(&mut self, direction: isize) {
        let viewport = self.command_log_view.last_viewport.get().max(1) as isize;
        self.scroll_command_log(direction * viewport);
    }

    /// Scrolls by half a viewport (Ctrl+U / Ctrl+D).
    pub(crate) fn half_page_command_log(&mut self, direction: isize) {
        let half = (self.command_log_view.last_viewport.get().max(2) / 2) as isize;
        self.scroll_command_log(direction * half);
    }

    pub(crate) fn command_log_to_start(&mut self) {
        self.command_log_view.go_to_start_pending.set(false);
        self.command_log_view.follow.set(false);
        self.command_log_view.offset.set(0);
    }

    pub(crate) fn command_log_to_end(&mut self) {
        self.command_log_view.go_to_start_pending.set(false);
        self.command_log_view.follow.set(true);
    }

    /// Handles a `g` keypress: the first arms the prefix, the second (`gg`)
    /// jumps to the top.
    pub(crate) fn command_log_arm_go_to_start(&mut self) {
        if self.command_log_view.go_to_start_pending.get() {
            self.command_log_to_start();
        } else {
            self.command_log_view.go_to_start_pending.set(true);
        }
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

    pub fn active_tab(&self) -> &'static str {
        self.tabs.active_title(APP_TABS).unwrap_or("")
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

    pub fn handle_key(&mut self, key: KeyEvent, keybindings: &KeyBindings) {
        let typing = match self.screen {
            Screen::Setup => true,
            Screen::Main if self.quick_switcher.is_some() => true,
            Screen::Main if self.project_dropdown.is_some() => true,
            Screen::Main if self.theme_dropdown.is_some() => true,
            Screen::Main if self.assignee_dropdown.is_some() => true,
            Screen::Main if self.filtered_tree.is_column_dropdown_open() => true,
            Screen::Main if self.filtered_tree.is_filter_focused() => true,
            Screen::Main if self.board_filter.is_focused() => true,
            _ => false,
        };

        if self.leader_pending {
            self.leader_pending = false;
            if let Some(Action::Quit) = keybindings.global_action_for(key) {
                self.dispatch(Action::Quit);
                return;
            }
            let action = keybindings.leader_action_for(key);
            if action != Action::None {
                self.dispatch(action);
            }
            return;
        }

        if self.is_help_open() {
            self.handle_help_key(key, keybindings);
            return;
        }

        if let Some(action) = keybindings.global_action_for(key) {
            let focused_text_input = self.screen == Screen::Setup
                || self.is_filter_focused()
                || self.is_board_filter_focused()
                || self.is_column_dropdown_filter_focused()
                || self.is_project_dropdown_filter_focused()
                || self.is_theme_dropdown_filter_focused()
                || self.is_assignee_dropdown_filter_focused()
                || self.is_quick_switcher_filter_focused()
                || self.is_board_group_dropdown_filter_focused();
            let is_navigation_shortcut = (key.code == KeyCode::Char('j')
                || key.code == KeyCode::Char('k'))
                && key.modifiers.contains(KeyModifiers::CONTROL);
            let printable_text = matches!(key.code, KeyCode::Char(_))
                && !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT);
            let is_ctrl_q = keybindings.is_forced_quit(key);
            let reserved_input_action = matches!(action, Action::OpenHelp);
            // On the board, the grouping key takes precedence over the global
            // reload binding so it can be bound to a letter that also reloads
            // elsewhere (reload stays available on the board via reload_list).
            let board_grouping_override = self.screen == Screen::Main
                && self.active_tab() == "Board"
                && !self.is_board_filter_focused()
                && keybindings.board_action_for(key) == Action::ToggleBoardGrouping;
            if !board_grouping_override
                && !(focused_text_input
                    && (printable_text || is_navigation_shortcut)
                    && !reserved_input_action
                    || typing && matches!(action, Action::Quit) && !is_ctrl_q)
            {
                self.dispatch(self.contextual_global_action(action));
                return;
            }
        }

        if self.quick_switcher.is_some() {
            self.dispatch(Action::QuickSwitcher(self.dropdown_key_action(
                key,
                keybindings,
                self.is_quick_switcher_filter_focused(),
                KeyBindings::project_dropdown_action_for,
            )));
            return;
        }

        if self.assignee_dropdown.is_some() {
            self.dispatch(Action::AssigneeDropdown(self.dropdown_key_action(
                key,
                keybindings,
                self.is_assignee_dropdown_filter_focused(),
                KeyBindings::project_dropdown_action_for,
            )));
            return;
        }

        if self.board_group_dropdown.is_some() {
            self.dispatch(Action::BoardGroupDropdown(
                self.dropdown_key_action(
                    key,
                    keybindings,
                    self.board_group_dropdown
                        .as_ref()
                        .is_some_and(MultiSelectDropdownState::is_filter_focused),
                    KeyBindings::project_dropdown_action_for,
                ),
            ));
            return;
        }

        if self.theme_dropdown.is_some() {
            self.dispatch(Action::ThemeDropdown(self.dropdown_key_action(
                key,
                keybindings,
                self.is_theme_dropdown_filter_focused(),
                KeyBindings::theme_dropdown_action_for,
            )));
            return;
        }

        match self.screen {
            Screen::Setup => self.dispatch_setup(keybindings.setup_action_for(key)),
            Screen::Main if self.is_command_log_open() => {
                self.dispatch(keybindings.command_log_action_for(key))
            }
            Screen::Main if self.is_sprint_details_open() => {
                self.dispatch(keybindings.sprint_details_action_for(key))
            }
            Screen::Main if self.project_dropdown.is_some() => {
                self.dispatch(Action::ProjectDropdown(self.dropdown_key_action(
                    key,
                    keybindings,
                    self.is_project_dropdown_filter_focused(),
                    KeyBindings::project_dropdown_action_for,
                )));
            }
            Screen::Main if self.filtered_tree.is_column_dropdown_open() => {
                let action = if self.filtered_tree.is_column_dropdown_filter_focused() {
                    let is_ctrl_space = key.code == KeyCode::Char(' ')
                        && key.modifiers.contains(KeyModifiers::CONTROL);
                    let is_ctrl_enter =
                        key.code == KeyCode::Enter && key.modifiers.contains(KeyModifiers::CONTROL);
                    if is_ctrl_space || is_ctrl_enter {
                        JiraFilteredTreeAction::Dropdown(
                            crate::components::generic::dropdown::DropdownAction::ToggleSelected,
                        )
                    } else if key.code == KeyCode::PageUp {
                        JiraFilteredTreeAction::Dropdown(
                            crate::components::generic::dropdown::DropdownAction::HalfPageUp,
                        )
                    } else if key.code == KeyCode::PageDown {
                        JiraFilteredTreeAction::Dropdown(
                            crate::components::generic::dropdown::DropdownAction::HalfPageDown,
                        )
                    } else if key.code == KeyCode::Home {
                        JiraFilteredTreeAction::Dropdown(
                            crate::components::generic::dropdown::DropdownAction::GoToStart,
                        )
                    } else if key.code == KeyCode::End {
                        JiraFilteredTreeAction::Dropdown(
                            crate::components::generic::dropdown::DropdownAction::GoToEnd,
                        )
                    } else if key.code == KeyCode::Esc
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
            Screen::Main if self.active_tab() == "Board" && self.board_filter.is_focused() => {
                self.dispatch_board_filter(keybindings.filter_action_for(key));
            }
            Screen::Main if self.filtered_tree.is_filter_focused() => {
                let action = keybindings.filter_action_for(key);
                if action == FilterAction::MoveSelectionUp {
                    self.dispatch(Action::JiraFilteredTree(
                        JiraFilteredTreeAction::FilteredTree(
                            crate::components::generic::filtered_tree::FilteredTreeAction::Tree(
                                crate::components::generic::tree::TreeAction::MoveUp,
                            ),
                        ),
                    ));
                } else if action == FilterAction::MoveSelectionDown {
                    self.dispatch(Action::JiraFilteredTree(
                        JiraFilteredTreeAction::FilteredTree(
                            crate::components::generic::filtered_tree::FilteredTreeAction::Tree(
                                crate::components::generic::tree::TreeAction::MoveDown,
                            ),
                        ),
                    ));
                } else {
                    self.dispatch_filter(action);
                }
            }
            Screen::Main => {
                let action = if self.active_tab() == "Board" {
                    keybindings.board_action_for(key)
                } else {
                    keybindings.jira_filtered_tree_action_for(key)
                };
                if self.active_tab() != "List"
                    && !matches!(
                        action,
                        Action::Tabs(_)
                            | Action::Board(_)
                            | Action::JiraFilteredTree(_)
                            | Action::FocusBoardFilter
                            | Action::ClearBoardFilter
                            | Action::ToggleBoardGrouping
                            | Action::ToggleSprintDetails
                            | Action::ToggleAssigneeDropdown
                            | Action::AssignSelectedToMe
                            | Action::UnassignSelected
                    )
                {
                    return;
                }
                self.dispatch(action);
            }
        }
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect, keybindings: &KeyBindings) {
        if self.is_help_open() {
            let scroll_delta = match mouse.kind {
                MouseEventKind::ScrollUp => Some(-1),
                MouseEventKind::ScrollDown => Some(1),
                _ => None,
            };
            if let Some(delta) = scroll_delta {
                let items = keybindings.help_items(
                    self.screen(),
                    self.active_tab(),
                    self.is_any_dropdown_open(),
                );
                self.move_help_selection(delta, items.len());
            }
            return;
        }
        if self.is_command_log_open() {
            match mouse.kind {
                MouseEventKind::ScrollUp => self.scroll_command_log(-3),
                MouseEventKind::ScrollDown => self.scroll_command_log(3),
                _ => {}
            }
            return;
        }
        if self.is_sprint_details_open() {
            return;
        }
        // Shift + vertical wheel and native horizontal wheel both scroll left/
        // right; a plain wheel scrolls up/down. `scroll_delta` keeps driving the
        // existing vertical consumers (help, dropdowns, list); `horizontal_delta`
        // is board-only.
        let shift = mouse.modifiers.contains(KeyModifiers::SHIFT);
        let (scroll_delta, horizontal_delta): (Option<isize>, Option<isize>) = match mouse.kind {
            MouseEventKind::ScrollUp if shift => (None, Some(-1)),
            MouseEventKind::ScrollDown if shift => (None, Some(1)),
            MouseEventKind::ScrollUp => (Some(-1), None),
            MouseEventKind::ScrollDown => (Some(1), None),
            MouseEventKind::ScrollLeft => (None, Some(-1)),
            MouseEventKind::ScrollRight => (None, Some(1)),
            _ => (None, None),
        };
        let is_left_click = matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left));
        if !is_left_click && scroll_delta.is_none() && horizontal_delta.is_none() {
            return;
        }

        let point = (mouse.column, mouse.row);
        let [frame_area, _status_area] = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Min(1),
                ratatui::layout::Constraint::Length(1),
            ])
            .areas(area);
        let outer = crate::ui::chrome::tabbed_frame(
            self.active_tab_index(),
            self.tabs_view_mode(),
            self.theme(),
        );
        let inner = outer.inner(frame_area);
        if let Some(delta) = scroll_delta {
            if self.handle_open_dropdown_scroll(point, inner, delta) {
                return;
            }
        }
        if self.screen == Screen::Main
            && self.active_tab() == "List"
            && self.filtered_tree.is_column_dropdown_open()
            && self.is_column_trigger_point(inner, point, keybindings)
        {
            self.close_dropdown(DropdownKind::JiraColumns);
            return;
        }
        if self.handle_open_dropdown_mouse(point, inner) {
            return;
        }
        if area.height > 0 && point.1 == area.height - 1 {
            if !self.current_project().is_empty()
                && point.0
                    >= area
                        .width
                        .saturating_sub(self.current_project().len() as u16 + 10)
            {
                self.open_project_dropdown();
            }
            return;
        }

        if !contains_point(inner, point) {
            return;
        }

        if self.screen != Screen::Main {
            return;
        }
        if self.active_tab() == "Board" {
            let [top_row, _content_area] = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Length(1),
                    ratatui::layout::Constraint::Min(1),
                ])
                .areas(inner);
            let group_width = (self.board_grouping.label().len() as u16 + 9).max(16);
            let [filter_area, group_area] = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Horizontal)
                .constraints([
                    ratatui::layout::Constraint::Min(1),
                    ratatui::layout::Constraint::Length(group_width),
                ])
                .areas(top_row);
            if contains_point(group_area, point) {
                self.toggle_dropdown(DropdownKind::BoardGroup);
                return;
            }
            if contains_point(filter_area, point) {
                self.board_filter.focus();
                return;
            }
            if let Some(delta) = scroll_delta {
                // Wheel scrolls the viewport one line per notch (matching the
                // list) without moving the selection.
                self.board.scroll_viewport(delta);
            }
            if let Some(delta) = horizontal_delta {
                // Shift/horizontal wheel pans the columns one cell per notch.
                self.board.scroll_viewport_horizontal(delta as i32);
            }
            return;
        }
        if self.active_tab() != "List" {
            return;
        }

        let [filter_row, content_area] = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Min(1),
            ])
            .areas(inner);
        let trigger_width = 9u16.saturating_add(keybindings.open_columns_label().len() as u16);
        let [filter_area, trigger_area] = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Min(1),
                ratatui::layout::Constraint::Length(trigger_width),
            ])
            .areas(filter_row);
        if contains_point(trigger_area, point) {
            self.toggle_dropdown(DropdownKind::JiraColumns);
            return;
        }
        if contains_point(filter_area, point) {
            self.dispatch_jira_filtered_tree(JiraFilteredTreeAction::FilteredTree(
                crate::components::generic::filtered_tree::FilteredTreeAction::FocusFilter,
            ));
            return;
        }

        let [content_main, _, _scrollbar_area] = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Min(1),
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Length(1),
            ])
            .areas(content_area);
        if !contains_point(content_main, point) {
            return;
        }

        let rows_start_y = match self.filtered_tree_view_mode() {
            FilteredTreeViewMode::List => content_main.y,
            FilteredTreeViewMode::Table => content_main.y.saturating_add(1),
        };
        if point.1 < rows_start_y {
            return;
        }
        let viewport_height = match self.filtered_tree_view_mode() {
            FilteredTreeViewMode::List => content_main.height as usize,
            FilteredTreeViewMode::Table => content_main.height.saturating_sub(1) as usize,
        };
        if let Some(delta) = scroll_delta {
            self.filtered_tree.scroll_viewport(delta, viewport_height);
            // Wheeling toward the bottom lazily pulls the next page; the wheel
            // moves the viewport, not the selection, so pass the real height.
            self.maybe_prefetch_more_roots(viewport_height);
            return;
        }
        if !is_left_click {
            return;
        }
        let visible_range = self.visible_issue_range(viewport_height);
        let visible_pos = point.1.saturating_sub(rows_start_y) as usize;
        let selected = visible_range.start.saturating_add(visible_pos);
        let rows = self.visible_issue_rows();
        if selected >= visible_range.end || selected >= rows.len() {
            return;
        }
        self.filtered_tree.select_item_index(selected);

        let row = &rows[selected];
        let chevron_x = content_main.x.saturating_add((row.depth * 2) as u16);
        if row.expandable && point.0 >= chevron_x && point.0 <= chevron_x.saturating_add(1) {
            self.dispatch_jira_filtered_tree(JiraFilteredTreeAction::FilteredTree(
                crate::components::generic::filtered_tree::FilteredTreeAction::Tree(
                    crate::components::generic::tree::TreeAction::ToggleExpanded,
                ),
            ));
        }
    }

    fn handle_open_dropdown_scroll(&mut self, point: (u16, u16), area: Rect, delta: isize) -> bool {
        if self.quick_switcher.is_some() {
            return self.scroll_centered_dropdown(point, area, DropdownKind::QuickSwitcher, delta);
        }
        if self.theme_dropdown.is_some() {
            return self.scroll_centered_dropdown(point, area, DropdownKind::ThemePicker, delta);
        }
        if self.project_dropdown.is_some() {
            return self.scroll_centered_dropdown(
                point,
                area,
                DropdownKind::ProjectSwitcher,
                delta,
            );
        }
        if self.assignee_dropdown.is_some() {
            return self.scroll_centered_dropdown(point, area, DropdownKind::AssigneePicker, delta);
        }
        if self.filtered_tree.is_column_dropdown_open() {
            return self.scroll_column_dropdown(point, area, delta);
        }
        false
    }

    fn scroll_centered_dropdown(
        &mut self,
        point: (u16, u16),
        area: Rect,
        kind: DropdownKind,
        delta: isize,
    ) -> bool {
        let (width, rows) = match kind {
            DropdownKind::QuickSwitcher => self
                .quick_switcher
                .as_ref()
                .map(|dropdown| dropdown_dimensions(dropdown, area, 34, 10)),
            DropdownKind::ThemePicker => self
                .theme_dropdown
                .as_ref()
                .map(|dropdown| dropdown_dimensions(dropdown, area, 34, 12)),
            DropdownKind::ProjectSwitcher => self
                .project_dropdown
                .as_ref()
                .map(|dropdown| dropdown_dimensions(dropdown, area, 32, 10)),
            DropdownKind::AssigneePicker => self
                .assignee_dropdown
                .as_ref()
                .map(|dropdown| dropdown_dimensions(dropdown, area, 32, 10)),
            DropdownKind::BoardGroup => self
                .board_group_dropdown
                .as_ref()
                .map(|dropdown| dropdown_dimensions(dropdown, area, 28, 6)),
            DropdownKind::JiraColumns => None,
        }
        .unwrap_or((0, 0));
        if width == 0 || rows == 0 {
            return false;
        }

        let rect = centered_rect(area, width, rows + 3);
        if !contains_point(rect, point) {
            return true;
        }
        self.scroll_dropdown(kind, delta);
        true
    }

    fn scroll_column_dropdown(&mut self, point: (u16, u16), area: Rect, delta: isize) -> bool {
        let Some(dropdown) = self.column_dropdown() else {
            return false;
        };
        let longest = dropdown
            .options()
            .iter()
            .map(|option| option.label.chars().count())
            .max()
            .unwrap_or(0) as u16;
        let width = area.width.min((longest + 6).max(20));
        let height = area.height.min(16);
        let rect = Rect {
            x: area.x + area.width.saturating_sub(width + 1),
            y: area.y + 1,
            width,
            height,
        };
        if !contains_point(rect, point) {
            return true;
        }
        self.filtered_tree.scroll_column_dropdown(delta);
        true
    }

    fn is_column_trigger_point(
        &self,
        inner: Rect,
        point: (u16, u16),
        keybindings: &KeyBindings,
    ) -> bool {
        let [filter_row, _content_area] = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Min(1),
            ])
            .areas(inner);
        let trigger_width = 9u16.saturating_add(keybindings.open_columns_label().len() as u16);
        let [_filter_area, trigger_area] = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Min(1),
                ratatui::layout::Constraint::Length(trigger_width),
            ])
            .areas(filter_row);
        contains_point(trigger_area, point)
    }

    fn handle_open_dropdown_mouse(&mut self, point: (u16, u16), area: Rect) -> bool {
        if self.quick_switcher.is_some() {
            return self.handle_centered_dropdown_mouse(point, area, DropdownKind::QuickSwitcher);
        }
        if self.theme_dropdown.is_some() {
            return self.handle_centered_dropdown_mouse(point, area, DropdownKind::ThemePicker);
        }
        if self.project_dropdown.is_some() {
            return self.handle_centered_dropdown_mouse(point, area, DropdownKind::ProjectSwitcher);
        }
        if self.assignee_dropdown.is_some() {
            return self.handle_centered_dropdown_mouse(point, area, DropdownKind::AssigneePicker);
        }
        if self.filtered_tree.is_column_dropdown_open() {
            return self.handle_column_dropdown_mouse(point, area);
        }
        false
    }

    fn handle_centered_dropdown_mouse(
        &mut self,
        point: (u16, u16),
        area: Rect,
        kind: DropdownKind,
    ) -> bool {
        let (width, rows) = match kind {
            DropdownKind::QuickSwitcher => self
                .quick_switcher
                .as_ref()
                .map(|dropdown| dropdown_dimensions(dropdown, area, 34, 10)),
            DropdownKind::ThemePicker => self
                .theme_dropdown
                .as_ref()
                .map(|dropdown| dropdown_dimensions(dropdown, area, 34, 12)),
            DropdownKind::ProjectSwitcher => self
                .project_dropdown
                .as_ref()
                .map(|dropdown| dropdown_dimensions(dropdown, area, 32, 10)),
            DropdownKind::AssigneePicker => self
                .assignee_dropdown
                .as_ref()
                .map(|dropdown| dropdown_dimensions(dropdown, area, 32, 10)),
            DropdownKind::BoardGroup => self
                .board_group_dropdown
                .as_ref()
                .map(|dropdown| dropdown_dimensions(dropdown, area, 28, 6)),
            DropdownKind::JiraColumns => None,
        }
        .unwrap_or((0, 0));
        if width == 0 || rows == 0 {
            return false;
        }

        let rect = centered_rect(area, width, rows + 3);
        if !contains_point(rect, point) {
            return true;
        }

        let inner = inset_rect(rect, 1, 1);
        let [_, padded_inner, _] = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Min(1),
                ratatui::layout::Constraint::Length(1),
            ])
            .areas(inner);
        let [filter_area, options_area] = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Min(1),
            ])
            .areas(padded_inner);

        if contains_point(filter_area, point) {
            match kind {
                DropdownKind::QuickSwitcher => {
                    if let Some(dropdown) = &mut self.quick_switcher {
                        dropdown.dispatch(
                            crate::components::generic::dropdown::DropdownAction::FocusFilter,
                        );
                    }
                }
                DropdownKind::ThemePicker => {
                    if let Some(dropdown) = &mut self.theme_dropdown {
                        dropdown.dispatch(
                            crate::components::generic::dropdown::DropdownAction::FocusFilter,
                        );
                    }
                }
                DropdownKind::ProjectSwitcher => {
                    if let Some(dropdown) = &mut self.project_dropdown {
                        dropdown.dispatch(
                            crate::components::generic::dropdown::DropdownAction::FocusFilter,
                        );
                    }
                }
                DropdownKind::AssigneePicker => {
                    if let Some(dropdown) = &mut self.assignee_dropdown {
                        dropdown.dispatch(
                            crate::components::generic::dropdown::DropdownAction::FocusFilter,
                        );
                    }
                }
                DropdownKind::BoardGroup => {
                    if let Some(dropdown) = &mut self.board_group_dropdown {
                        dropdown.dispatch(
                            crate::components::generic::dropdown::DropdownAction::FocusFilter,
                        );
                    }
                }
                DropdownKind::JiraColumns => {}
            }
            return true;
        }

        if !contains_point(options_area, point) {
            return true;
        }

        let row = point.1.saturating_sub(options_area.y) as usize;
        self.click_dropdown_option(kind, row, options_area.height as usize);
        true
    }

    fn scroll_dropdown(&mut self, kind: DropdownKind, delta: isize) {
        match kind {
            DropdownKind::QuickSwitcher => {
                if let Some(dropdown) = &mut self.quick_switcher {
                    dropdown.scroll_viewport(delta);
                }
            }
            DropdownKind::ThemePicker => {
                if let Some(dropdown) = &mut self.theme_dropdown {
                    dropdown.scroll_viewport(delta);
                }
            }
            DropdownKind::ProjectSwitcher => {
                if let Some(dropdown) = &mut self.project_dropdown {
                    dropdown.scroll_viewport(delta);
                }
            }
            DropdownKind::AssigneePicker => {
                if let Some(dropdown) = &mut self.assignee_dropdown {
                    dropdown.scroll_viewport(delta);
                }
            }
            DropdownKind::BoardGroup => {
                if let Some(dropdown) = &mut self.board_group_dropdown {
                    dropdown.scroll_viewport(delta);
                }
            }
            DropdownKind::JiraColumns => {}
        }
    }

    fn click_dropdown_option(&mut self, kind: DropdownKind, row: usize, height: usize) {
        match kind {
            DropdownKind::QuickSwitcher => {
                let Some(index) = dropdown_index_at(self.quick_switcher.as_ref(), row, height)
                else {
                    return;
                };
                if let Some(dropdown) = &mut self.quick_switcher {
                    dropdown.set_selected_index(index);
                }
                self.commit_quick_switcher_index(index);
            }
            DropdownKind::ThemePicker => {
                let Some(index) = dropdown_index_at(self.theme_dropdown.as_ref(), row, height)
                else {
                    return;
                };
                if let Some(dropdown) = &mut self.theme_dropdown {
                    dropdown.set_selected_index(index);
                }
                self.dispatch_theme_dropdown(
                    crate::components::generic::dropdown::DropdownAction::ToggleSelected,
                );
            }
            DropdownKind::ProjectSwitcher => {
                let Some(index) = dropdown_index_at(self.project_dropdown.as_ref(), row, height)
                else {
                    return;
                };
                if let Some(dropdown) = &mut self.project_dropdown {
                    dropdown.set_selected_index(index);
                }
                self.dispatch_project_dropdown(
                    crate::components::generic::dropdown::DropdownAction::ToggleSelected,
                );
            }
            DropdownKind::AssigneePicker => {
                let Some(index) = dropdown_index_at(self.assignee_dropdown.as_ref(), row, height)
                else {
                    return;
                };
                if let Some(dropdown) = &mut self.assignee_dropdown {
                    dropdown.set_selected_index(index);
                }
                self.dispatch_assignee_dropdown(
                    crate::components::generic::dropdown::DropdownAction::ToggleSelected,
                );
            }
            DropdownKind::BoardGroup => {
                let Some(index) =
                    dropdown_index_at(self.board_group_dropdown.as_ref(), row, height)
                else {
                    return;
                };
                if let Some(dropdown) = &mut self.board_group_dropdown {
                    dropdown.set_selected_index(index);
                }
                self.dispatch_board_group_dropdown(
                    crate::components::generic::dropdown::DropdownAction::ToggleSelected,
                );
            }
            DropdownKind::JiraColumns => {}
        }
    }

    fn handle_column_dropdown_mouse(&mut self, point: (u16, u16), area: Rect) -> bool {
        let Some(dropdown) = self.column_dropdown() else {
            return false;
        };
        let longest = dropdown
            .options()
            .iter()
            .map(|option| option.label.chars().count())
            .max()
            .unwrap_or(0) as u16;
        let width = area.width.min((longest + 6).max(20));
        let height = area.height.min(16);
        let rect = Rect {
            x: area.x + area.width.saturating_sub(width + 1),
            y: area.y + 1,
            width,
            height,
        };
        if !contains_point(rect, point) {
            return true;
        }
        let inner = inset_rect(rect, 1, 1);
        let [_, padded_inner] = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Min(1),
            ])
            .areas(inner);
        let [content_area, _] = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Min(1),
                ratatui::layout::Constraint::Length(1),
            ])
            .areas(padded_inner);
        let [filter_area, options_area] = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Min(1),
            ])
            .areas(content_area);
        if contains_point(filter_area, point) {
            self.dispatch_jira_filtered_tree(JiraFilteredTreeAction::Dropdown(
                crate::components::generic::dropdown::DropdownAction::FocusFilter,
            ));
            return true;
        }
        if !contains_point(options_area, point) {
            return true;
        }
        let row = point.1.saturating_sub(options_area.y) as usize;
        self.filtered_tree
            .click_column_dropdown_row(row, options_area.height as usize);
        true
    }

    fn handle_help_key(&mut self, key: KeyEvent, keybindings: &KeyBindings) {
        let item_count = keybindings
            .help_items(
                self.screen(),
                self.active_tab(),
                self.is_any_dropdown_open(),
            )
            .len();
        match keybindings.help_dialog_action_for(key) {
            crate::keymap::HelpDialogAction::Close => self.close_dialog(DialogKind::Help),
            crate::keymap::HelpDialogAction::Up => self.move_help_selection(-1, item_count),
            crate::keymap::HelpDialogAction::Down => self.move_help_selection(1, item_count),
            crate::keymap::HelpDialogAction::PageUp => self.move_help_selection(-4, item_count),
            crate::keymap::HelpDialogAction::PageDown => self.move_help_selection(4, item_count),
            crate::keymap::HelpDialogAction::First => {
                if let Some(selected) = self.help_selected_mut() {
                    *selected = 0;
                }
            }
            crate::keymap::HelpDialogAction::Last => {
                if let Some(selected) = self.help_selected_mut() {
                    *selected = item_count.saturating_sub(1);
                }
            }
            crate::keymap::HelpDialogAction::None => {}
        }
    }

    /// Mutable handle to the help dialog's selection index, when help is open.
    fn help_selected_mut(&mut self) -> Option<&mut usize> {
        match &mut self.modal {
            Some(ModalState::Help { selected }) => Some(selected),
            _ => None,
        }
    }

    fn move_help_selection(&mut self, delta: isize, item_count: usize) {
        let Some(selected) = self.help_selected_mut() else {
            return;
        };
        if item_count == 0 {
            *selected = 0;
            return;
        }
        *selected = selected.saturating_add_signed(delta).min(item_count - 1);
    }

    fn dropdown_key_action(
        &self,
        key: KeyEvent,
        keybindings: &KeyBindings,
        filter_focused: bool,
        normal_action: fn(
            &KeyBindings,
            KeyEvent,
        ) -> crate::components::generic::dropdown::DropdownAction,
    ) -> crate::components::generic::dropdown::DropdownAction {
        if filter_focused {
            let is_ctrl_space =
                key.code == KeyCode::Char(' ') && key.modifiers.contains(KeyModifiers::CONTROL);
            let is_ctrl_enter =
                key.code == KeyCode::Enter && key.modifiers.contains(KeyModifiers::CONTROL);
            if is_ctrl_space || is_ctrl_enter {
                return crate::components::generic::dropdown::DropdownAction::ToggleSelected;
            }
            if key.code == KeyCode::Enter {
                return crate::components::generic::dropdown::DropdownAction::Filter(
                    FilterAction::Submit,
                );
            }
            if key.code == KeyCode::PageUp {
                return crate::components::generic::dropdown::DropdownAction::HalfPageUp;
            }
            if key.code == KeyCode::PageDown {
                return crate::components::generic::dropdown::DropdownAction::HalfPageDown;
            }
            if key.code == KeyCode::Home {
                return crate::components::generic::dropdown::DropdownAction::GoToStart;
            }
            if key.code == KeyCode::End {
                return crate::components::generic::dropdown::DropdownAction::GoToEnd;
            }
            if key.code == KeyCode::Esc
                || key.code == KeyCode::Char('[') && key.modifiers.contains(KeyModifiers::CONTROL)
            {
                crate::components::generic::dropdown::DropdownAction::Close
            } else {
                crate::components::generic::dropdown::DropdownAction::Filter(
                    keybindings.filter_action_for(key),
                )
            }
        } else if key.code == KeyCode::Esc
            || key.code == KeyCode::Char('[') && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            crate::components::generic::dropdown::DropdownAction::Close
        } else {
            normal_action(keybindings, key)
        }
    }

    pub fn dispatch(&mut self, action: Action) {
        match action {
            Action::Tabs(action) => self.dispatch_tabs(action),
            Action::JiraFilteredTree(action) => self.dispatch_jira_filtered_tree(action),
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
            Action::GoToBoard => self.select_tab("Board"),
            Action::GoToList => self.select_tab("List"),
            Action::GoToTimeline => self.select_tab("Timeline"),
            Action::GoToFilters => self.select_tab("Filters"),
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

    fn dispatch_board_filter(&mut self, action: FilterAction) {
        match action {
            FilterAction::Quit => self.running = false,
            _ => {
                self.board_filter.dispatch(action);
            }
        }
    }
    fn contextual_global_action(&self, action: Action) -> Action {
        if matches!(action, Action::ReloadList | Action::ReloadNode) && self.active_tab() == "Board"
        {
            Action::ReloadBoard
        } else {
            action
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
            SetupAction::Delete => {
                let cursor = self.setup.cursors[field_idx];
                let val = self.setup.active_value_mut();
                input::delete_forwards(val, cursor);
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
        let can_dispatch = self.tabs.is_active(APP_TABS, "List")
            || self.tabs.is_active(APP_TABS, "Board")
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
            JiraFilteredTreeEvent::FilterChanged(term) => self.run_search(term),
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

    fn open_dialog(&mut self, dialog: DialogKind) {
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
            DropdownKind::QuickSwitcher => self.open_quick_switcher(),
            DropdownKind::ProjectSwitcher => self.open_project_dropdown(),
            DropdownKind::ThemePicker => self.open_theme_dropdown(),
            DropdownKind::AssigneePicker => self.open_assignee_dropdown(),
            DropdownKind::BoardGroup => self.open_board_group_dropdown(),
        }
    }

    fn close_dropdown(&mut self, dropdown: DropdownKind) {
        match dropdown {
            DropdownKind::JiraColumns => self.filtered_tree.close_column_dropdown(),
            DropdownKind::QuickSwitcher => self.quick_switcher = None,
            DropdownKind::ProjectSwitcher => self.project_dropdown = None,
            DropdownKind::ThemePicker => self.close_theme_dropdown_without_selection(),
            DropdownKind::AssigneePicker => self.assignee_dropdown = None,
            DropdownKind::BoardGroup => self.board_group_dropdown = None,
        }
    }

    fn is_dropdown_open(&self, dropdown: DropdownKind) -> bool {
        match dropdown {
            DropdownKind::JiraColumns => self.filtered_tree.is_column_dropdown_open(),
            DropdownKind::QuickSwitcher => self.quick_switcher.is_some(),
            DropdownKind::ProjectSwitcher => self.project_dropdown.is_some(),
            DropdownKind::ThemePicker => self.theme_dropdown.is_some(),
            DropdownKind::AssigneePicker => self.assignee_dropdown.is_some(),
            DropdownKind::BoardGroup => self.board_group_dropdown.is_some(),
        }
    }

    fn close_dropdowns(&mut self) {
        self.quick_switcher = None;
        self.close_theme_dropdown_without_selection();
        self.project_dropdown = None;
        self.assignee_dropdown = None;
        self.filtered_tree.close_column_dropdown();
        self.board_group_dropdown = None;
    }

    fn close_dialogs(&mut self) {
        self.modal = None;
    }

    fn open_quick_switcher(&mut self) {
        self.close_overlays();
        let mut actions = vec![
            QuickAction::CommandLog,
            QuickAction::ThemePicker,
            QuickAction::ProjectPicker,
            QuickAction::Board,
            QuickAction::List,
            QuickAction::Timeline,
            QuickAction::Filters,
        ];
        if self.active_tab() == "List" {
            actions.insert(3, QuickAction::ReloadList);
        } else if self.active_tab() == "Board" {
            actions.insert(3, QuickAction::ReloadBoard);
        }
        let options = actions
            .into_iter()
            .map(|action| DropdownOption {
                selected: false,
                label: action.label(),
                value: action,
            })
            .collect();
        self.quick_switcher = Some(
            MultiSelectDropdownState::new(options)
                .single_select()
                .with_filter_focused(),
        );
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
            .map(|project| DropdownOption {
                selected: project.key == current_project,
                label: format!("{}  {}", project.key, project.name),
                value: project,
            })
            .collect();
        self.project_dropdown = Some(
            MultiSelectDropdownState::new(options)
                .single_select()
                .focus_selected()
                .with_filter_focused(),
        );
    }

    fn open_assignee_dropdown(&mut self) {
        let Some(issue_key) = self.selected_assignment_issue_key().map(str::to_owned) else {
            self.status = String::from("No issue selected.");
            return;
        };
        let current_assignee = self.selected_assignment_assignee(&issue_key);

        if self.users.is_empty() && current_assignee.is_none() {
            self.status = String::from("No assignable Jira users available.");
            return;
        }

        self.close_overlays();
        let mut options = vec![DropdownOption {
            selected: current_assignee.is_none(),
            label: String::from("Unassigned"),
            value: None,
        }];
        options.extend(self.users.iter().cloned().map(|user| DropdownOption {
            selected: current_assignee.as_deref() == Some(user.display_name.as_str()),
            label: user.display_name.clone(),
            value: Some(user),
        }));
        self.assignee_dropdown = Some(
            MultiSelectDropdownState::new(options)
                .single_select()
                .focus_selected()
                .with_filter_focused(),
        );
    }

    fn open_theme_dropdown(&mut self) {
        self.close_overlays();
        let current_theme = self.theme.name();
        let options = self
            .theme
            .choices()
            .into_iter()
            .map(|choice| DropdownOption {
                selected: choice.name == current_theme,
                label: choice.label(),
                value: choice,
            })
            .collect();
        self.theme_dropdown = Some(
            MultiSelectDropdownState::new(options)
                .single_select()
                .focus_selected()
                .with_filter_focused(),
        );
        self.theme_preview_origin = Some(self.theme.clone());
    }

    fn open_board_group_dropdown(&mut self) {
        self.close_overlays();
        let options = BoardGrouping::ALL
            .into_iter()
            .map(|grouping| DropdownOption {
                selected: grouping == self.board_grouping,
                label: grouping.label().to_owned(),
                value: grouping,
            })
            .collect();
        self.board_group_dropdown = Some(
            MultiSelectDropdownState::new(options)
                .single_select()
                .focus_selected()
                .with_filter_focused(),
        );
    }

    fn dispatch_board_group_dropdown(
        &mut self,
        action: crate::components::generic::dropdown::DropdownAction,
    ) {
        let Some(dropdown) = &mut self.board_group_dropdown else {
            return;
        };
        match dropdown.dispatch(action) {
            Some(DropdownEvent::Closed) => self.board_group_dropdown = None,
            Some(DropdownEvent::Toggled(index)) => {
                let Some(grouping) = self
                    .board_group_dropdown
                    .as_ref()
                    .and_then(|dropdown| dropdown.options().get(index))
                    .map(|option| option.value)
                else {
                    return;
                };
                self.board_grouping = grouping;
                self.board_group_dropdown = None;
                self.board.select_first(self.board_filter.value(), grouping);
            }
            None => {}
        }
    }

    fn close_theme_dropdown_without_selection(&mut self) {
        self.theme_dropdown = None;
        if let Some(theme) = self.theme_preview_origin.take() {
            self.theme = theme;
        }
    }

    fn preview_focused_theme(&mut self) {
        let Some(dropdown) = &self.theme_dropdown else {
            return;
        };
        let Some(choice) = dropdown
            .options()
            .get(dropdown.selected_index())
            .map(|option| option.value)
        else {
            return;
        };
        let base = self.theme_preview_origin.as_ref().unwrap_or(&self.theme);
        self.theme = base.with_name(choice.name);
    }

    fn dispatch_theme_dropdown(
        &mut self,
        action: crate::components::generic::dropdown::DropdownAction,
    ) {
        let event = {
            let Some(dropdown) = &mut self.theme_dropdown else {
                return;
            };
            dropdown.dispatch(action)
        };
        match event {
            Some(DropdownEvent::Closed) => self.close_theme_dropdown_without_selection(),
            Some(DropdownEvent::Toggled(index)) => {
                let Some(choice) = self
                    .theme_dropdown
                    .as_ref()
                    .and_then(|dropdown| dropdown.options().get(index))
                    .map(|option| option.value)
                else {
                    return;
                };
                let base = self
                    .theme_preview_origin
                    .take()
                    .unwrap_or_else(|| self.theme.clone());
                self.theme_dropdown = None;
                self.set_theme(base.with_name(choice.name));
                self.pending_effects.push(AppEffect::SaveTheme(choice.name));
                self.status = format!("Theme switched to {}.", choice.name.label());
            }
            None => self.preview_focused_theme(),
        }
    }

    fn dispatch_quick_switcher(
        &mut self,
        action: crate::components::generic::dropdown::DropdownAction,
    ) {
        if matches!(
            action,
            crate::components::generic::dropdown::DropdownAction::Filter(FilterAction::Submit)
        ) {
            self.commit_quick_switcher_selection();
            return;
        }

        let Some(dropdown) = &mut self.quick_switcher else {
            return;
        };
        match dropdown.dispatch(action) {
            Some(DropdownEvent::Closed) => self.quick_switcher = None,
            Some(DropdownEvent::Toggled(index)) => self.commit_quick_switcher_index(index),
            None => {}
        }
    }

    fn commit_quick_switcher_selection(&mut self) {
        let Some(index) = self.quick_switcher.as_ref().and_then(|dropdown| {
            let selected = dropdown.selected_index();
            dropdown
                .visible_options()
                .into_iter()
                .find(|(index, _)| *index == selected)
                .or_else(|| dropdown.visible_options().into_iter().next())
                .map(|(index, _)| index)
        }) else {
            return;
        };
        self.commit_quick_switcher_index(index);
    }

    fn commit_quick_switcher_index(&mut self, index: usize) {
        let Some(choice) = self
            .quick_switcher
            .as_ref()
            .and_then(|dropdown| dropdown.options().get(index))
            .map(|option| option.value)
        else {
            return;
        };
        self.quick_switcher = None;
        self.run_quick_action(choice);
    }

    fn run_quick_action(&mut self, action: QuickAction) {
        match action {
            QuickAction::CommandLog => self.open_dialog(DialogKind::CommandLog),
            QuickAction::ThemePicker => self.open_theme_dropdown(),
            QuickAction::ProjectPicker => self.open_project_dropdown(),
            QuickAction::ReloadList => self.reload_list(),
            QuickAction::ReloadBoard => self.reload_board(),
            QuickAction::Board => self.select_tab("Board"),
            QuickAction::List => self.select_tab("List"),
            QuickAction::Timeline => self.select_tab("Timeline"),
            QuickAction::Filters => self.select_tab("Filters"),
        }
    }

    fn select_tab(&mut self, title: &str) {
        if let Some(index) = APP_TABS.iter().position(|tab| *tab == title) {
            self.tabs.set_selected(index);
            self.screen = Screen::Main;
            self.filtered_tree.clear_transient_input();
            self.close_overlays();
        }
    }

    fn dispatch_assignee_dropdown(
        &mut self,
        action: crate::components::generic::dropdown::DropdownAction,
    ) {
        let Some(dropdown) = &mut self.assignee_dropdown else {
            return;
        };
        match dropdown.dispatch(action) {
            Some(DropdownEvent::Closed) => {
                self.assignee_dropdown = None;
            }
            Some(DropdownEvent::Toggled(index)) => self.commit_assignee_index(index),
            None => {}
        }
    }

    fn commit_assignee_index(&mut self, index: usize) {
        let Some(assignee) = self
            .assignee_dropdown
            .as_ref()
            .and_then(|dropdown| dropdown.options().get(index))
            .map(|option| option.value.clone())
        else {
            return;
        };
        self.assignee_dropdown = None;
        self.queue_selected_assignment(assignee);
    }

    fn assign_selected_to_me(&mut self) {
        let Some(current_user) = self.current_user.clone() else {
            self.status = String::from("Current Jira user is not loaded.");
            return;
        };
        self.queue_selected_assignment(Some(current_user));
    }

    fn queue_selected_assignment(&mut self, assignee: Option<UserSummary>) {
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
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        self.pending_assignment_requests
            .insert(issue_key.clone(), request_id);
        self.pending_effects.push(AppEffect::AssignIssue {
            request_id,
            issue_key,
            assignee,
            credentials,
        });
    }
    fn dispatch_project_dropdown(
        &mut self,
        action: crate::components::generic::dropdown::DropdownAction,
    ) {
        let Some(dropdown) = &mut self.project_dropdown else {
            return;
        };
        match dropdown.dispatch(action) {
            Some(DropdownEvent::Closed) => {
                self.project_dropdown = None;
            }
            Some(DropdownEvent::Toggled(index)) => {
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
        self.status = format!("Loading Jira project {}", project.key);
        self.queue_jira_load(
            JiraLoadPurpose::SwitchProject,
            credentials,
            crate::services::jira::ROOT_PAGE_SIZE,
        );
    }

    fn submit_setup(&mut self) {
        let Some(credentials) = self.setup.credentials() else {
            self.status = String::from("All Jira credential fields are required.");
            return;
        };

        self.status = String::from("Loading Jira issues");
        self.credentials = Some(credentials.clone());
        self.filtered_tree.set_jira_site(credentials.site.clone());
        self.queue_jira_load(
            JiraLoadPurpose::Setup,
            credentials,
            crate::services::jira::ROOT_PAGE_SIZE,
        );
    }

    fn reload_board(&mut self) {
        if !self.tabs.is_active(APP_TABS, "Board") {
            return;
        }

        let Some(credentials) = self.credentials.clone() else {
            self.status = String::from("No Jira credentials available for reload.");
            return;
        };

        self.status = String::from("Reloading Jira board...");
        let request_id = self.next_request_id();
        self.active_load_request_id = Some(request_id);
        self.pending_effects.push(AppEffect::ReloadBoardOnly {
            request_id,
            credentials,
        });
    }
}

fn dropdown_dimensions<T>(
    dropdown: &MultiSelectDropdownState<T>,
    area: Rect,
    minimum_width: u16,
    max_rows: u16,
) -> (u16, u16) {
    let longest = dropdown
        .options()
        .iter()
        .map(|option| option.label.chars().count())
        .max()
        .unwrap_or(0) as u16;
    let width = area.width.min((longest + 6).max(minimum_width));
    let rows = dropdown.visible_row_count().min(max_rows as usize) as u16;
    let height = area.height.min((rows + 3).max(5));
    (width, height.saturating_sub(3))
}

fn dropdown_index_at<T>(
    dropdown: Option<&MultiSelectDropdownState<T>>,
    row: usize,
    height: usize,
) -> Option<usize> {
    dropdown?
        .visible_window(height)
        .into_iter()
        .filter_map(|entry| match entry {
            DropdownVisibleOption::Option { index, .. } => Some(index),
            _ => None,
        })
        .nth(row)
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

fn inset_rect(area: Rect, x: u16, y: u16) -> Rect {
    Rect {
        x: area.x.saturating_add(x),
        y: area.y.saturating_add(y),
        width: area.width.saturating_sub(x.saturating_mul(2)),
        height: area.height.saturating_sub(y.saturating_mul(2)),
    }
}

fn contains_point(area: Rect, point: (u16, u16)) -> bool {
    point.0 >= area.x
        && point.0 < area.x.saturating_add(area.width)
        && point.1 >= area.y
        && point.1 < area.y.saturating_add(area.height)
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

use std::{env, fs, io, path::PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    app::{Action, BoardAction, QuickAction, Screen, SetupAction},
    components::{
        generic::{
            dropdown::DropdownAction, filter::FilterAction, filtered_tree::FilteredTreeAction,
            tabs::TabAction, tree::TreeAction,
        },
        jira::filtered_tree::JiraFilteredTreeAction,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpScope {
    Local,
    Global,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpItem {
    pub scope: HelpScope,
    pub binding: String,
    pub summary: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpDialogAction {
    Close,
    Up,
    Down,
    PageUp,
    PageDown,
    First,
    Last,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBindings {
    tabs: TabsKeyBindings,
    board: BoardKeyBindings,
    tree: TreeKeyBindings,
    quit: KeySpec,
    reload_node: KeySpec,
    reload_list: KeySpec,
    board_details: KeySpec,
    board_group: KeySpec,
    leader: KeySpec,
    leader_command_log: KeySpec,
    leader_project: KeySpec,
    leader_theme: KeySpec,
    leader_board: KeySpec,
    leader_list: KeySpec,
    leader_timeline: KeySpec,
    leader_filters: KeySpec,
    quick_switcher: KeySpec,
    open_help: KeySpec,
    dropdown: DropdownKeyBindings,
    help: HelpKeyBindings,
    setup_next_field: KeySpec,
    setup_previous_field: KeySpec,
    setup_submit: KeySpec,
    setup_backspace: KeySpec,
    setup_quit: KeySpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TabsKeyBindings {
    previous: KeySpec,
    next: KeySpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TreeKeyBindings {
    move_up: KeySpec,
    move_down: KeySpec,
    half_page_up: KeySpec,
    half_page_down: KeySpec,
    collapse: KeySpec,
    expand: KeySpec,
    toggle_expand: KeySpec,
    collapse_all: KeySpec,
    open_columns: KeySpec,
    yank_issue_url: KeySpec,
    open_assignee: KeySpec,
    assign_to_me: KeySpec,
    unassign: KeySpec,
    go_to_end: KeySpec,
    go_to_start_prefix: KeySpec,
    focus_filter: KeySpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BoardKeyBindings {
    move_left: Vec<KeySpec>,
    move_right: Vec<KeySpec>,
    move_up: Vec<KeySpec>,
    move_down: Vec<KeySpec>,
    page_up: Vec<KeySpec>,
    page_down: Vec<KeySpec>,
    first: Vec<KeySpec>,
    last: Vec<KeySpec>,
}

impl Default for BoardKeyBindings {
    fn default() -> Self {
        Self {
            move_left: vec![KeySpec::plain('h')],
            move_right: vec![KeySpec::plain('l')],
            move_up: vec![KeySpec::plain('k')],
            move_down: vec![KeySpec::plain('j')],
            page_up: vec![
                KeySpec::code_with_modifiers(KeyCode::Char('u'), KeyModifiers::CONTROL),
                KeySpec::code(KeyCode::PageUp),
            ],
            page_down: vec![
                KeySpec::code_with_modifiers(KeyCode::Char('d'), KeyModifiers::CONTROL),
                KeySpec::code(KeyCode::PageDown),
            ],
            first: vec![KeySpec::code(KeyCode::Home)],
            last: vec![
                KeySpec::code(KeyCode::End),
                KeySpec::code_with_modifiers(KeyCode::Char('g'), KeyModifiers::SHIFT),
            ],
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct DropdownKeyBindings {
    close: KeySpec,
    focus_filter: KeySpec,
    submit: KeySpec,
    toggle_selected: KeySpec,
    move_up: KeySpec,
    move_down: KeySpec,
    half_page_up: KeySpec,
    half_page_down: KeySpec,
    first: KeySpec,
    last: KeySpec,
}

impl Default for DropdownKeyBindings {
    fn default() -> Self {
        Self {
            close: KeySpec::code(KeyCode::Esc),
            focus_filter: KeySpec::plain('/'),
            submit: KeySpec::code(KeyCode::Enter),
            toggle_selected: KeySpec::plain(' '),
            move_up: KeySpec::plain('k'),
            move_down: KeySpec::plain('j'),
            half_page_up: KeySpec::code_with_modifiers(KeyCode::Char('u'), KeyModifiers::CONTROL),
            half_page_down: KeySpec::code_with_modifiers(KeyCode::Char('d'), KeyModifiers::CONTROL),
            first: KeySpec::plain('g'),
            last: KeySpec::shifted('g'),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HelpKeyBindings {
    close: KeySpec,
    move_up: KeySpec,
    move_down: KeySpec,
    page_up: KeySpec,
    page_down: KeySpec,
    first: KeySpec,
    last: KeySpec,
}

impl Default for TabsKeyBindings {
    fn default() -> Self {
        Self {
            previous: KeySpec::plain('['),
            next: KeySpec::plain(']'),
        }
    }
}

impl Default for HelpKeyBindings {
    fn default() -> Self {
        Self {
            close: KeySpec::code(KeyCode::Esc),
            move_up: KeySpec::plain('k'),
            move_down: KeySpec::plain('j'),
            page_up: KeySpec::code_with_modifiers(KeyCode::Char('u'), KeyModifiers::CONTROL),
            page_down: KeySpec::code_with_modifiers(KeyCode::Char('d'), KeyModifiers::CONTROL),
            first: KeySpec::plain('g'),
            last: KeySpec::shifted('g'),
        }
    }
}

impl Default for TreeKeyBindings {
    fn default() -> Self {
        Self {
            move_up: KeySpec::plain('k'),
            move_down: KeySpec::plain('j'),
            half_page_up: KeySpec::code_with_modifiers(KeyCode::Char('u'), KeyModifiers::CONTROL),
            half_page_down: KeySpec::code_with_modifiers(KeyCode::Char('d'), KeyModifiers::CONTROL),
            collapse: KeySpec::plain('h'),
            expand: KeySpec::plain('l'),
            toggle_expand: KeySpec::plain(' '),
            collapse_all: KeySpec::plain('z'),
            open_columns: KeySpec::plain('c'),
            yank_issue_url: KeySpec::plain('y'),
            open_assignee: KeySpec::plain('a'),
            assign_to_me: KeySpec::plain('i'),
            unassign: KeySpec::plain('u'),
            go_to_end: KeySpec::code_with_modifiers(KeyCode::Char('g'), KeyModifiers::SHIFT),
            go_to_start_prefix: KeySpec::plain('g'),
            focus_filter: KeySpec::plain('/'),
        }
    }
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            tabs: TabsKeyBindings::default(),
            board: BoardKeyBindings::default(),
            tree: TreeKeyBindings::default(),
            quit: KeySpec::code_with_modifiers(KeyCode::Char('q'), KeyModifiers::CONTROL),
            reload_node: KeySpec::plain('r'),
            reload_list: KeySpec::shifted('r'),
            board_details: KeySpec::plain('d'),
            board_group: KeySpec::plain('r'),
            leader: KeySpec::code_with_modifiers(KeyCode::Char('x'), KeyModifiers::CONTROL),
            leader_command_log: KeySpec::plain('c'),
            leader_project: KeySpec::plain('p'),
            leader_theme: KeySpec::plain('s'),
            leader_board: KeySpec::plain('b'),
            leader_list: KeySpec::plain('l'),
            leader_timeline: KeySpec::plain('t'),
            leader_filters: KeySpec::plain('f'),
            quick_switcher: KeySpec::code_with_modifiers(KeyCode::Char('k'), KeyModifiers::CONTROL),
            open_help: KeySpec::plain('?'),
            dropdown: DropdownKeyBindings::default(),
            help: HelpKeyBindings::default(),
            setup_next_field: KeySpec::code(KeyCode::Tab),
            setup_previous_field: KeySpec::code(KeyCode::BackTab),
            setup_submit: KeySpec::code(KeyCode::Enter),
            setup_backspace: KeySpec::code(KeyCode::Backspace),
            setup_quit: KeySpec::code_with_modifiers(KeyCode::Char('c'), KeyModifiers::CONTROL),
        }
    }
}

impl KeyBindings {
    pub fn load() -> Self {
        let Some(path) = keybindings_path() else {
            return Self::default();
        };

        Self::load_from_path(path).unwrap_or_default()
    }

    pub fn load_from_path(path: impl Into<PathBuf>) -> io::Result<Self> {
        let text = fs::read_to_string(path.into())?;
        Ok(Self::from_toml_str(&text))
    }

    pub fn from_toml_str(text: &str) -> Self {
        let mut bindings = Self::default();
        let Ok(value) = toml::from_str::<toml::Table>(text) else {
            return bindings;
        };

        set_key(&value, "global", "quit", &mut bindings.quit);
        set_key(&value, "global", "reload_node", &mut bindings.reload_node);
        set_key(&value, "global", "reload_list", &mut bindings.reload_list);
        set_key(&value, "board", "details", &mut bindings.board_details);
        set_key(&value, "board", "group", &mut bindings.board_group);
        set_key(&value, "global", "leader", &mut bindings.leader);
        set_key(
            &value,
            "leader",
            "command_log",
            &mut bindings.leader_command_log,
        );
        set_key(&value, "leader", "project", &mut bindings.leader_project);
        set_key(&value, "leader", "theme", &mut bindings.leader_theme);
        set_key(&value, "leader", "board", &mut bindings.leader_board);
        set_key(&value, "leader", "list", &mut bindings.leader_list);
        set_key(&value, "leader", "timeline", &mut bindings.leader_timeline);
        set_key(&value, "leader", "filters", &mut bindings.leader_filters);
        set_key(
            &value,
            "global",
            "quick_switcher",
            &mut bindings.quick_switcher,
        );
        set_key(&value, "global", "open_help", &mut bindings.open_help);
        set_key(&value, "help", "close", &mut bindings.help.close);
        set_key(&value, "help", "move_up", &mut bindings.help.move_up);
        set_key(&value, "help", "move_down", &mut bindings.help.move_down);
        set_key(&value, "dropdown", "close", &mut bindings.dropdown.close);
        set_key(
            &value,
            "dropdown",
            "focus_filter",
            &mut bindings.dropdown.focus_filter,
        );
        set_key(&value, "dropdown", "submit", &mut bindings.dropdown.submit);
        set_key(
            &value,
            "dropdown",
            "toggle_selected",
            &mut bindings.dropdown.toggle_selected,
        );
        set_key(
            &value,
            "dropdown",
            "move_up",
            &mut bindings.dropdown.move_up,
        );
        set_key(
            &value,
            "dropdown",
            "move_down",
            &mut bindings.dropdown.move_down,
        );
        set_key(
            &value,
            "dropdown",
            "half_page_up",
            &mut bindings.dropdown.half_page_up,
        );
        set_key(
            &value,
            "dropdown",
            "half_page_down",
            &mut bindings.dropdown.half_page_down,
        );
        set_key(&value, "dropdown", "first", &mut bindings.dropdown.first);
        set_key(&value, "dropdown", "last", &mut bindings.dropdown.last);
        set_key(&value, "help", "page_up", &mut bindings.help.page_up);
        set_key(&value, "help", "page_down", &mut bindings.help.page_down);
        set_key(&value, "help", "first", &mut bindings.help.first);
        set_key(&value, "help", "last", &mut bindings.help.last);
        set_key(&value, "tabs", "previous_tab", &mut bindings.tabs.previous);
        set_key(&value, "tabs", "next_tab", &mut bindings.tabs.next);
        set_keys(&value, "board", "move_left", &mut bindings.board.move_left);
        set_keys(
            &value,
            "board",
            "move_right",
            &mut bindings.board.move_right,
        );
        set_keys(&value, "board", "move_up", &mut bindings.board.move_up);
        set_keys(&value, "board", "move_down", &mut bindings.board.move_down);
        set_keys(&value, "board", "page_up", &mut bindings.board.page_up);
        set_keys(&value, "board", "page_down", &mut bindings.board.page_down);
        set_keys(&value, "board", "first", &mut bindings.board.first);
        set_keys(&value, "board", "last", &mut bindings.board.last);
        set_key(&value, "tree", "move_up", &mut bindings.tree.move_up);
        set_key(&value, "tree", "move_down", &mut bindings.tree.move_down);
        set_key(
            &value,
            "tree",
            "half_page_up",
            &mut bindings.tree.half_page_up,
        );
        set_key(
            &value,
            "tree",
            "half_page_down",
            &mut bindings.tree.half_page_down,
        );
        set_key(&value, "tree", "collapse", &mut bindings.tree.collapse);
        set_key(&value, "tree", "expand", &mut bindings.tree.expand);
        set_key(
            &value,
            "tree",
            "toggle_expand",
            &mut bindings.tree.toggle_expand,
        );
        set_key(
            &value,
            "tree",
            "collapse_all",
            &mut bindings.tree.collapse_all,
        );
        set_key(
            &value,
            "tree",
            "open_columns",
            &mut bindings.tree.open_columns,
        );
        set_key(
            &value,
            "tree",
            "copy_issue_url",
            &mut bindings.tree.yank_issue_url,
        );
        set_key(
            &value,
            "tree",
            "yank_issue_url",
            &mut bindings.tree.yank_issue_url,
        );
        set_key(
            &value,
            "tree",
            "open_assignee",
            &mut bindings.tree.open_assignee,
        );
        set_key(
            &value,
            "tree",
            "assign_to_me",
            &mut bindings.tree.assign_to_me,
        );
        set_key(&value, "tree", "unassign", &mut bindings.tree.unassign);
        set_key(&value, "tree", "go_to_end", &mut bindings.tree.go_to_end);
        set_key(
            &value,
            "tree",
            "go_to_start_prefix",
            &mut bindings.tree.go_to_start_prefix,
        );
        set_key(&value, "tree", "search", &mut bindings.tree.focus_filter);
        set_key(
            &value,
            "setup",
            "next_field",
            &mut bindings.setup_next_field,
        );
        set_key(
            &value,
            "setup",
            "previous_field",
            &mut bindings.setup_previous_field,
        );
        set_key(&value, "setup", "submit", &mut bindings.setup_submit);
        set_key(&value, "setup", "backspace", &mut bindings.setup_backspace);
        set_key(&value, "setup", "quit", &mut bindings.setup_quit);

        bindings
    }

    pub fn global_action_for(&self, key: KeyEvent) -> Option<Action> {
        if self.open_help.matches(key) {
            Some(Action::OpenHelp)
        } else if self.leader.matches(key) {
            Some(Action::Leader)
        } else if self.quick_switcher.matches(key) {
            Some(Action::ToggleQuickSwitcher)
        } else if self.reload_list.matches(key) {
            Some(Action::ReloadList)
        } else if self.reload_node.matches(key) {
            Some(Action::ReloadNode)
        } else if is_ctrl_q(key) {
            Some(Action::Quit)
        } else {
            None
        }
    }

    pub fn is_forced_quit(&self, key: KeyEvent) -> bool {
        is_ctrl_q(key)
    }

    pub fn leader_action_for(&self, key: KeyEvent) -> Action {
        if self.leader_command_log.matches(key) {
            Action::ToggleCommandLog
        } else if self.leader_project.matches(key) {
            Action::ToggleProjectDropdown
        } else if self.leader_theme.matches(key) {
            Action::ToggleThemeDropdown
        } else if self.leader_board.matches(key) {
            Action::GoToBoard
        } else if self.leader_list.matches(key) {
            Action::GoToList
        } else if self.leader_timeline.matches(key) {
            Action::GoToTimeline
        } else if self.leader_filters.matches(key) {
            Action::GoToFilters
        } else {
            Action::None
        }
    }

    pub fn action_for(&self, key: KeyEvent) -> Action {
        self.jira_filtered_tree_action_for(key)
    }

    pub fn board_action_for(&self, key: KeyEvent) -> Action {
        if self.tabs.previous.matches(key) {
            Action::Tabs(TabAction::Previous)
        } else if self.tabs.next.matches(key) {
            Action::Tabs(TabAction::Next)
        } else if self.tree.focus_filter.matches(key) {
            Action::FocusBoardFilter
        } else if is_escape_key(key) || is_ctrl_left_bracket(key) {
            Action::ClearBoardFilter
        } else if self.board_details.matches(key) {
            Action::ToggleSprintDetails
        } else if key.code == KeyCode::Char(' ') {
            Action::Board(BoardAction::ToggleCollapse)
        } else if self.tree.collapse_all.matches(key) {
            Action::Board(BoardAction::CollapseAllGroups)
        } else if KeySpec::shifted('z').matches(key) {
            Action::Board(BoardAction::ExpandAllGroups)
        } else if self.tree.open_assignee.matches(key) {
            Action::ToggleAssigneeDropdown
        } else if self.tree.assign_to_me.matches(key) {
            Action::AssignSelectedToMe
        } else if self.tree.unassign.matches(key) {
            Action::UnassignSelected
        } else if matches_any(&self.board.move_left, key) {
            Action::Board(BoardAction::MoveLeft)
        } else if matches_any(&self.board.move_right, key) {
            Action::Board(BoardAction::MoveRight)
        } else if matches_any(&self.board.move_up, key) {
            Action::Board(BoardAction::MoveUp)
        } else if matches_any(&self.board.move_down, key) {
            Action::Board(BoardAction::MoveDown)
        } else if matches_any(&self.board.page_up, key) {
            Action::Board(BoardAction::HalfPageUp)
        } else if matches_any(&self.board.page_down, key) {
            Action::Board(BoardAction::HalfPageDown)
        } else if self.board_group.matches(key) {
            Action::ToggleBoardGrouping
        } else if self.tree.go_to_start_prefix.matches(key) {
            Action::Board(BoardAction::GoToStartPrefix)
        } else if matches_any(&self.board.first, key) {
            Action::Board(BoardAction::GoToStart)
        } else if matches_any(&self.board.last, key) {
            Action::Board(BoardAction::GoToEnd)
        } else {
            Action::None
        }
    }

    pub fn jira_filtered_tree_action_for(&self, key: KeyEvent) -> Action {
        if self.tabs.previous.matches(key) {
            Action::Tabs(TabAction::Previous)
        } else if self.tabs.next.matches(key) {
            Action::Tabs(TabAction::Next)
        } else if self.tree.open_columns.matches(key) {
            Action::JiraFilteredTree(JiraFilteredTreeAction::OpenColumns)
        } else if self.tree.open_assignee.matches(key) {
            Action::ToggleAssigneeDropdown
        } else if self.tree.assign_to_me.matches(key) {
            Action::AssignSelectedToMe
        } else if self.tree.unassign.matches(key) {
            Action::UnassignSelected
        } else if self.tree.yank_issue_url.matches(key) {
            Action::JiraFilteredTree(JiraFilteredTreeAction::YankIssueUrlPrefix)
        } else if let Some(action) = self.filtered_tree_action_for(key) {
            Action::JiraFilteredTree(JiraFilteredTreeAction::FilteredTree(action))
        } else {
            Action::None
        }
    }

    pub fn filtered_tree_action_for(&self, key: KeyEvent) -> Option<FilteredTreeAction> {
        if self.tree.move_up.matches(key) {
            Some(FilteredTreeAction::Tree(TreeAction::MoveUp))
        } else if self.tree.move_down.matches(key) {
            Some(FilteredTreeAction::Tree(TreeAction::MoveDown))
        } else if self.tree.half_page_up.matches(key) || key.code == KeyCode::PageUp {
            Some(FilteredTreeAction::Tree(TreeAction::HalfPageUp))
        } else if self.tree.half_page_down.matches(key) || key.code == KeyCode::PageDown {
            Some(FilteredTreeAction::Tree(TreeAction::HalfPageDown))
        } else if self.tree.collapse.matches(key) {
            Some(FilteredTreeAction::Tree(TreeAction::Collapse))
        } else if self.tree.expand.matches(key) {
            Some(FilteredTreeAction::Tree(TreeAction::Expand))
        } else if self.tree.toggle_expand.matches(key) {
            Some(FilteredTreeAction::Tree(TreeAction::ToggleExpanded))
        } else if self.tree.collapse_all.matches(key)
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            Some(FilteredTreeAction::Tree(TreeAction::CollapseAll))
        } else if self.tree.go_to_end.matches(key) || key.code == KeyCode::End {
            Some(FilteredTreeAction::Tree(TreeAction::GoToEnd))
        } else if key.code == KeyCode::Home {
            Some(FilteredTreeAction::Tree(TreeAction::GoToStart))
        } else if self.tree.go_to_start_prefix.matches(key) {
            Some(FilteredTreeAction::Tree(TreeAction::GotoPrefix))
        } else if self.tree.focus_filter.matches(key) {
            Some(FilteredTreeAction::FocusFilter)
        } else if is_escape_key(key) || is_ctrl_left_bracket(key) {
            Some(FilteredTreeAction::ClearFilter)
        } else {
            None
        }
    }

    pub fn command_log_action_for(&self, key: KeyEvent) -> Action {
        if is_escape_key(key) {
            return Action::CloseCommandLog;
        }
        if is_ctrl_q(key) {
            return Action::Quit;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return match key.code {
                KeyCode::Char('u') => Action::HalfPageCommandLog(-1),
                KeyCode::Char('d') => Action::HalfPageCommandLog(1),
                _ => Action::None,
            };
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => Action::ScrollCommandLog(-1),
            KeyCode::Down | KeyCode::Char('j') => Action::ScrollCommandLog(1),
            KeyCode::PageUp => Action::PageCommandLog(-1),
            KeyCode::PageDown => Action::PageCommandLog(1),
            KeyCode::Home => Action::CommandLogToStart,
            KeyCode::End | KeyCode::Char('G') => Action::CommandLogToEnd,
            KeyCode::Char('g') => Action::CommandLogToStartPrefix,
            _ => Action::None,
        }
    }

    pub fn sprint_details_action_for(&self, key: KeyEvent) -> Action {
        if is_escape_key(key) || is_ctrl_left_bracket(key) || self.board_details.matches(key) {
            Action::CloseSprintDetails
        } else if is_ctrl_q(key) {
            Action::Quit
        } else {
            Action::None
        }
    }

    pub fn help_dialog_action_for(&self, key: KeyEvent) -> HelpDialogAction {
        if self.open_help.matches(key) || self.help.close.matches(key) || is_escape_key(key) {
            HelpDialogAction::Close
        } else if self.help.move_up.matches(key) || key.code == KeyCode::Up {
            HelpDialogAction::Up
        } else if self.help.move_down.matches(key) || key.code == KeyCode::Down {
            HelpDialogAction::Down
        } else if self.help.page_up.matches(key) || matches!(key.code, KeyCode::PageUp) {
            HelpDialogAction::PageUp
        } else if self.help.page_down.matches(key) || matches!(key.code, KeyCode::PageDown) {
            HelpDialogAction::PageDown
        } else if self.help.first.matches(key) || matches!(key.code, KeyCode::Home) {
            HelpDialogAction::First
        } else if self.help.last.matches(key) || matches!(key.code, KeyCode::End) {
            HelpDialogAction::Last
        } else {
            HelpDialogAction::None
        }
    }

    pub fn open_columns_label(&self) -> String {
        self.tree.open_columns.label()
    }

    pub fn open_help_label(&self) -> String {
        self.open_help.label()
    }

    pub fn quick_action_shortcut_label(&self, action: QuickAction) -> String {
        match action {
            QuickAction::CommandLog => self.leader_shortcut_label(&self.leader_command_log),
            QuickAction::ThemePicker => self.leader_shortcut_label(&self.leader_theme),
            QuickAction::ProjectPicker => self.leader_shortcut_label(&self.leader_project),
            QuickAction::ReloadList => self.reload_list.label(),
            QuickAction::ReloadBoard => self.reload_list.label(),
            QuickAction::Board => self.leader_shortcut_label(&self.leader_board),
            QuickAction::List => self.leader_shortcut_label(&self.leader_list),
            QuickAction::Timeline => self.leader_shortcut_label(&self.leader_timeline),
            QuickAction::Filters => self.leader_shortcut_label(&self.leader_filters),
        }
    }

    pub fn board_hint_text(&self) -> String {
        format!(
            "{} search | {} details | {} reload | {} columns | {} cards | {} page | {}/{} groups | {} help",
            self.tree.focus_filter.label(),
            self.board_details.label(),
            self.reload_list.label(),
            key_labels(&self.board.move_left, &self.board.move_right),
            key_labels(&self.board.move_up, &self.board.move_down),
            key_labels(&self.board.page_up, &self.board.page_down),
            self.tree.collapse_all.label(),
            KeySpec::shifted('z').label(),
            self.open_help.label()
        )
    }

    fn leader_shortcut_label(&self, binding: &KeySpec) -> String {
        format!("{} {}", self.leader.label(), binding.label())
    }

    pub fn setup_hint_text(&self) -> String {
        format!(
            "{} next field | {} previous field | {} load issues | {} quit",
            self.setup_next_field.label(),
            self.setup_previous_field.label(),
            self.setup_submit.label(),
            self.setup_quit.label()
        )
    }

    pub fn list_hint_text(&self) -> String {
        format!(
            "{} search | {} columns | {} assignee | {} leader | {} actions | {} help",
            self.tree.focus_filter.label(),
            self.tree.open_columns.label(),
            self.tree.open_assignee.label(),
            self.leader.label(),
            self.quick_switcher.label(),
            self.open_help.label()
        )
    }

    pub fn column_dropdown_context_action_for(
        &self,
        key: KeyEvent,
    ) -> Option<JiraFilteredTreeAction> {
        self.tree
            .open_columns
            .matches(key)
            .then_some(JiraFilteredTreeAction::OpenColumns)
    }

    pub fn project_dropdown_action_for(&self, key: KeyEvent) -> DropdownAction {
        match self.dropdown_action_for(key) {
            JiraFilteredTreeAction::Dropdown(action) => action,
            _ => DropdownAction::None,
        }
    }

    pub fn help_items(
        &self,
        screen: Screen,
        active_tab: &str,
        dropdown_open: bool,
    ) -> Vec<HelpItem> {
        let mut items = Vec::new();

        if dropdown_open {
            items.push(self.help_item(
                HelpScope::Local,
                format!(
                    "{} / {}",
                    self.dropdown.toggle_selected.label(),
                    "Ctrl+Space"
                ),
                "Toggle selection",
                "Toggle current option (multi-select) or select it (single-select).",
            ));
            items.push(self.help_item(
                HelpScope::Local,
                "Ctrl+Enter".to_string(),
                "Do selection",
                "Select and submit the current option from the search input.",
            ));
            items.push(self.help_item(
                HelpScope::Local,
                "Ctrl+J / Ctrl+K".to_string(),
                "Navigate search options",
                "Move option selection down/up while typing in the search input.",
            ));
            items.push(self.help_item(
                HelpScope::Local,
                self.dropdown.focus_filter.label(),
                "Focus search",
                "Focus the search input to filter options.",
            ));
            items.push(self.help_item(
                HelpScope::Local,
                "Ctrl+C".to_string(),
                "Clear search",
                "Clear the search input text.",
            ));
            items.push(self.help_item(
                HelpScope::Local,
                "Esc".to_string(),
                "Clear / Close",
                "Clear search input focus (first press) and close the dropdown (second press).",
            ));
            items.push(self.help_item(
                HelpScope::Local,
                "j / k / Down / Up".to_string(),
                "Move selection",
                "Move selection down/up when search is not focused.",
            ));
            items.push(self.help_item(
                HelpScope::Local,
                "gg / G / Home / End".to_string(),
                "Start / End",
                "Jump to the first or last option.",
            ));
            items.push(self.help_item(
                HelpScope::Local,
                "ctrl+u / ctrl+d / PageUp / PageDown".to_string(),
                "Scroll page",
                "Page up/down through options.",
            ));
        } else {
            match screen {
                Screen::Setup => {
                    items.push(self.help_item(
                        HelpScope::Local,
                        self.setup_next_field.label(),
                        "Next field",
                        "Move focus to the next setup field.",
                    ));
                    items.push(self.help_item(
                        HelpScope::Local,
                        self.setup_previous_field.label(),
                        "Previous field",
                        "Move focus to the previous setup field.",
                    ));
                    items.push(self.help_item(
                        HelpScope::Local,
                        self.setup_submit.label(),
                        "Load issues",
                        "Save credentials and load Jira issues.",
                    ));
                    items.push(self.help_item(
                        HelpScope::Local,
                        self.setup_quit.label(),
                        "Quit",
                        "Exit without saving setup data.",
                    ));
                }
                Screen::Main => {
                    items.push(self.help_item(
                        HelpScope::Local,
                        format!("{}/{}", self.tabs.previous.label(), self.tabs.next.label()),
                        "Switch tabs",
                        "Move between the top-level Jira tabs.",
                    ));
                    if active_tab == "Board" {
                        items.push(self.help_item(
                            HelpScope::Local,
                            self.tree.focus_filter.label(),
                            "Search board",
                            "Focus the board search and narrow visible cards.",
                        ));
                        items.push(self.help_item(
                            HelpScope::Local,
                            self.board_details.label(),
                            "Sprint details",
                            "Show the active sprint's name, goal, dates, and time remaining.",
                        ));
                        items.push(self.help_item(
                            HelpScope::Local,
                            self.board_group.label(),
                            "Group board",
                            "Group the board by assignee, epic, stories, or spaces.",
                        ));
                        items.push(self.help_item(
                            HelpScope::Local,
                            key_labels(&self.board.move_left, &self.board.move_right),
                            "Move columns",
                            "Move to the nearest issue in the previous or next board column.",
                        ));
                        items.push(self.help_item(
                            HelpScope::Local,
                            key_labels(&self.board.move_up, &self.board.move_down),
                            "Move cards",
                            "Move to the previous or next issue in the current board column.",
                        ));
                        items.push(self.help_item(
                            HelpScope::Local,
                            key_list_label(&self.board.page_up)
                                + " / "
                                + &key_list_label(&self.board.page_down),
                            "Page cards",
                            "Move through board issues by half pages.",
                        ));
                        items.push(self.help_item(
                            HelpScope::Local,
                            key_list_label(&self.board.first)
                                + " / "
                                + &key_list_label(&self.board.last),
                            "Start / End",
                            "Jump to the first or last board issue.",
                        ));
                        items.push(self.help_item(
                            HelpScope::Local,
                            self.tree.open_assignee.label(),
                            "Assign",
                            "Open the assignee selector for the selected board card.",
                        ));
                        items.push(self.help_item(
                            HelpScope::Local,
                            self.tree.assign_to_me.label(),
                            "Assign to me",
                            "Assign the selected board card to the current Jira user.",
                        ));
                        items.push(self.help_item(
                            HelpScope::Local,
                            self.tree.unassign.label(),
                            "Unassign",
                            "Clear the selected board card assignee.",
                        ));
                        items.push(self.help_item(
                            HelpScope::Local,
                            self.reload_list.label(),
                            "Reload board",
                            "Reload the active Jira board.",
                        ));
                    }
                    if active_tab == "List" {
                        items.push(self.help_item(
                            HelpScope::Local,
                            self.tree.focus_filter.label(),
                            "Search issues",
                            "Focus the issue filter and narrow the current list.",
                        ));
                        items.push(self.help_item(
                            HelpScope::Local,
                            format!(
                                "{}/{}",
                                self.tree.move_up.label(),
                                self.tree.move_down.label()
                            ),
                            "Move selection",
                            format!(
                                "Move selection ({} / {} / PageUp / PageDown for paging, Home / End for edges).",
                                self.tree.half_page_up.label(),
                                self.tree.half_page_down.label()
                            ),
                        ));
                        items.push(self.help_item(
                            HelpScope::Local,
                            "gg / G / Home / End".to_string(),
                            "Start / End",
                            "Jump to the first or last visible issue.",
                        ));
                        items.push(self.help_item(
                            HelpScope::Local,
                            "ctrl+u / ctrl+d / PageUp / PageDown".to_string(),
                            "Scroll page",
                            "Move through the issue list by half pages.",
                        ));
                        items.push(self.help_item(
                            HelpScope::Local,
                            format!(
                                "{}/{}",
                                self.tree.collapse.label(),
                                self.tree.expand.label()
                            ),
                            "Collapse / expand",
                            "Collapse a branch or expand the selected parent row.",
                        ));
                        items.push(self.help_item(
                            HelpScope::Local,
                            self.tree.toggle_expand.label(),
                            "Toggle branch",
                            "Expand or collapse the selected tree row.",
                        ));
                        items.push(self.help_item(
                            HelpScope::Local,
                            self.tree.open_columns.label(),
                            "Columns",
                            "Open the column picker for the issue table.",
                        ));
                        items.push(self.help_item(
                            HelpScope::Local,
                            self.tree.open_assignee.label(),
                            "Assign",
                            "Open the assignee selector for the selected issue.",
                        ));
                        items.push(self.help_item(
                            HelpScope::Local,
                            self.tree.assign_to_me.label(),
                            "Assign to me",
                            "Assign the selected issue to the current Jira user.",
                        ));
                        items.push(self.help_item(
                            HelpScope::Local,
                            self.tree.unassign.label(),
                            "Unassign",
                            "Clear the selected issue assignee.",
                        ));
                        let yank_label = self.tree.yank_issue_url.label();
                        let yank_binding = if yank_label.len() == 1 {
                            format!("{}{}", yank_label.to_lowercase(), yank_label.to_lowercase())
                        } else {
                            format!("{} {}", yank_label, yank_label)
                        };
                        items.push(self.help_item(
                            HelpScope::Local,
                            yank_binding,
                            "Copy issue URL",
                            "Copy the selected Jira issue URL to the clipboard.",
                        ));
                        items.push(self.help_item(
                            HelpScope::Local,
                            self.reload_node.label(),
                            "Reload node",
                            "Reload the selected issue's children from Jira.",
                        ));
                        items.push(self.help_item(
                            HelpScope::Local,
                            self.reload_list.label(),
                            "Reload list",
                            "Reload all issues for the active Jira project.",
                        ));
                    }
                }
            }
        }
        self.append_global_help_items(&mut items);

        items
    }

    fn append_global_help_items(&self, items: &mut Vec<HelpItem>) {
        items.push(self.help_item(
            HelpScope::Global,
            self.leader.label(),
            "Leader key",
            format!(
                "Press leader, then {} log, {} project, {} theme, or {} / {} / {} / {} tabs.",
                self.leader_command_log.label(),
                self.leader_project.label(),
                self.leader_theme.label(),
                self.leader_board.label(),
                self.leader_list.label(),
                self.leader_timeline.label(),
                self.leader_filters.label()
            ),
        ));
        items.push(self.help_item(
            HelpScope::Global,
            format!(
                "{} {}",
                self.leader.label(),
                self.leader_command_log.label()
            ),
            "Command log",
            "Open the command log dialog.",
        ));
        items.push(self.help_item(
            HelpScope::Global,
            format!("{} {}", self.leader.label(), self.leader_project.label()),
            "Project picker",
            "Open the project picker centered on the current project.",
        ));
        items.push(self.help_item(
            HelpScope::Global,
            format!("{} {}", self.leader.label(), self.leader_theme.label()),
            "Theme picker",
            "Open the theme picker and preview themes while moving.",
        ));
        items.push(self.help_item(
            HelpScope::Global,
            format!("{} {}", self.leader.label(), self.leader_board.label()),
            "Go to Board",
            "Jump to the Board tab.",
        ));
        items.push(self.help_item(
            HelpScope::Global,
            format!("{} {}", self.leader.label(), self.leader_list.label()),
            "Go to List",
            "Jump to the List tab.",
        ));
        items.push(self.help_item(
            HelpScope::Global,
            format!("{} {}", self.leader.label(), self.leader_timeline.label()),
            "Go to Timeline",
            "Jump to the Timeline tab.",
        ));
        items.push(self.help_item(
            HelpScope::Global,
            format!("{} {}", self.leader.label(), self.leader_filters.label()),
            "Go to Filters",
            "Jump to the Filters tab.",
        ));
        items.push(self.help_item(
            HelpScope::Global,
            self.quick_switcher.label(),
            "Quick actions",
            "Open the centered quick actions menu.",
        ));
        items.push(self.help_item(
            HelpScope::Global,
            self.open_help.label(),
            "Close help",
            "Close the keyboard help dialog.",
        ));
    }

    fn help_item(
        &self,
        scope: HelpScope,
        binding: String,
        summary: impl Into<String>,
        description: impl Into<String>,
    ) -> HelpItem {
        HelpItem {
            scope,
            binding,
            summary: summary.into(),
            description: description.into(),
        }
    }

    pub fn theme_dropdown_action_for(&self, key: KeyEvent) -> DropdownAction {
        self.project_dropdown_action_for(key)
    }

    pub fn dropdown_action_for(&self, key: KeyEvent) -> JiraFilteredTreeAction {
        let action = if self.dropdown.close.matches(key) || is_ctrl_left_bracket(key) {
            DropdownAction::Close
        } else if self.dropdown.focus_filter.matches(key) {
            DropdownAction::FocusFilter
        } else if self.dropdown.half_page_up.matches(key) || key.code == KeyCode::PageUp {
            DropdownAction::HalfPageUp
        } else if self.dropdown.half_page_down.matches(key) || key.code == KeyCode::PageDown {
            DropdownAction::HalfPageDown
        } else if self.dropdown.last.matches(key) || key.code == KeyCode::End {
            DropdownAction::GoToEnd
        } else if key.code == KeyCode::Home {
            DropdownAction::GoToStart
        } else if self.dropdown.first.matches(key) {
            DropdownAction::GotoPrefix
        } else if self.dropdown.submit.matches(key) || self.dropdown.toggle_selected.matches(key) {
            DropdownAction::ToggleSelected
        } else if self.dropdown.move_up.matches(key) || key.code == KeyCode::Up {
            DropdownAction::MoveUp
        } else if self.dropdown.move_down.matches(key) || key.code == KeyCode::Down {
            DropdownAction::MoveDown
        } else {
            DropdownAction::None
        };
        JiraFilteredTreeAction::Dropdown(action)
    }

    #[allow(clippy::collapsible_if)]
    pub fn filter_action_for(&self, key: KeyEvent) -> FilterAction {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('a') => return FilterAction::MoveCursorStart,
                KeyCode::Char('e') => return FilterAction::MoveCursorEnd,
                KeyCode::Char('c') => return FilterAction::Clear,
                KeyCode::Left => return FilterAction::MoveCursorWordLeft,
                KeyCode::Right => return FilterAction::MoveCursorWordRight,
                KeyCode::Char('w') => return FilterAction::DeleteWordLeft,
                KeyCode::Delete => return FilterAction::DeleteWordRight,
                KeyCode::Char('f') => return FilterAction::MoveCursorRight,
                KeyCode::Char('b') => return FilterAction::MoveCursorLeft,
                KeyCode::Char('k') => return FilterAction::MoveSelectionUp,
                KeyCode::Char('j') => return FilterAction::MoveSelectionDown,
                KeyCode::Char('u') => return FilterAction::DeleteToStart,
                _ => {}
            }
        }

        if key.modifiers.contains(KeyModifiers::ALT) {
            match key.code {
                KeyCode::Char('d') => return FilterAction::DeleteWordRight,
                KeyCode::Char('f') => return FilterAction::MoveCursorWordRight,
                KeyCode::Char('b') => return FilterAction::MoveCursorWordLeft,
                _ => {}
            }
        }

        if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
            if let KeyCode::Char(c) = key.code {
                return FilterAction::Text(c);
            }
        }

        if is_ctrl_q(key) {
            FilterAction::Quit
        } else if is_escape_key(key)
            || (key.code == KeyCode::Enter && !key.modifiers.contains(KeyModifiers::CONTROL))
            || key.code == KeyCode::Char('/') && key.modifiers.contains(KeyModifiers::CONTROL)
            || is_ctrl_left_bracket(key)
        {
            FilterAction::Exit
        } else if key.code == KeyCode::Backspace {
            FilterAction::Backspace
        } else if key.modifiers.is_empty() {
            match key.code {
                KeyCode::Left => FilterAction::MoveCursorLeft,
                KeyCode::Right => FilterAction::MoveCursorRight,
                KeyCode::Delete => FilterAction::Delete,
                _ => FilterAction::None,
            }
        } else {
            FilterAction::None
        }
    }

    #[allow(clippy::collapsible_if)]
    pub fn setup_action_for(&self, key: KeyEvent) -> SetupAction {
        if self.setup_quit.matches(key) {
            return SetupAction::Quit;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('a') => return SetupAction::MoveCursorStart,
                KeyCode::Char('e') => return SetupAction::MoveCursorEnd,
                KeyCode::Char('c') => return SetupAction::Clear,
                KeyCode::Left => return SetupAction::MoveCursorWordLeft,
                KeyCode::Right => return SetupAction::MoveCursorWordRight,
                KeyCode::Char('w') => return SetupAction::DeleteWordLeft,
                KeyCode::Delete => return SetupAction::DeleteWordRight,
                KeyCode::Char('f') => return SetupAction::MoveCursorRight,
                KeyCode::Char('b') => return SetupAction::MoveCursorLeft,
                KeyCode::Char('k') => return SetupAction::DeleteToEnd,
                KeyCode::Char('u') => return SetupAction::DeleteToStart,
                _ => {}
            }
        }

        if key.modifiers.contains(KeyModifiers::ALT) {
            match key.code {
                KeyCode::Char('d') => return SetupAction::DeleteWordRight,
                KeyCode::Char('f') => return SetupAction::MoveCursorWordRight,
                KeyCode::Char('b') => return SetupAction::MoveCursorWordLeft,
                _ => {}
            }
        }

        if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
            if let KeyCode::Char(c) = key.code {
                return SetupAction::Text(c);
            }
        }

        if self.setup_next_field.matches(key) {
            SetupAction::NextField
        } else if self.setup_previous_field.matches(key) {
            SetupAction::PreviousField
        } else if self.setup_submit.matches(key) {
            SetupAction::Submit
        } else if self.setup_backspace.matches(key) {
            SetupAction::Backspace
        } else if self.setup_quit.matches(key) {
            SetupAction::Quit
        } else if key.modifiers.is_empty() {
            match key.code {
                KeyCode::Left => SetupAction::MoveCursorLeft,
                KeyCode::Right => SetupAction::MoveCursorRight,
                KeyCode::Delete => SetupAction::Delete,
                _ => SetupAction::None,
            }
        } else {
            SetupAction::None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeySpec {
    code: KeyCode,
    modifiers: KeyModifiers,
}

impl KeySpec {
    const fn plain(c: char) -> Self {
        Self {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE,
        }
    }

    const fn shifted(c: char) -> Self {
        Self {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::SHIFT,
        }
    }

    const fn code(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::NONE,
        }
    }

    const fn code_with_modifiers(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    fn matches(self, key: KeyEvent) -> bool {
        if self == KeySpec::from(key) {
            return true;
        }

        if let KeySpec {
            code: KeyCode::Char(expected),
            modifiers,
        } = self
            && modifiers == KeyModifiers::CONTROL
            && expected.is_ascii_lowercase()
            && let KeyCode::Char(actual) = key.code
        {
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && actual.to_ascii_lowercase() == expected
            {
                return true;
            }
            if key.modifiers.is_empty() && actual == control_code_for(expected) {
                return true;
            }
        }

        matches!(
            (self.code, self.modifiers, key.code, key.modifiers),
            (
                KeyCode::Char(expected),
                KeyModifiers::NONE,
                KeyCode::Char(actual),
                KeyModifiers::SHIFT
            ) if expected == actual && !actual.is_ascii_alphanumeric()
        )
    }

    pub fn label(self) -> String {
        let key = match self.code {
            KeyCode::Char(' ') => String::from("Space"),
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Esc => String::from("Esc"),
            KeyCode::Enter => String::from("Enter"),
            KeyCode::Tab => String::from("Tab"),
            KeyCode::BackTab => String::from("Shift+Tab"),
            KeyCode::Backspace => String::from("Backspace"),
            KeyCode::Delete => String::from("Delete"),
            KeyCode::Left => String::from("Left"),
            KeyCode::Right => String::from("Right"),
            KeyCode::Up => String::from("Up"),
            KeyCode::Down => String::from("Down"),
            KeyCode::Home => String::from("Home"),
            KeyCode::End => String::from("End"),
            _ => format!("{:?}", self.code),
        };

        if self.modifiers.contains(KeyModifiers::CONTROL) {
            format!("Ctrl+{key}")
        } else if self.modifiers.contains(KeyModifiers::ALT) {
            format!("Alt+{key}")
        } else if self.modifiers.contains(KeyModifiers::SHIFT)
            && !matches!(self.code, KeyCode::BackTab)
        {
            format!("Shift+{key}")
        } else {
            key
        }
    }
}

impl From<KeyEvent> for KeySpec {
    fn from(value: KeyEvent) -> Self {
        match value.code {
            KeyCode::Char(c) if c.is_ascii_uppercase() => Self {
                code: KeyCode::Char(c.to_ascii_lowercase()),
                modifiers: value.modifiers | KeyModifiers::SHIFT,
            },
            code => Self {
                code,
                modifiers: value.modifiers,
            },
        }
    }
}

fn set_keys(value: &toml::Table, section: &str, key: &str, destination: &mut Vec<KeySpec>) {
    let Some(configured) = value.get(section).and_then(|section| section.get(key)) else {
        return;
    };
    let keys = if let Some(text) = configured.as_str() {
        parse_key(text).into_iter().collect::<Vec<_>>()
    } else if let Some(values) = configured.as_array() {
        values
            .iter()
            .filter_map(toml::Value::as_str)
            .filter_map(parse_key)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    if !keys.is_empty() {
        *destination = keys;
    }
}

fn matches_any(bindings: &[KeySpec], key: KeyEvent) -> bool {
    bindings.iter().any(|binding| binding.matches(key))
}

fn key_labels(primary: &[KeySpec], secondary: &[KeySpec]) -> String {
    let mut labels = Vec::new();
    for binding in primary.iter().chain(secondary) {
        let label = binding.label();
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    labels.join("/")
}

fn is_ctrl_q(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('q' | 'Q') if key.modifiers.contains(KeyModifiers::CONTROL))
        || matches!(key.code, KeyCode::Char('\u{11}') if key.modifiers.is_empty())
}
fn control_code_for(key: char) -> char {
    ((key as u8) & 0x1f) as char
}

fn key_list_label(bindings: &[KeySpec]) -> String {
    let mut labels = Vec::new();
    for binding in bindings {
        let label = binding.label();
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    labels.join(" / ")
}

fn set_key(value: &toml::Table, section: &str, key: &str, destination: &mut KeySpec) {
    if let Some(configured) = value
        .get(section)
        .and_then(|section| section.get(key))
        .and_then(toml::Value::as_str)
        .and_then(parse_key)
    {
        *destination = configured;
    }
}

fn parse_key(value: &str) -> Option<KeySpec> {
    let value = value.trim();

    if let Some(rest) = value.strip_prefix("ctrl+") {
        let mut chars = rest.chars();
        let key = chars.next()?;
        if chars.next().is_none() {
            return Some(KeySpec {
                code: KeyCode::Char(key),
                modifiers: KeyModifiers::CONTROL,
            });
        }
    }

    let code = match value {
        "esc" => KeyCode::Esc,
        "enter" => KeyCode::Enter,
        "tab" => KeyCode::Tab,
        "backtab" => KeyCode::BackTab,
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "space" => KeyCode::Char(' '),
        _ => {
            let mut chars = value.chars();
            let key = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            if key.is_ascii_uppercase() {
                return Some(KeySpec::shifted(key.to_ascii_lowercase()));
            }
            KeyCode::Char(key)
        }
    };

    Some(KeySpec {
        code,
        modifiers: KeyModifiers::NONE,
    })
}

fn is_escape_key(key: KeyEvent) -> bool {
    key.code == KeyCode::Esc
}

fn is_ctrl_left_bracket(key: KeyEvent) -> bool {
    key.code == KeyCode::Char('[') && key.modifiers.contains(KeyModifiers::CONTROL)
}

fn keybindings_path() -> Option<PathBuf> {
    let home = env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".tira/keybindings.toml"))
}

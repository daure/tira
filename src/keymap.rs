use std::{env, fs, io, path::PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    app::{
        Action, BoardAction, BoardTicketDirection, QuickAction, Screen, SetupAction,
        TicketDialogAction, TimelineAction,
    },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpContext {
    Normal,
    Dropdown,
    CommandLog,
    TicketDialog,
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
    nav: NavBindings,
    tabs: TabsKeyBindings,
    board: BoardKeyBindings,
    tree: TreeKeyBindings,
    command_log: CommandLogKeyBindings,
    close: Vec<KeySpec>,
    quit: KeySpec,
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
    quick_switcher: KeySpec,
    open_ticket: KeySpec,
    close_ticket: KeySpec,
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
struct NavBindings {
    up: Vec<KeySpec>,
    down: Vec<KeySpec>,
    half_page_up: Vec<KeySpec>,
    half_page_down: Vec<KeySpec>,
    page_up: Vec<KeySpec>,
    page_down: Vec<KeySpec>,
    goto_start_prefix: Vec<KeySpec>,
    goto_end: Vec<KeySpec>,
    home: Vec<KeySpec>,
    end: Vec<KeySpec>,
    arrow_up: Vec<KeySpec>,
    arrow_down: Vec<KeySpec>,
    scroll_left: Vec<KeySpec>,
    scroll_right: Vec<KeySpec>,
}

impl Default for NavBindings {
    fn default() -> Self {
        Self {
            up: vec![KeySpec::plain('k')],
            down: vec![KeySpec::plain('j')],
            half_page_up: vec![KeySpec::code_with_modifiers(
                KeyCode::Char('u'),
                KeyModifiers::CONTROL,
            )],
            half_page_down: vec![KeySpec::code_with_modifiers(
                KeyCode::Char('d'),
                KeyModifiers::CONTROL,
            )],
            page_up: vec![KeySpec::code(KeyCode::PageUp)],
            page_down: vec![KeySpec::code(KeyCode::PageDown)],
            goto_start_prefix: vec![KeySpec::plain('g')],
            goto_end: vec![KeySpec::shifted('g')],
            home: vec![KeySpec::code(KeyCode::Home)],
            end: vec![KeySpec::code(KeyCode::End)],
            arrow_up: vec![KeySpec::code(KeyCode::Up)],
            arrow_down: vec![KeySpec::code(KeyCode::Down)],
            scroll_left: vec![
                KeySpec::shifted('h'),
                KeySpec::code_with_modifiers(KeyCode::Left, KeyModifiers::SHIFT),
            ],
            scroll_right: vec![
                KeySpec::shifted('l'),
                KeySpec::code_with_modifiers(KeyCode::Right, KeyModifiers::SHIFT),
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TabsKeyBindings {
    previous: KeySpec,
    next: KeySpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TreeKeyBindings {
    move_up: Option<Vec<KeySpec>>,
    move_down: Option<Vec<KeySpec>>,
    half_page_up: Option<Vec<KeySpec>>,
    half_page_down: Option<Vec<KeySpec>>,
    collapse: KeySpec,
    expand: KeySpec,
    toggle_expand: KeySpec,
    collapse_all: KeySpec,
    collapse_all_aliases: Vec<KeySpec>,
    expand_all: KeySpec,
    open_columns: KeySpec,
    yank_issue_url: KeySpec,
    open_assignee: KeySpec,
    assign_to_me: KeySpec,
    unassign: KeySpec,
    go_to_end: Option<Vec<KeySpec>>,
    go_to_start_prefix: Option<Vec<KeySpec>>,
    focus_filter: KeySpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BoardKeyBindings {
    move_left: Vec<KeySpec>,
    move_right: Vec<KeySpec>,
    move_up: Option<Vec<KeySpec>>,
    move_down: Option<Vec<KeySpec>>,
    page_up: Option<Vec<KeySpec>>,
    page_down: Option<Vec<KeySpec>>,
    first: Option<Vec<KeySpec>>,
    last: Option<Vec<KeySpec>>,
    move_ticket_left: Vec<KeySpec>,
    move_ticket_right: Vec<KeySpec>,
    move_ticket_up: Vec<KeySpec>,
    move_ticket_down: Vec<KeySpec>,
    toggle_move_mode: KeySpec,
    place_move_mode: KeySpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommandLogKeyBindings {
    close: KeySpec,
    move_up: Option<Vec<KeySpec>>,
    move_down: Option<Vec<KeySpec>>,
    half_page_up: Option<Vec<KeySpec>>,
    half_page_down: Option<Vec<KeySpec>>,
    first: Option<Vec<KeySpec>>,
    last: Option<Vec<KeySpec>>,
}

impl Default for CommandLogKeyBindings {
    fn default() -> Self {
        Self {
            close: KeySpec::code(KeyCode::Esc),
            move_up: None,
            move_down: None,
            half_page_up: None,
            half_page_down: None,
            first: None,
            last: None,
        }
    }
}

impl Default for BoardKeyBindings {
    fn default() -> Self {
        Self {
            move_left: vec![KeySpec::plain('h')],
            move_right: vec![KeySpec::plain('l')],
            move_up: None,
            move_down: None,
            page_up: None,
            page_down: None,
            first: None,
            last: None,
            move_ticket_left: vec![KeySpec::shifted('h')],
            move_ticket_right: vec![KeySpec::shifted('l')],
            move_ticket_up: vec![KeySpec::shifted('k')],
            move_ticket_down: vec![KeySpec::shifted('j')],
            toggle_move_mode: KeySpec::plain('m'),
            place_move_mode: KeySpec::code(KeyCode::Enter),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct DropdownKeyBindings {
    close: KeySpec,
    focus_filter: KeySpec,
    submit: KeySpec,
    toggle_selected: KeySpec,
    submit_from_filter: KeySpec,
    toggle_from_filter: KeySpec,
    filter_move_up: KeySpec,
    filter_move_down: KeySpec,
    filter_clear: KeySpec,
    move_up: Option<Vec<KeySpec>>,
    move_down: Option<Vec<KeySpec>>,
    half_page_up: Option<Vec<KeySpec>>,
    half_page_down: Option<Vec<KeySpec>>,
    first: Option<Vec<KeySpec>>,
    last: Option<Vec<KeySpec>>,
}

impl Default for DropdownKeyBindings {
    fn default() -> Self {
        Self {
            close: KeySpec::code(KeyCode::Esc),
            focus_filter: KeySpec::plain('/'),
            submit: KeySpec::code(KeyCode::Enter),
            toggle_selected: KeySpec::plain(' '),
            submit_from_filter: KeySpec::code_with_modifiers(KeyCode::Enter, KeyModifiers::CONTROL),
            toggle_from_filter: KeySpec::code_with_modifiers(
                KeyCode::Char(' '),
                KeyModifiers::CONTROL,
            ),
            filter_move_up: KeySpec::code_with_modifiers(KeyCode::Char('k'), KeyModifiers::CONTROL),
            filter_move_down: KeySpec::code_with_modifiers(
                KeyCode::Char('j'),
                KeyModifiers::CONTROL,
            ),
            filter_clear: KeySpec::code_with_modifiers(KeyCode::Char('c'), KeyModifiers::CONTROL),
            move_up: None,
            move_down: None,
            half_page_up: None,
            half_page_down: None,
            first: None,
            last: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HelpKeyBindings {
    close: KeySpec,
    move_up: Option<Vec<KeySpec>>,
    move_down: Option<Vec<KeySpec>>,
    page_up: Option<Vec<KeySpec>>,
    page_down: Option<Vec<KeySpec>>,
    first: Option<Vec<KeySpec>>,
    last: Option<Vec<KeySpec>>,
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
            move_up: None,
            move_down: None,
            page_up: None,
            page_down: None,
            first: None,
            last: None,
        }
    }
}

impl Default for TreeKeyBindings {
    fn default() -> Self {
        Self {
            move_up: None,
            move_down: None,
            half_page_up: None,
            half_page_down: None,
            collapse: KeySpec::plain('h'),
            expand: KeySpec::plain('l'),
            toggle_expand: KeySpec::plain(' '),
            collapse_all: KeySpec::plain('z'),
            collapse_all_aliases: vec![KeySpec::code_with_modifiers(
                KeyCode::Char('c'),
                KeyModifiers::CONTROL,
            )],
            expand_all: KeySpec::shifted('z'),
            open_columns: KeySpec::plain('c'),
            yank_issue_url: KeySpec::plain('y'),
            open_assignee: KeySpec::plain('a'),
            assign_to_me: KeySpec::plain('i'),
            unassign: KeySpec::plain('u'),
            go_to_end: None,
            go_to_start_prefix: None,
            focus_filter: KeySpec::plain('/'),
        }
    }
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            nav: NavBindings::default(),
            tabs: TabsKeyBindings::default(),
            board: BoardKeyBindings::default(),
            tree: TreeKeyBindings::default(),
            command_log: CommandLogKeyBindings::default(),
            close: vec![
                KeySpec::code(KeyCode::Esc),
                KeySpec::code_with_modifiers(KeyCode::Char('['), KeyModifiers::CONTROL),
            ],
            quit: KeySpec::code_with_modifiers(KeyCode::Char('q'), KeyModifiers::CONTROL),
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
            quick_switcher: KeySpec::code_with_modifiers(KeyCode::Char('k'), KeyModifiers::CONTROL),
            open_ticket: KeySpec::code(KeyCode::Enter),
            close_ticket: KeySpec::code(KeyCode::Esc),
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

        set_keys(&value, "nav", "up", &mut bindings.nav.up);
        set_keys(&value, "nav", "down", &mut bindings.nav.down);
        set_keys(
            &value,
            "nav",
            "half_page_up",
            &mut bindings.nav.half_page_up,
        );
        set_keys(
            &value,
            "nav",
            "half_page_down",
            &mut bindings.nav.half_page_down,
        );
        set_keys(&value, "nav", "page_up", &mut bindings.nav.page_up);
        set_keys(&value, "nav", "page_down", &mut bindings.nav.page_down);
        set_keys(
            &value,
            "nav",
            "goto_start_prefix",
            &mut bindings.nav.goto_start_prefix,
        );
        set_keys(&value, "nav", "goto_end", &mut bindings.nav.goto_end);
        set_keys(&value, "nav", "home", &mut bindings.nav.home);
        set_keys(&value, "nav", "end", &mut bindings.nav.end);
        set_keys(&value, "nav", "arrow_up", &mut bindings.nav.arrow_up);
        set_keys(&value, "nav", "arrow_down", &mut bindings.nav.arrow_down);
        set_keys(&value, "nav", "scroll_left", &mut bindings.nav.scroll_left);
        set_keys(
            &value,
            "nav",
            "scroll_right",
            &mut bindings.nav.scroll_right,
        );

        set_keys(&value, "global", "close", &mut bindings.close);
        set_key(&value, "global", "quit", &mut bindings.quit);
        set_key(&value, "global", "reload_list", &mut bindings.reload_list);
        set_key(&value, "board", "details", &mut bindings.board_details);
        set_key(&value, "board", "group", &mut bindings.board_group);
        set_keys(
            &value,
            "board",
            "move_ticket_left",
            &mut bindings.board.move_ticket_left,
        );
        set_keys(
            &value,
            "board",
            "move_ticket_right",
            &mut bindings.board.move_ticket_right,
        );
        set_keys(
            &value,
            "board",
            "move_ticket_up",
            &mut bindings.board.move_ticket_up,
        );
        set_keys(
            &value,
            "board",
            "move_ticket_down",
            &mut bindings.board.move_ticket_down,
        );
        set_key(
            &value,
            "board",
            "toggle_move_mode",
            &mut bindings.board.toggle_move_mode,
        );
        set_key(
            &value,
            "board",
            "place_move_mode",
            &mut bindings.board.place_move_mode,
        );
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
        set_key(
            &value,
            "global",
            "quick_switcher",
            &mut bindings.quick_switcher,
        );
        set_key(&value, "ticket", "open", &mut bindings.open_ticket);
        set_key(&value, "ticket", "close", &mut bindings.close_ticket);
        set_key(
            &value,
            "command_log",
            "close",
            &mut bindings.command_log.close,
        );
        set_keys_opt(
            &value,
            "command_log",
            "move_up",
            &mut bindings.command_log.move_up,
        );
        set_keys_opt(
            &value,
            "command_log",
            "move_down",
            &mut bindings.command_log.move_down,
        );
        set_keys_opt(
            &value,
            "command_log",
            "half_page_up",
            &mut bindings.command_log.half_page_up,
        );
        set_keys_opt(
            &value,
            "command_log",
            "half_page_down",
            &mut bindings.command_log.half_page_down,
        );
        set_keys_opt(
            &value,
            "command_log",
            "first",
            &mut bindings.command_log.first,
        );
        set_keys_opt(
            &value,
            "command_log",
            "last",
            &mut bindings.command_log.last,
        );
        set_key(&value, "global", "open_help", &mut bindings.open_help);
        set_key(&value, "help", "close", &mut bindings.help.close);
        set_keys_opt(&value, "help", "move_up", &mut bindings.help.move_up);
        set_keys_opt(&value, "help", "move_down", &mut bindings.help.move_down);
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
            "submit_from_filter",
            &mut bindings.dropdown.submit_from_filter,
        );
        set_key(
            &value,
            "dropdown",
            "toggle_from_filter",
            &mut bindings.dropdown.toggle_from_filter,
        );
        set_key(
            &value,
            "dropdown",
            "filter_move_up",
            &mut bindings.dropdown.filter_move_up,
        );
        set_key(
            &value,
            "dropdown",
            "filter_move_down",
            &mut bindings.dropdown.filter_move_down,
        );
        set_key(
            &value,
            "dropdown",
            "filter_clear",
            &mut bindings.dropdown.filter_clear,
        );
        set_keys_opt(
            &value,
            "dropdown",
            "move_up",
            &mut bindings.dropdown.move_up,
        );
        set_keys_opt(
            &value,
            "dropdown",
            "move_down",
            &mut bindings.dropdown.move_down,
        );
        set_keys_opt(
            &value,
            "dropdown",
            "half_page_up",
            &mut bindings.dropdown.half_page_up,
        );
        set_keys_opt(
            &value,
            "dropdown",
            "half_page_down",
            &mut bindings.dropdown.half_page_down,
        );
        set_keys_opt(&value, "dropdown", "first", &mut bindings.dropdown.first);
        set_keys_opt(&value, "dropdown", "last", &mut bindings.dropdown.last);
        set_keys_opt(&value, "help", "page_up", &mut bindings.help.page_up);
        set_keys_opt(&value, "help", "page_down", &mut bindings.help.page_down);
        set_keys_opt(&value, "help", "first", &mut bindings.help.first);
        set_keys_opt(&value, "help", "last", &mut bindings.help.last);
        set_key(&value, "tabs", "previous_tab", &mut bindings.tabs.previous);
        set_key(&value, "tabs", "next_tab", &mut bindings.tabs.next);
        set_keys(&value, "board", "move_left", &mut bindings.board.move_left);
        set_keys(
            &value,
            "board",
            "move_right",
            &mut bindings.board.move_right,
        );
        set_keys_opt(&value, "board", "move_up", &mut bindings.board.move_up);
        set_keys_opt(&value, "board", "move_down", &mut bindings.board.move_down);
        set_keys_opt(&value, "board", "page_up", &mut bindings.board.page_up);
        set_keys_opt(&value, "board", "page_down", &mut bindings.board.page_down);
        set_keys_opt(&value, "board", "first", &mut bindings.board.first);
        set_keys_opt(&value, "board", "last", &mut bindings.board.last);
        set_keys_opt(&value, "tree", "move_up", &mut bindings.tree.move_up);
        set_keys_opt(&value, "tree", "move_down", &mut bindings.tree.move_down);
        set_keys_opt(
            &value,
            "tree",
            "half_page_up",
            &mut bindings.tree.half_page_up,
        );
        set_keys_opt(
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
        set_keys(
            &value,
            "tree",
            "collapse_all_aliases",
            &mut bindings.tree.collapse_all_aliases,
        );
        set_key(&value, "tree", "expand_all", &mut bindings.tree.expand_all);
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
        set_keys_opt(&value, "tree", "go_to_end", &mut bindings.tree.go_to_end);
        set_keys_opt(
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
        } else if self.quit.matches(key) || is_ctrl_q(key) {
            Some(Action::Quit)
        } else {
            None
        }
    }

    pub fn is_forced_quit(&self, key: KeyEvent) -> bool {
        is_ctrl_q(key)
    }

    fn close_matches(&self, key: KeyEvent) -> bool {
        matches_any(&self.close, key)
    }

    fn context_close_matches(&self, close: KeySpec, key: KeyEvent) -> bool {
        close.matches(key) || self.close_matches(key)
    }

    pub fn dropdown_close_matches(&self, key: KeyEvent) -> bool {
        self.context_close_matches(self.dropdown.close, key)
    }

    fn close_label(&self, close: KeySpec) -> String {
        join_labels(std::iter::once(&close).chain(self.close.iter()), " / ")
    }

    fn collapse_all_label(&self) -> String {
        join_labels(
            std::iter::once(&self.tree.collapse_all).chain(self.tree.collapse_all_aliases.iter()),
            " / ",
        )
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
        } else if self.close_matches(key) {
            Action::ClearBoardFilter
        } else if self.board_details.matches(key) {
            Action::ToggleSprintDetails
        } else if self.board.place_move_mode.matches(key) {
            Action::PlaceBoardTicketMoveMode
        } else if self.open_ticket.matches(key) {
            Action::OpenTicketDialog
        } else if self.tree.toggle_expand.matches(key) {
            Action::Board(BoardAction::ToggleCollapse)
        } else if self.tree.collapse_all.matches(key) {
            Action::Board(BoardAction::CollapseAllGroups)
        } else if self.tree.expand_all.matches(key) {
            Action::Board(BoardAction::ExpandAllGroups)
        } else if self.tree.open_assignee.matches(key) {
            Action::ToggleAssigneeDropdown
        } else if self.tree.assign_to_me.matches(key) {
            Action::AssignSelectedToMe
        } else if self.tree.unassign.matches(key) {
            Action::UnassignSelected
        } else if self.board.toggle_move_mode.matches(key) {
            Action::ToggleBoardTicketMoveMode
        } else if matches_any(&self.board.move_ticket_left, key) {
            Action::MoveBoardTicket(BoardTicketDirection::Left)
        } else if matches_any(&self.board.move_ticket_right, key) {
            Action::MoveBoardTicket(BoardTicketDirection::Right)
        } else if matches_any(&self.board.move_ticket_up, key) {
            Action::MoveBoardTicket(BoardTicketDirection::Up)
        } else if matches_any(&self.board.move_ticket_down, key) {
            Action::MoveBoardTicket(BoardTicketDirection::Down)
        } else if matches_any(&self.board.move_left, key) {
            Action::Board(BoardAction::MoveLeft)
        } else if matches_any(&self.board.move_right, key) {
            Action::Board(BoardAction::MoveRight)
        } else if matches_any(resolve(&self.board.move_up, &self.nav.up), key) {
            Action::Board(BoardAction::MoveUp)
        } else if matches_any(resolve(&self.board.move_down, &self.nav.down), key) {
            Action::Board(BoardAction::MoveDown)
        } else if matches_any(resolve(&self.board.page_up, &self.nav.half_page_up), key)
            || matches_any(&self.nav.page_up, key)
        {
            Action::Board(BoardAction::HalfPageUp)
        } else if matches_any(
            resolve(&self.board.page_down, &self.nav.half_page_down),
            key,
        ) || matches_any(&self.nav.page_down, key)
        {
            Action::Board(BoardAction::HalfPageDown)
        } else if self.board_group.matches(key) {
            Action::ToggleBoardGrouping
        } else if matches_any(&self.nav.goto_start_prefix, key) {
            Action::Board(BoardAction::GoToStartPrefix)
        } else if matches_any(resolve(&self.board.first, &self.nav.home), key) {
            Action::Board(BoardAction::GoToStart)
        } else if matches_any(resolve(&self.board.last, &self.nav.end), key)
            || matches_any(&self.nav.goto_end, key)
        {
            Action::Board(BoardAction::GoToEnd)
        } else {
            Action::None
        }
    }

    /// Resolves a configured horizontal-scroll key to a direction: -1 for left,
    /// +1 for right, or `None`. Shared by the List and Timeline tabs.
    fn horizontal_scroll_direction(&self, key: KeyEvent) -> Option<i32> {
        if matches_any(&self.nav.scroll_left, key) {
            Some(-1)
        } else if matches_any(&self.nav.scroll_right, key) {
            Some(1)
        } else {
            None
        }
    }

    /// Resolves a key on the Timeline tab. Reuses the List view's tree key
    /// resolution so every navigation key (j/k, half/full page, gg/G, Home/End)
    /// and expand/collapse behaves identically; adds tab switching and the
    /// Shift+H/L horizontal axis scroll and the shared tree filter.
    pub fn timeline_action_for(&self, key: KeyEvent) -> Action {
        if self.tabs.previous.matches(key) {
            Action::Tabs(TabAction::Previous)
        } else if self.tabs.next.matches(key) {
            Action::Tabs(TabAction::Next)
        } else if let Some(dir) = self.horizontal_scroll_direction(key) {
            if dir < 0 {
                Action::Timeline(TimelineAction::ScrollLeft)
            } else {
                Action::Timeline(TimelineAction::ScrollRight)
            }
        } else if self.open_ticket.matches(key) {
            Action::OpenTicketDialog
        } else if self.tree.focus_filter.matches(key) {
            Action::Timeline(TimelineAction::FocusFilter)
        } else if self.close_matches(key) {
            Action::Timeline(TimelineAction::ClearFilter)
        } else if let Some(FilteredTreeAction::Tree(tree_action)) =
            self.filtered_tree_action_for(key)
        {
            Action::Timeline(TimelineAction::Tree(tree_action))
        } else {
            Action::None
        }
    }

    pub fn jira_filtered_tree_action_for(&self, key: KeyEvent) -> Action {
        if self.tabs.previous.matches(key) {
            Action::Tabs(TabAction::Previous)
        } else if self.tabs.next.matches(key) {
            Action::Tabs(TabAction::Next)
        } else if let Some(dir) = self.horizontal_scroll_direction(key) {
            Action::ScrollListHorizontal(dir * LIST_H_SCROLL_STEP)
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
        } else if self.open_ticket.matches(key) {
            Action::OpenTicketDialog
        } else if let Some(action) = self.filtered_tree_action_for(key) {
            Action::JiraFilteredTree(JiraFilteredTreeAction::FilteredTree(action))
        } else {
            Action::None
        }
    }

    pub fn filtered_tree_action_for(&self, key: KeyEvent) -> Option<FilteredTreeAction> {
        if matches_any(resolve(&self.tree.move_up, &self.nav.up), key) {
            Some(FilteredTreeAction::Tree(TreeAction::MoveUp))
        } else if matches_any(resolve(&self.tree.move_down, &self.nav.down), key) {
            Some(FilteredTreeAction::Tree(TreeAction::MoveDown))
        } else if matches_any(
            resolve(&self.tree.half_page_up, &self.nav.half_page_up),
            key,
        ) || matches_any(&self.nav.page_up, key)
        {
            Some(FilteredTreeAction::Tree(TreeAction::HalfPageUp))
        } else if matches_any(
            resolve(&self.tree.half_page_down, &self.nav.half_page_down),
            key,
        ) || matches_any(&self.nav.page_down, key)
        {
            Some(FilteredTreeAction::Tree(TreeAction::HalfPageDown))
        } else if self.tree.collapse.matches(key) {
            Some(FilteredTreeAction::Tree(TreeAction::Collapse))
        } else if self.tree.expand.matches(key) {
            Some(FilteredTreeAction::Tree(TreeAction::Expand))
        } else if self.tree.toggle_expand.matches(key) {
            Some(FilteredTreeAction::Tree(TreeAction::ToggleExpanded))
        } else if self.tree.collapse_all.matches(key)
            || matches_any(&self.tree.collapse_all_aliases, key)
        {
            Some(FilteredTreeAction::Tree(TreeAction::CollapseAll))
        } else if self.tree.expand_all.matches(key) {
            Some(FilteredTreeAction::Tree(TreeAction::ExpandAll))
        } else if matches_any(resolve(&self.tree.go_to_end, &self.nav.goto_end), key)
            || matches_any(&self.nav.end, key)
        {
            Some(FilteredTreeAction::Tree(TreeAction::GoToEnd))
        } else if matches_any(&self.nav.home, key) {
            Some(FilteredTreeAction::Tree(TreeAction::GoToStart))
        } else if matches_any(
            resolve(&self.tree.go_to_start_prefix, &self.nav.goto_start_prefix),
            key,
        ) {
            Some(FilteredTreeAction::Tree(TreeAction::GotoPrefix))
        } else if self.tree.focus_filter.matches(key) {
            Some(FilteredTreeAction::FocusFilter)
        } else if self.close_matches(key) {
            Some(FilteredTreeAction::ClearFilter)
        } else {
            None
        }
    }

    pub fn command_log_action_for(&self, key: KeyEvent) -> Action {
        if self.context_close_matches(self.command_log.close, key) {
            return Action::CloseCommandLog;
        }
        if is_ctrl_q(key) {
            return Action::Quit;
        }
        if matches_any(resolve(&self.command_log.move_up, &self.nav.up), key)
            || matches_any(&self.nav.arrow_up, key)
        {
            Action::ScrollCommandLog(-1)
        } else if matches_any(resolve(&self.command_log.move_down, &self.nav.down), key)
            || matches_any(&self.nav.arrow_down, key)
        {
            Action::ScrollCommandLog(1)
        } else if matches_any(
            resolve(&self.command_log.half_page_up, &self.nav.half_page_up),
            key,
        ) {
            Action::HalfPageCommandLog(-1)
        } else if matches_any(
            resolve(&self.command_log.half_page_down, &self.nav.half_page_down),
            key,
        ) {
            Action::HalfPageCommandLog(1)
        } else if matches_any(&self.nav.page_up, key) {
            Action::PageCommandLog(-1)
        } else if matches_any(&self.nav.page_down, key) {
            Action::PageCommandLog(1)
        } else if matches_any(
            resolve(&self.command_log.first, &self.nav.goto_start_prefix),
            key,
        ) {
            Action::CommandLogToStartPrefix
        } else if matches_any(&self.nav.home, key) {
            Action::CommandLogToStart
        } else if matches_any(resolve(&self.command_log.last, &self.nav.goto_end), key)
            || matches_any(&self.nav.end, key)
        {
            Action::CommandLogToEnd
        } else {
            Action::None
        }
    }

    pub fn sprint_details_action_for(&self, key: KeyEvent) -> Action {
        if self.close_matches(key) || self.board_details.matches(key) {
            Action::CloseSprintDetails
        } else if is_ctrl_q(key) {
            Action::Quit
        } else {
            Action::None
        }
    }

    pub fn ticket_dialog_action_for(&self, key: KeyEvent) -> Action {
        if self.context_close_matches(self.close_ticket, key) {
            Action::CloseTicketDialog
        } else if is_ctrl_q(key) {
            Action::Quit
        } else if self.tabs.previous.matches(key) {
            Action::TicketDialog(TicketDialogAction::PreviousTab)
        } else if self.tabs.next.matches(key) {
            Action::TicketDialog(TicketDialogAction::NextTab)
        } else {
            Action::None
        }
    }

    pub fn help_dialog_action_for(&self, key: KeyEvent) -> HelpDialogAction {
        if self.open_help.matches(key) || self.context_close_matches(self.help.close, key) {
            HelpDialogAction::Close
        } else if matches_any(resolve(&self.help.move_up, &self.nav.up), key)
            || matches_any(&self.nav.arrow_up, key)
        {
            HelpDialogAction::Up
        } else if matches_any(resolve(&self.help.move_down, &self.nav.down), key)
            || matches_any(&self.nav.arrow_down, key)
        {
            HelpDialogAction::Down
        } else if matches_any(resolve(&self.help.page_up, &self.nav.half_page_up), key)
            || matches_any(&self.nav.page_up, key)
        {
            HelpDialogAction::PageUp
        } else if matches_any(resolve(&self.help.page_down, &self.nav.half_page_down), key)
            || matches_any(&self.nav.page_down, key)
        {
            HelpDialogAction::PageDown
        } else if matches_any(resolve(&self.help.first, &self.nav.goto_start_prefix), key)
            || matches_any(&self.nav.home, key)
        {
            HelpDialogAction::First
        } else if matches_any(resolve(&self.help.last, &self.nav.goto_end), key)
            || matches_any(&self.nav.end, key)
        {
            HelpDialogAction::Last
        } else {
            HelpDialogAction::None
        }
    }

    pub fn open_columns_label(&self) -> String {
        self.tree.open_columns.label()
    }

    pub fn board_details_label(&self) -> String {
        self.board_details.label()
    }

    pub fn board_group_label(&self) -> String {
        self.board_group.label()
    }

    pub fn open_help_label(&self) -> String {
        self.open_help.label()
    }

    /// Width (in cells) of the compact "{?} shortcuts" toolbar hint, including a
    /// one-cell trailing gap so it never butts against the frame border.
    pub fn shortcuts_hint_width(&self) -> u16 {
        (self.open_help.label().chars().count() + " shortcuts".chars().count() + 1) as u16
    }

    /// Width (in cells) of the non-collapsed "columns" toolbar trigger,
    /// reserving space for the rendered "olumns" text plus padding alongside the
    /// keybinding label.
    pub fn column_trigger_width(&self) -> u16 {
        (self.open_columns_label().chars().count() + " columns ".chars().count()) as u16
    }

    pub fn quick_action_shortcut_label(&self, action: QuickAction) -> String {
        match action {
            QuickAction::CommandLog => self.leader_shortcut_label(&self.leader_command_log),
            QuickAction::ThemePicker => self.leader_shortcut_label(&self.leader_theme),
            QuickAction::ProjectPicker => self.leader_shortcut_label(&self.leader_project),
            QuickAction::ReloadList => self.reload_list.label(),
            QuickAction::ReloadBoard => self.reload_list.label(),
            QuickAction::ReloadTimeline => self.reload_list.label(),
            QuickAction::Board => self.leader_shortcut_label(&self.leader_board),
            QuickAction::List => self.leader_shortcut_label(&self.leader_list),
            QuickAction::Timeline => self.leader_shortcut_label(&self.leader_timeline),
            QuickAction::Shortcuts => self.open_help.label(),
        }
    }

    pub fn board_hint_text(&self) -> String {
        let move_up = resolved_owned(&self.board.move_up, &self.nav.up);
        let move_down = resolved_owned(&self.board.move_down, &self.nav.down);
        let page_up = resolved_owned(&self.board.page_up, &self.board_shared_page_up());
        let page_down = resolved_owned(&self.board.page_down, &self.board_shared_page_down());
        format!(
            "{} search | {} details | {} reload | {} columns | {} cards | {} page | {}/{} groups | {} help",
            self.tree.focus_filter.label(),
            self.board_details.label(),
            self.reload_list.label(),
            join_labels(
                self.board.move_left.iter().chain(&self.board.move_right),
                "/"
            ),
            join_labels(move_up.iter().chain(&move_down), "/"),
            join_labels(page_up.iter().chain(&page_down), "/"),
            self.collapse_all_label(),
            self.tree.expand_all.label(),
            self.open_help.label()
        )
    }

    pub fn command_log_hint_text(&self) -> String {
        let move_up = resolved_owned(&self.command_log.move_up, &self.nav.up);
        let move_down = resolved_owned(&self.command_log.move_down, &self.nav.down);
        let page_up = resolved_owned(&self.command_log.half_page_up, &self.nav.half_page_up);
        let page_down = resolved_owned(&self.command_log.half_page_down, &self.nav.half_page_down);
        format!(
            "{} close | {} move | {} page | {} help",
            self.close_label(self.command_log.close),
            join_labels(
                move_up
                    .iter()
                    .chain(&move_down)
                    .chain(&self.nav.arrow_up)
                    .chain(&self.nav.arrow_down),
                "/"
            ),
            join_labels(
                page_up
                    .iter()
                    .chain(&page_down)
                    .chain(&self.nav.page_up)
                    .chain(&self.nav.page_down),
                "/"
            ),
            self.open_help.label()
        )
    }

    pub fn ticket_dialog_hint_text(&self) -> String {
        format!(
            "{} close | {}/{} tabs | {} help",
            self.close_label(self.close_ticket),
            self.tabs.previous.label(),
            self.tabs.next.label(),
            self.open_help.label()
        )
    }

    fn board_shared_page_up(&self) -> Vec<KeySpec> {
        self.nav
            .half_page_up
            .iter()
            .chain(&self.nav.page_up)
            .copied()
            .collect()
    }

    fn board_shared_page_down(&self) -> Vec<KeySpec> {
        self.nav
            .half_page_down
            .iter()
            .chain(&self.nav.page_down)
            .copied()
            .collect()
    }

    fn board_shared_last(&self) -> Vec<KeySpec> {
        self.nav
            .end
            .iter()
            .chain(&self.nav.goto_end)
            .copied()
            .collect()
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

    pub fn timeline_hint_text(&self) -> String {
        let move_up = resolved_owned(&self.tree.move_up, &self.nav.up);
        let move_down = resolved_owned(&self.tree.move_down, &self.nav.down);
        let scroll = join_labels(
            self.nav.scroll_left.iter().chain(&self.nav.scroll_right),
            "/",
        );
        format!(
            "{} search | {} move | {}/{} expand/collapse | {}/{} all | {scroll} scroll | {} leader | {} help",
            self.tree.focus_filter.label(),
            join_labels(move_up.iter().chain(&move_down), "/"),
            self.tree.expand.label(),
            self.tree.collapse.label(),
            self.tree.expand_all.label(),
            self.collapse_all_label(),
            self.leader.label(),
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
        let context = if dropdown_open {
            HelpContext::Dropdown
        } else {
            HelpContext::Normal
        };
        self.help_items_for_context(screen, active_tab, context)
    }

    pub fn help_items_for_context(
        &self,
        screen: Screen,
        active_tab: &str,
        context: HelpContext,
    ) -> Vec<HelpItem> {
        let mut items = match context {
            HelpContext::Dropdown => self.dropdown_help_items(),
            HelpContext::CommandLog => self.command_log_help_items(),
            HelpContext::TicketDialog => self.ticket_dialog_help_items(),
            HelpContext::Normal => match screen {
                Screen::Setup => self.setup_help_items(),
                Screen::Main => {
                    let mut items = vec![self.help_item(
                        HelpScope::Local,
                        format!("{}/{}", self.tabs.previous.label(), self.tabs.next.label()),
                        "Switch tabs",
                        "Move between the top-level Jira tabs.",
                    )];
                    if active_tab == "Board" {
                        items.extend(self.board_help_items());
                    }
                    if active_tab == "List" {
                        items.extend(self.list_help_items());
                    }
                    if active_tab == "Timeline" {
                        items.extend(self.timeline_help_items());
                    }
                    items
                }
            },
        };
        self.append_global_help_items(&mut items);

        items
    }

    fn command_log_help_items(&self) -> Vec<HelpItem> {
        let move_up = resolve(&self.command_log.move_up, &self.nav.up);
        let move_down = resolve(&self.command_log.move_down, &self.nav.down);
        let half_page_up = resolve(&self.command_log.half_page_up, &self.nav.half_page_up);
        let half_page_down = resolve(&self.command_log.half_page_down, &self.nav.half_page_down);
        let first = resolve(&self.command_log.first, &self.nav.goto_start_prefix);
        let last = resolve(&self.command_log.last, &self.nav.goto_end);
        vec![
            self.help_item(
                HelpScope::Local,
                self.close_label(self.command_log.close),
                "Close log",
                "Close the command log dialog.",
            ),
            self.help_item(
                HelpScope::Local,
                join_labels(
                    move_up
                        .iter()
                        .chain(move_down)
                        .chain(&self.nav.arrow_up)
                        .chain(&self.nav.arrow_down),
                    " / ",
                ),
                "Move log",
                "Scroll the command log up or down.",
            ),
            self.help_item(
                HelpScope::Local,
                join_labels(
                    half_page_up
                        .iter()
                        .chain(half_page_down)
                        .chain(&self.nav.page_up)
                        .chain(&self.nav.page_down),
                    " / ",
                ),
                "Page log",
                "Page through command log entries.",
            ),
            self.help_item(
                HelpScope::Local,
                join_labels(
                    first
                        .iter()
                        .chain(last)
                        .chain(&self.nav.home)
                        .chain(&self.nav.end),
                    " / ",
                ),
                "Start / End",
                "Jump to the first or last command log entry.",
            ),
        ]
    }

    fn ticket_dialog_help_items(&self) -> Vec<HelpItem> {
        vec![
            self.help_item(
                HelpScope::Local,
                self.close_label(self.close_ticket),
                "Close ticket",
                "Close the ticket dialog.",
            ),
            self.help_item(
                HelpScope::Local,
                format!(
                    "{} / {}",
                    self.tabs.previous.label(),
                    self.tabs.next.label()
                ),
                "Switch ticket tabs",
                "Move between ticket dialog tabs.",
            ),
        ]
    }

    fn dropdown_help_items(&self) -> Vec<HelpItem> {
        let mut items = Vec::new();
        items.push(self.help_item(
            HelpScope::Local,
            format!(
                "{} / {}",
                self.dropdown.toggle_selected.label(),
                self.dropdown.toggle_from_filter.label()
            ),
            "Toggle selection",
            "Toggle current option (multi-select) or select it (single-select).",
        ));
        items.push(self.help_item(
            HelpScope::Local,
            self.dropdown.submit_from_filter.label(),
            "Do selection",
            "Select and submit the current option from the search input.",
        ));
        items.push(self.help_item(
            HelpScope::Local,
            join_labels(
                [
                    &self.dropdown.filter_move_down,
                    &self.dropdown.filter_move_up,
                ],
                " / ",
            ),
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
            self.dropdown.filter_clear.label(),
            "Clear search",
            "Clear the search input text.",
        ));
        items.push(self.help_item(
            HelpScope::Local,
            self.close_label(self.dropdown.close),
            "Clear / Close",
            "Clear search input focus (first press) and close the dropdown (second press).",
        ));
        let move_down = resolve(&self.dropdown.move_down, &self.nav.down);
        let move_up = resolve(&self.dropdown.move_up, &self.nav.up);
        items.push(
            self.help_item(
                HelpScope::Local,
                join_labels(
                    move_down
                        .iter()
                        .chain(move_up)
                        .chain(&self.nav.arrow_down)
                        .chain(&self.nav.arrow_up),
                    " / ",
                ),
                "Move selection",
                "Move selection down/up when search is not focused.",
            ),
        );
        let prefix = resolve(&self.dropdown.first, &self.nav.goto_start_prefix);
        let goto_end = resolve(&self.dropdown.last, &self.nav.goto_end);
        let prefix_label = prefix
            .first()
            .map(|spec| doubled_label(&spec.label()))
            .unwrap_or_default();
        items.push(self.help_item(
            HelpScope::Local,
            format!(
                "{prefix_label} / {}",
                join_labels(
                    goto_end.iter().chain(&self.nav.home).chain(&self.nav.end),
                    " / "
                )
            ),
            "Start / End",
            "Jump to the first or last option.",
        ));
        let half_page_up = resolve(&self.dropdown.half_page_up, &self.nav.half_page_up);
        let half_page_down = resolve(&self.dropdown.half_page_down, &self.nav.half_page_down);
        items.push(
            self.help_item(
                HelpScope::Local,
                join_labels(
                    half_page_up
                        .iter()
                        .chain(half_page_down)
                        .chain(&self.nav.page_up)
                        .chain(&self.nav.page_down),
                    " / ",
                ),
                "Scroll page",
                "Page up/down through options.",
            ),
        );
        items
    }

    fn setup_help_items(&self) -> Vec<HelpItem> {
        let mut items = Vec::new();
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
        items
    }

    fn board_help_items(&self) -> Vec<HelpItem> {
        let mut items = Vec::new();
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
            self.open_ticket.label(),
            "Open ticket",
            "Open the selected board card in the ticket dialog.",
        ));
        items.push(self.help_item(
            HelpScope::Local,
            self.board_group.label(),
            "Group board",
            "Group the board by assignee, epic, stories, or spaces.",
        ));
        items.push(self.help_item(
            HelpScope::Local,
            join_labels(
                self.board.move_left.iter().chain(&self.board.move_right),
                "/",
            ),
            "Move columns",
            "Move to the nearest issue in the previous or next board column.",
        ));
        let move_up = resolved_owned(&self.board.move_up, &self.nav.up);
        let move_down = resolved_owned(&self.board.move_down, &self.nav.down);
        items.push(self.help_item(
            HelpScope::Local,
            join_labels(move_up.iter().chain(&move_down), "/"),
            "Move cards",
            "Move to the previous or next issue in the current board column.",
        ));
        items.push(self.help_item(
            HelpScope::Local,
            self.board.toggle_move_mode.label(),
            "Move mode",
            "Start moving the selected ticket, then press again to place it.",
        ));
        items.push(self.help_item(
            HelpScope::Local,
            self.board.place_move_mode.label(),
            "Place ticket",
            "Place the selected ticket while in move mode.",
        ));
        items.push(
            self.help_item(
                HelpScope::Local,
                join_labels(
                    self.board
                        .move_ticket_left
                        .iter()
                        .chain(&self.board.move_ticket_right)
                        .chain(&self.board.move_ticket_up)
                        .chain(&self.board.move_ticket_down),
                    "/",
                ),
                "Move ticket",
                "Move the selected ticket between columns or reorder it locally.",
            ),
        );
        let page_up = resolved_owned(&self.board.page_up, &self.board_shared_page_up());
        let page_down = resolved_owned(&self.board.page_down, &self.board_shared_page_down());
        items.push(self.help_item(
            HelpScope::Local,
            join_labels(&page_up, " / ") + " / " + &join_labels(&page_down, " / "),
            "Page cards",
            "Move through board issues by half pages.",
        ));
        let first = resolved_owned(&self.board.first, &self.nav.home);
        let last = resolved_owned(&self.board.last, &self.board_shared_last());
        items.push(self.help_item(
            HelpScope::Local,
            join_labels(&first, " / ") + " / " + &join_labels(&last, " / "),
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
        items
    }

    fn list_help_items(&self) -> Vec<HelpItem> {
        let mut items = Vec::new();
        items.push(self.help_item(
            HelpScope::Local,
            self.tree.focus_filter.label(),
            "Search issues",
            "Focus the issue filter and narrow the current list.",
        ));
        items.push(self.help_item(
            HelpScope::Local,
            self.open_ticket.label(),
            "Open ticket",
            "Open the selected issue in the ticket dialog.",
        ));
        let move_up = resolve(&self.tree.move_up, &self.nav.up);
        let move_down = resolve(&self.tree.move_down, &self.nav.down);
        let half_page_up = resolve(&self.tree.half_page_up, &self.nav.half_page_up);
        let half_page_down = resolve(&self.tree.half_page_down, &self.nav.half_page_down);
        let paging = join_labels(
            half_page_up
                .iter()
                .chain(half_page_down)
                .chain(&self.nav.page_up)
                .chain(&self.nav.page_down),
            " / ",
        );
        let edges = join_labels(self.nav.home.iter().chain(&self.nav.end), " / ");
        items.push(self.help_item(
            HelpScope::Local,
            join_labels(move_up.iter().chain(move_down), "/"),
            "Move selection",
            format!("Move selection ({paging} for paging, {edges} for edges)."),
        ));
        let prefix = resolve(&self.tree.go_to_start_prefix, &self.nav.goto_start_prefix);
        let goto_end = resolve(&self.tree.go_to_end, &self.nav.goto_end);
        let prefix_label = prefix
            .first()
            .map(|spec| doubled_label(&spec.label()))
            .unwrap_or_default();
        items.push(self.help_item(
            HelpScope::Local,
            format!(
                "{prefix_label} / {}",
                join_labels(
                    goto_end.iter().chain(&self.nav.home).chain(&self.nav.end),
                    " / "
                )
            ),
            "Start / End",
            "Jump to the first or last visible issue.",
        ));
        items.push(
            self.help_item(
                HelpScope::Local,
                join_labels(
                    half_page_up
                        .iter()
                        .chain(half_page_down)
                        .chain(&self.nav.page_up)
                        .chain(&self.nav.page_down),
                    " / ",
                ),
                "Scroll page",
                "Move through the issue list by half pages.",
            ),
        );
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
            join_labels(
                self.nav.scroll_left.iter().chain(&self.nav.scroll_right),
                " / ",
            ),
            "Scroll columns",
            "Pan the table left or right when columns overflow the width.",
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
        let yank_binding = doubled_label(&self.tree.yank_issue_url.label());
        items.push(self.help_item(
            HelpScope::Local,
            yank_binding,
            "Copy issue URL",
            "Copy the selected Jira issue URL to the clipboard.",
        ));
        items.push(self.help_item(
            HelpScope::Local,
            self.reload_list.label(),
            "Reload list",
            "Reload all issues for the active Jira project.",
        ));
        items
    }

    fn timeline_help_items(&self) -> Vec<HelpItem> {
        let move_up = resolve(&self.tree.move_up, &self.nav.up);
        let move_down = resolve(&self.tree.move_down, &self.nav.down);
        vec![
            self.help_item(
                HelpScope::Local,
                self.tree.focus_filter.label(),
                "Search",
                "Filter timeline epics and child issues locally with fuzzy matching.",
            ),
            self.help_item(
                HelpScope::Local,
                self.open_ticket.label(),
                "Open ticket",
                "Open the selected timeline item in the ticket dialog.",
            ),
            self.help_item(
                HelpScope::Local,
                join_labels(move_down.iter().chain(move_up), " / "),
                "Move selection",
                "Move the timeline selection between epics and child issues.",
            ),
            self.help_item(
                HelpScope::Local,
                join_labels(
                    [
                        &self.tree.expand,
                        &self.tree.collapse,
                        &self.tree.toggle_expand,
                    ],
                    " / ",
                ),
                "Expand / collapse",
                "Expand or collapse the selected epic to show its child issues.",
            ),
            self.help_item(
                HelpScope::Local,
                format!(
                    "{} / {}",
                    self.tree.expand_all.label(),
                    self.collapse_all_label()
                ),
                "Expand / collapse all",
                "Expand or collapse every loaded epic on the timeline.",
            ),
            self.help_item(
                HelpScope::Local,
                join_labels(
                    self.nav.scroll_left.iter().chain(&self.nav.scroll_right),
                    " / ",
                ),
                "Scroll timeline",
                "Scroll the timeline axis left/right through the months.",
            ),
            self.help_item(
                HelpScope::Local,
                self.reload_list.label(),
                "Reload timeline",
                "Reload the timeline epics and child issues from Jira.",
            ),
        ]
    }

    fn append_global_help_items(&self, items: &mut Vec<HelpItem>) {
        items.push(self.help_item(
            HelpScope::Global,
            self.leader.label(),
            "Leader key",
            format!(
                "Press leader, then {} log, {} project, {} theme, or {} / {} / {} tabs.",
                self.leader_command_log.label(),
                self.leader_project.label(),
                self.leader_theme.label(),
                self.leader_board.label(),
                self.leader_list.label(),
                self.leader_timeline.label()
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
            self.quick_switcher.label(),
            "Quick actions",
            "Open the centered quick actions menu.",
        ));
        items.push(self.help_item(
            HelpScope::Global,
            self.open_help.label(),
            "Close shortcuts",
            "Close the shortcuts dialog.",
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
        let action = if self.dropdown_close_matches(key) {
            DropdownAction::Close
        } else if self.dropdown.focus_filter.matches(key) {
            DropdownAction::FocusFilter
        } else if matches_any(
            resolve(&self.dropdown.half_page_up, &self.nav.half_page_up),
            key,
        ) || matches_any(&self.nav.page_up, key)
        {
            DropdownAction::HalfPageUp
        } else if matches_any(
            resolve(&self.dropdown.half_page_down, &self.nav.half_page_down),
            key,
        ) || matches_any(&self.nav.page_down, key)
        {
            DropdownAction::HalfPageDown
        } else if matches_any(resolve(&self.dropdown.last, &self.nav.goto_end), key)
            || matches_any(&self.nav.end, key)
        {
            DropdownAction::GoToEnd
        } else if matches_any(&self.nav.home, key) {
            DropdownAction::GoToStart
        } else if matches_any(
            resolve(&self.dropdown.first, &self.nav.goto_start_prefix),
            key,
        ) {
            DropdownAction::GotoPrefix
        } else if self.dropdown.submit.matches(key) || self.dropdown.toggle_selected.matches(key) {
            DropdownAction::ToggleSelected
        } else if matches_any(resolve(&self.dropdown.move_up, &self.nav.up), key)
            || matches_any(&self.nav.arrow_up, key)
        {
            DropdownAction::MoveUp
        } else if matches_any(resolve(&self.dropdown.move_down, &self.nav.down), key)
            || matches_any(&self.nav.arrow_down, key)
        {
            DropdownAction::MoveDown
        } else {
            DropdownAction::None
        };
        JiraFilteredTreeAction::Dropdown(action)
    }

    pub(crate) fn dropdown_filter_focused_action_for(&self, key: KeyEvent) -> DropdownAction {
        if self.dropdown.toggle_from_filter.matches(key)
            || self.dropdown.submit_from_filter.matches(key)
        {
            DropdownAction::ToggleSelected
        } else if self.dropdown.submit.matches(key) {
            DropdownAction::Filter(FilterAction::Submit)
        } else if self.dropdown.filter_move_up.matches(key) {
            DropdownAction::Filter(FilterAction::MoveSelectionUp)
        } else if self.dropdown.filter_move_down.matches(key) {
            DropdownAction::Filter(FilterAction::MoveSelectionDown)
        } else if self.dropdown.filter_clear.matches(key) {
            DropdownAction::Filter(FilterAction::Clear)
        } else if matches_any(
            resolve(&self.dropdown.half_page_up, &self.nav.half_page_up),
            key,
        ) || matches_any(&self.nav.page_up, key)
        {
            DropdownAction::HalfPageUp
        } else if matches_any(
            resolve(&self.dropdown.half_page_down, &self.nav.half_page_down),
            key,
        ) || matches_any(&self.nav.page_down, key)
        {
            DropdownAction::HalfPageDown
        } else if matches_any(resolve(&self.dropdown.last, &self.nav.goto_end), key)
            || matches_any(&self.nav.end, key)
        {
            DropdownAction::GoToEnd
        } else if matches_any(&self.nav.home, key) {
            DropdownAction::GoToStart
        } else if matches_any(
            resolve(&self.dropdown.first, &self.nav.goto_start_prefix),
            key,
        ) {
            DropdownAction::GotoPrefix
        } else if self.dropdown_close_matches(key) {
            DropdownAction::Close
        } else {
            DropdownAction::Filter(self.filter_action_for(key))
        }
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
        } else if self.close_matches(key)
            || (key.code == KeyCode::Enter && !key.modifiers.contains(KeyModifiers::CONTROL))
            || key.code == KeyCode::Char('/') && key.modifiers.contains(KeyModifiers::CONTROL)
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
            KeyCode::BackTab => String::from("⇧Tab"),
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
            format!("⌃{key}")
        } else if self.modifiers.contains(KeyModifiers::ALT) {
            format!("⌥{key}")
        } else if self.modifiers.contains(KeyModifiers::SHIFT)
            && !matches!(self.code, KeyCode::BackTab)
        {
            shifted_label(self.code, &key)
        } else {
            key
        }
    }
}

fn shifted_label(code: KeyCode, fallback: &str) -> String {
    match code {
        KeyCode::Char(c) if c.is_ascii_alphabetic() => c.to_ascii_uppercase().to_string(),
        KeyCode::Char(c) => c.to_string(),
        _ => format!("Shift+{fallback}"),
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

fn set_keys_opt(
    value: &toml::Table,
    section: &str,
    key: &str,
    destination: &mut Option<Vec<KeySpec>>,
) {
    let mut parsed = Vec::new();
    set_keys(value, section, key, &mut parsed);
    if !parsed.is_empty() {
        *destination = Some(parsed);
    }
}

fn resolve<'a>(ovr: &'a Option<Vec<KeySpec>>, shared: &'a [KeySpec]) -> &'a [KeySpec] {
    match ovr {
        Some(keys) if !keys.is_empty() => keys,
        _ => shared,
    }
}

fn resolved_owned(ovr: &Option<Vec<KeySpec>>, shared: &[KeySpec]) -> Vec<KeySpec> {
    match ovr {
        Some(keys) if !keys.is_empty() => keys.clone(),
        _ => shared.to_vec(),
    }
}

fn matches_any(bindings: &[KeySpec], key: KeyEvent) -> bool {
    bindings.iter().any(|binding| binding.matches(key))
}

/// Cells panned per keypress when scrolling the issue table horizontally.
const LIST_H_SCROLL_STEP: i32 = 12;

fn doubled_label(label: &str) -> String {
    if label.chars().count() == 1 {
        let lower = label.to_lowercase();
        format!("{lower}{lower}")
    } else {
        format!("{label} {label}")
    }
}

fn join_labels<'a>(bindings: impl IntoIterator<Item = &'a KeySpec>, sep: &str) -> String {
    let mut labels = Vec::new();
    for binding in bindings {
        let label = binding.label();
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    labels.join(sep)
}

fn is_ctrl_q(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('q' | 'Q') if key.modifiers.contains(KeyModifiers::CONTROL))
        || matches!(key.code, KeyCode::Char('\u{11}') if key.modifiers.is_empty())
}
fn control_code_for(key: char) -> char {
    ((key as u8) & 0x1f) as char
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

    if let Some(rest) = value.strip_prefix("shift+") {
        let mut chars = rest.chars();
        let key = chars.next()?;
        if chars.next().is_none() {
            return Some(KeySpec::shifted(key.to_ascii_lowercase()));
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

fn keybindings_path() -> Option<PathBuf> {
    let home = env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".tira/keybindings.toml"))
}

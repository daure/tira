use std::{env, fs, io, path::PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    app::{Action, SetupAction},
    components::{
        generic::{
            dropdown::DropdownAction, filter::FilterAction, filtered_tree::FilteredTreeAction,
            tabs::TabAction, tree::TreeAction,
        },
        jira::filtered_tree::JiraFilteredTreeAction,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBindings {
    tabs: TabsKeyBindings,
    tree: TreeKeyBindings,
    quit: KeySpec,
    reload_list: KeySpec,
    toggle_command_log: KeySpec,
    switch_project: KeySpec,
    switch_theme: KeySpec,
    open_help: KeySpec,
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
    expand_all: KeySpec,
    open_columns: KeySpec,
    yank_issue_url: KeySpec,
    go_to_end: KeySpec,
    go_to_start_prefix: KeySpec,
    focus_filter: KeySpec,
}

impl Default for TabsKeyBindings {
    fn default() -> Self {
        Self {
            previous: KeySpec::plain('['),
            next: KeySpec::plain(']'),
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
            expand_all: KeySpec::shifted('z'),
            open_columns: KeySpec::plain('c'),
            yank_issue_url: KeySpec::plain('y'),
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
            tree: TreeKeyBindings::default(),
            quit: KeySpec::plain('q'),
            reload_list: KeySpec::shifted('r'),
            toggle_command_log: KeySpec::shifted('l'),
            switch_project: KeySpec::shifted('p'),
            switch_theme: KeySpec::code_with_modifiers(KeyCode::Char('t'), KeyModifiers::CONTROL),
            open_help: KeySpec::plain('?'),
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
        set_key(&value, "global", "reload_list", &mut bindings.reload_list);
        set_key(
            &value,
            "global",
            "toggle_command_log",
            &mut bindings.toggle_command_log,
        );
        set_key(
            &value,
            "global",
            "switch_project",
            &mut bindings.switch_project,
        );
        set_key(&value, "global", "switch_theme", &mut bindings.switch_theme);
        set_key(&value, "global", "open_help", &mut bindings.open_help);
        set_key(&value, "tabs", "previous_tab", &mut bindings.tabs.previous);
        set_key(&value, "tabs", "next_tab", &mut bindings.tabs.next);
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
        } else if self.toggle_command_log.matches(key) {
            Some(Action::ToggleCommandLog)
        } else if self.switch_project.matches(key) {
            Some(Action::ToggleProjectDropdown)
        } else if self.switch_theme.matches(key) {
            Some(Action::ToggleThemeDropdown)
        } else if self.reload_list.matches(key) {
            Some(Action::ReloadList)
        } else if self.quit.matches(key)
            || (key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            Some(Action::Quit)
        } else {
            None
        }
    }

    pub fn action_for(&self, key: KeyEvent) -> Action {
        self.jira_filtered_tree_action_for(key)
    }

    pub fn jira_filtered_tree_action_for(&self, key: KeyEvent) -> Action {
        if self.tabs.previous.matches(key) {
            Action::Tabs(TabAction::Previous)
        } else if self.tabs.next.matches(key) {
            Action::Tabs(TabAction::Next)
        } else if self.tree.open_columns.matches(key) {
            Action::JiraFilteredTree(JiraFilteredTreeAction::OpenColumns)
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
        } else if self.tree.half_page_up.matches(key) {
            Some(FilteredTreeAction::Tree(TreeAction::HalfPageUp))
        } else if self.tree.half_page_down.matches(key) {
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
        } else if self.tree.expand_all.matches(key) {
            Some(FilteredTreeAction::Tree(TreeAction::ExpandAll))
        } else if self.tree.go_to_end.matches(key) {
            Some(FilteredTreeAction::Tree(TreeAction::GoToEnd))
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
        if self.toggle_command_log.matches(key) || is_escape_key(key) {
            Action::CloseCommandLog
        } else if self.quit.matches(key) {
            Action::Quit
        } else {
            Action::None
        }
    }

    pub fn help_dialog_action_for(&self, key: KeyEvent) -> Action {
        if self.open_help.matches(key) || is_escape_key(key) {
            Action::CloseHelp
        } else {
            Action::None
        }
    }

    pub fn open_columns_label(&self) -> String {
        self.tree.open_columns.label()
    }

    pub fn open_help_label(&self) -> String {
        self.open_help.label()
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
            "{} search | {} columns | {} project | {} theme | {} help",
            self.tree.focus_filter.label(),
            self.tree.open_columns.label(),
            self.switch_project.label(),
            self.switch_theme.label(),
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

    pub fn theme_dropdown_action_for(&self, key: KeyEvent) -> DropdownAction {
        self.project_dropdown_action_for(key)
    }

    pub fn dropdown_action_for(&self, key: KeyEvent) -> JiraFilteredTreeAction {
        if is_escape_key(key) || is_ctrl_left_bracket(key) {
            JiraFilteredTreeAction::Dropdown(DropdownAction::Close)
        } else if key.code == KeyCode::Char('/') && key.modifiers.is_empty() {
            JiraFilteredTreeAction::Dropdown(DropdownAction::FocusFilter)
        } else if key.code == KeyCode::Char('u') && key.modifiers.contains(KeyModifiers::CONTROL) {
            JiraFilteredTreeAction::Dropdown(DropdownAction::HalfPageUp)
        } else if key.code == KeyCode::Char('d') && key.modifiers.contains(KeyModifiers::CONTROL) {
            JiraFilteredTreeAction::Dropdown(DropdownAction::HalfPageDown)
        } else if key.code == KeyCode::Char('G') && key.modifiers.contains(KeyModifiers::SHIFT) {
            JiraFilteredTreeAction::Dropdown(DropdownAction::GoToEnd)
        } else if key.code == KeyCode::Char('g') && key.modifiers.is_empty() {
            JiraFilteredTreeAction::Dropdown(DropdownAction::GotoPrefix)
        } else if key.code == KeyCode::Enter || key.code == KeyCode::Char(' ') {
            JiraFilteredTreeAction::Dropdown(DropdownAction::ToggleSelected)
        } else if key.code == KeyCode::Up || key.code == KeyCode::Char('k') {
            JiraFilteredTreeAction::Dropdown(DropdownAction::MoveUp)
        } else if key.code == KeyCode::Down || key.code == KeyCode::Char('j') {
            JiraFilteredTreeAction::Dropdown(DropdownAction::MoveDown)
        } else {
            JiraFilteredTreeAction::Dropdown(DropdownAction::None)
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
                KeyCode::Char('k') => return FilterAction::DeleteToEnd,
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

        if self.quit.matches(key) {
            FilterAction::Quit
        } else if is_escape_key(key)
            || key.code == KeyCode::Enter
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
        self == KeySpec::from(key)
    }

    pub fn label(self) -> String {
        let key = match self.code {
            KeyCode::Char(' ') => String::from("Space"),
            KeyCode::Char(c) => c.to_ascii_uppercase().to_string(),
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

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DropdownKind {
    JiraColumns,
    QuickSwitcher,
    ProjectSwitcher,
    ThemePicker,
    AssigneePicker,
    BoardGroup,
}

/// The single, mutually-exclusive app-level single-select overlay. The column
/// dropdown lives inside `JiraFilteredTreeState` (multi-select, distinct
/// geometry) and is intentionally NOT part of this enum.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum Overlay {
    Quick(MultiSelectDropdownState<QuickAction>),
    Project(MultiSelectDropdownState<ProjectSummary>),
    Theme(MultiSelectDropdownState<ThemeChoice>),
    Assignee(MultiSelectDropdownState<Option<UserSummary>>),
    BoardGroup(MultiSelectDropdownState<BoardGrouping>),
}

impl Overlay {
    pub(crate) fn kind(&self) -> DropdownKind {
        match self {
            Self::Quick(_) => DropdownKind::QuickSwitcher,
            Self::Project(_) => DropdownKind::ProjectSwitcher,
            Self::Theme(_) => DropdownKind::ThemePicker,
            Self::Assignee(_) => DropdownKind::AssigneePicker,
            Self::BoardGroup(_) => DropdownKind::BoardGroup,
        }
    }

    pub(crate) fn is_filter_focused(&self) -> bool {
        match self {
            Self::Quick(d) => d.is_filter_focused(),
            Self::Project(d) => d.is_filter_focused(),
            Self::Theme(d) => d.is_filter_focused(),
            Self::Assignee(d) => d.is_filter_focused(),
            Self::BoardGroup(d) => d.is_filter_focused(),
        }
    }

    pub(crate) fn is_animating(&self) -> bool {
        match self {
            Self::Quick(d) => d.is_animating(),
            Self::Project(d) => d.is_animating(),
            Self::Theme(d) => d.is_animating(),
            Self::Assignee(d) => d.is_animating(),
            Self::BoardGroup(d) => d.is_animating(),
        }
    }

    pub(crate) fn tick(&mut self, dt: std::time::Duration) {
        match self {
            Self::Quick(d) => d.tick(dt),
            Self::Project(d) => d.tick(dt),
            Self::Theme(d) => d.tick(dt),
            Self::Assignee(d) => d.tick(dt),
            Self::BoardGroup(d) => d.tick(dt),
        }
    }

    pub(crate) fn focus_filter(&mut self) {
        let action = crate::components::generic::dropdown::DropdownAction::FocusFilter;
        match self {
            Self::Quick(d) => {
                d.dispatch(action);
            }
            Self::Project(d) => {
                d.dispatch(action);
            }
            Self::Theme(d) => {
                d.dispatch(action);
            }
            Self::Assignee(d) => {
                d.dispatch(action);
            }
            Self::BoardGroup(d) => {
                d.dispatch(action);
            }
        }
    }

    pub(crate) fn scroll_viewport(&mut self, delta: isize) {
        match self {
            Self::Quick(d) => d.scroll_viewport(delta),
            Self::Project(d) => d.scroll_viewport(delta),
            Self::Theme(d) => d.scroll_viewport(delta),
            Self::Assignee(d) => d.scroll_viewport(delta),
            Self::BoardGroup(d) => d.scroll_viewport(delta),
        }
    }
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
    Shortcuts,
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
            Self::Shortcuts => "Shortcuts",
        }
        .to_owned()
    }
}

impl App {
    pub fn is_assignee_dropdown_open(&self) -> bool {
        matches!(self.overlay, Some(Overlay::Assignee(_)))
    }

    pub fn is_project_dropdown_open(&self) -> bool {
        matches!(self.overlay, Some(Overlay::Project(_)))
    }

    pub fn is_theme_dropdown_open(&self) -> bool {
        matches!(self.overlay, Some(Overlay::Theme(_)))
    }

    pub fn is_quick_switcher_open(&self) -> bool {
        matches!(self.overlay, Some(Overlay::Quick(_)))
    }

    pub fn is_board_group_dropdown_open(&self) -> bool {
        matches!(self.overlay, Some(Overlay::BoardGroup(_)))
    }

    pub fn is_any_dropdown_open(&self) -> bool {
        self.overlay.is_some() || self.is_column_dropdown_open()
    }

    pub(crate) fn toggle_dropdown(&mut self, dropdown: DropdownKind) {
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

    pub(crate) fn close_dropdown(&mut self, dropdown: DropdownKind) {
        match dropdown {
            DropdownKind::JiraColumns => self.filtered_tree.close_column_dropdown(),
            DropdownKind::ThemePicker => self.close_theme_dropdown_without_selection(),
            _ => {
                if self.overlay.as_ref().map(Overlay::kind) == Some(dropdown) {
                    self.overlay = None;
                }
            }
        }
    }

    fn is_dropdown_open(&self, dropdown: DropdownKind) -> bool {
        self.overlay.as_ref().map(Overlay::kind) == Some(dropdown)
            || (dropdown == DropdownKind::JiraColumns && self.is_column_dropdown_open())
    }

    pub(crate) fn close_dropdowns(&mut self) {
        if matches!(self.overlay, Some(Overlay::Theme(_)))
            && let Some(theme) = self.theme_preview_origin.take()
        {
            self.theme = theme;
        }
        self.overlay = None;
        self.filtered_tree.close_column_dropdown();
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
            QuickAction::Shortcuts,
        ];
        if self.active_tab() == ApplicationTab::List {
            actions.insert(3, QuickAction::ReloadList);
        } else if self.active_tab() == ApplicationTab::Board {
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
        self.overlay = Some(Overlay::Quick(
            MultiSelectDropdownState::new(options)
                .single_select()
                .with_filter_focused(),
        ));
    }

    pub(crate) fn open_project_dropdown(&mut self) {
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
        self.overlay = Some(Overlay::Project(
            MultiSelectDropdownState::new(options)
                .single_select()
                .focus_selected()
                .with_filter_focused(),
        ));
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
        self.overlay = Some(Overlay::Assignee(
            MultiSelectDropdownState::new(options)
                .single_select()
                .focus_selected()
                .with_filter_focused(),
        ));
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
        self.overlay = Some(Overlay::Theme(
            MultiSelectDropdownState::new(options)
                .single_select()
                .focus_selected()
                .with_filter_focused(),
        ));
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
        self.overlay = Some(Overlay::BoardGroup(
            MultiSelectDropdownState::new(options)
                .single_select()
                .focus_selected()
                .with_filter_focused(),
        ));
    }

    pub(crate) fn dispatch_board_group_dropdown(
        &mut self,
        action: crate::components::generic::dropdown::DropdownAction,
    ) {
        let Some(Overlay::BoardGroup(dropdown)) = &mut self.overlay else {
            return;
        };
        match dropdown.dispatch(action) {
            Some(DropdownEvent::Closed) => self.overlay = None,
            Some(DropdownEvent::Toggled(index)) => {
                let Some(grouping) = self
                    .board_group_dropdown()
                    .and_then(|dropdown| dropdown.options().get(index))
                    .map(|option| option.value)
                else {
                    return;
                };
                self.board_grouping = grouping;
                self.overlay = None;
                self.board.select_first(self.board_filter.value(), grouping);
            }
            None => {}
        }
    }

    fn close_theme_dropdown_without_selection(&mut self) {
        self.overlay = None;
        if let Some(theme) = self.theme_preview_origin.take() {
            self.theme = theme;
        }
    }

    fn preview_focused_theme(&mut self) {
        let Some(Overlay::Theme(dropdown)) = &self.overlay else {
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

    pub(crate) fn dispatch_theme_dropdown(
        &mut self,
        action: crate::components::generic::dropdown::DropdownAction,
    ) {
        let event = {
            let Some(Overlay::Theme(dropdown)) = &mut self.overlay else {
                return;
            };
            dropdown.dispatch(action)
        };
        match event {
            Some(DropdownEvent::Closed) => self.close_theme_dropdown_without_selection(),
            Some(DropdownEvent::Toggled(index)) => {
                let Some(choice) = self
                    .theme_dropdown()
                    .and_then(|dropdown| dropdown.options().get(index))
                    .map(|option| option.value)
                else {
                    return;
                };
                let base = self
                    .theme_preview_origin
                    .take()
                    .unwrap_or_else(|| self.theme.clone());
                self.overlay = None;
                self.set_theme(base.with_name(choice.name));
                self.pending_effects.push(AppEffect::SaveTheme(choice.name));
                self.status = format!("Theme switched to {}.", choice.name.label());
            }
            None => self.preview_focused_theme(),
        }
    }

    pub(crate) fn dispatch_quick_switcher(
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

        let Some(Overlay::Quick(dropdown)) = &mut self.overlay else {
            return;
        };
        match dropdown.dispatch(action) {
            Some(DropdownEvent::Closed) => self.overlay = None,
            Some(DropdownEvent::Toggled(index)) => self.commit_quick_switcher_index(index),
            None => {}
        }
    }

    fn commit_quick_switcher_selection(&mut self) {
        let Some(index) = self.quick_switcher().and_then(|dropdown| {
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

    pub(crate) fn commit_quick_switcher_index(&mut self, index: usize) {
        let Some(choice) = self
            .quick_switcher()
            .and_then(|dropdown| dropdown.options().get(index))
            .map(|option| option.value)
        else {
            return;
        };
        self.overlay = None;
        self.run_quick_action(choice);
    }

    fn run_quick_action(&mut self, action: QuickAction) {
        match action {
            QuickAction::CommandLog => self.open_dialog(DialogKind::CommandLog),
            QuickAction::ThemePicker => self.open_theme_dropdown(),
            QuickAction::ProjectPicker => self.open_project_dropdown(),
            QuickAction::ReloadList => self.reload_list(),
            QuickAction::ReloadBoard => self.reload_board(),
            QuickAction::Board => self.select_tab(ApplicationTab::Board),
            QuickAction::List => self.select_tab(ApplicationTab::List),
            QuickAction::Timeline => self.select_tab(ApplicationTab::Timeline),
            QuickAction::Shortcuts => self.open_dialog(DialogKind::Help),
        }
    }

    pub(crate) fn select_tab(&mut self, tab: ApplicationTab) {
        self.tabs.set_selected(tab.index());
        self.screen = Screen::Main;
        self.filtered_tree.clear_transient_input();
        self.close_overlays();
    }

    pub(crate) fn dispatch_assignee_dropdown(
        &mut self,
        action: crate::components::generic::dropdown::DropdownAction,
    ) {
        let Some(Overlay::Assignee(dropdown)) = &mut self.overlay else {
            return;
        };
        match dropdown.dispatch(action) {
            Some(DropdownEvent::Closed) => {
                self.overlay = None;
            }
            Some(DropdownEvent::Toggled(index)) => self.commit_assignee_index(index),
            None => {}
        }
    }

    fn commit_assignee_index(&mut self, index: usize) {
        let Some(assignee) = self
            .assignee_dropdown()
            .and_then(|dropdown| dropdown.options().get(index))
            .map(|option| option.value.clone())
        else {
            return;
        };
        self.overlay = None;
        self.queue_selected_assignment(assignee);
    }

    pub(crate) fn dispatch_project_dropdown(
        &mut self,
        action: crate::components::generic::dropdown::DropdownAction,
    ) {
        let Some(Overlay::Project(dropdown)) = &mut self.overlay else {
            return;
        };
        match dropdown.dispatch(action) {
            Some(DropdownEvent::Closed) => {
                self.overlay = None;
            }
            Some(DropdownEvent::Toggled(index)) => {
                let Some(project) = dropdown
                    .options()
                    .get(index)
                    .map(|option| option.value.clone())
                else {
                    return;
                };
                self.overlay = None;
                self.switch_project(project);
            }
            None => {}
        }
    }
}

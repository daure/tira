use super::*;

impl App {
    pub fn handle_key(&mut self, key: KeyEvent, keybindings: &KeyBindings) {
        let typing = self.overlay_captures_keys();

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

        if let Some(action) = keybindings.global_action_for(key)
            && self.should_dispatch_global(key, &action, keybindings, typing)
        {
            self.dispatch(self.contextual_global_action(action));
            return;
        }

        if let Some(Overlay::Quick(_)) = &self.overlay {
            self.dispatch(Action::QuickSwitcher(self.dropdown_key_action(
                key,
                keybindings,
                self.is_quick_switcher_filter_focused(),
                KeyBindings::project_dropdown_action_for,
            )));
            return;
        }

        if let Some(Overlay::Assignee(_)) = &self.overlay {
            self.dispatch(Action::AssigneeDropdown(self.dropdown_key_action(
                key,
                keybindings,
                self.is_assignee_dropdown_filter_focused(),
                KeyBindings::project_dropdown_action_for,
            )));
            return;
        }

        if let Some(Overlay::BoardGroup(_)) = &self.overlay {
            self.dispatch(Action::BoardGroupDropdown(
                self.dropdown_key_action(
                    key,
                    keybindings,
                    self.is_board_group_dropdown_filter_focused(),
                    KeyBindings::project_dropdown_action_for,
                ),
            ));
            return;
        }

        if let Some(Overlay::Theme(_)) = &self.overlay {
            self.dispatch(Action::ThemeDropdown(self.dropdown_key_action(
                key,
                keybindings,
                self.is_theme_dropdown_filter_focused(),
                KeyBindings::theme_dropdown_action_for,
            )));
            return;
        }

        if let Some(Overlay::Project(_)) = &self.overlay {
            self.dispatch(Action::ProjectDropdown(self.dropdown_key_action(
                key,
                keybindings,
                self.is_project_dropdown_filter_focused(),
                KeyBindings::project_dropdown_action_for,
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
            Screen::Main if self.filtered_tree.is_column_dropdown_open() => {
                let action = if self.filtered_tree.is_column_dropdown_filter_focused() {
                    JiraFilteredTreeAction::Dropdown(self.dropdown_key_action(
                        key,
                        keybindings,
                        true,
                        KeyBindings::project_dropdown_action_for,
                    ))
                } else if let Some(action) = keybindings.column_dropdown_context_action_for(key) {
                    action
                } else {
                    keybindings.dropdown_action_for(key)
                };
                self.dispatch(Action::JiraFilteredTree(action));
            }
            Screen::Main if self.active_tab() == ApplicationTab::Board && self.board_filter.is_focused() => {
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
                let action = if self.active_tab() == ApplicationTab::Board {
                    keybindings.board_action_for(key)
                } else {
                    keybindings.jira_filtered_tree_action_for(key)
                };
                if self.active_tab() != ApplicationTab::List
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

    /// Whether a key bound to a global action should dispatch it now, rather
    /// than yield to text input or an overlay. Returns false when a focused text
    /// input claims a printable/navigation key, when typing should swallow a
    /// bare quit, or when the board's grouping key overrides the global binding.
    fn should_dispatch_global(
        &self,
        key: KeyEvent,
        action: &Action,
        keybindings: &KeyBindings,
        typing: bool,
    ) -> bool {
        let focused_text_input = self.text_input_focused();
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
            && self.active_tab() == ApplicationTab::Board
            && !self.is_board_filter_focused()
            && keybindings.board_action_for(key) == Action::ToggleBoardGrouping;
        !board_grouping_override
            && !(focused_text_input
                && (printable_text || is_navigation_shortcut)
                && !reserved_input_action
                || typing && matches!(action, Action::Quit) && !is_ctrl_q)
    }

    fn handle_help_key(&mut self, key: KeyEvent, keybindings: &KeyBindings) {
        let item_count = keybindings
            .help_items(
                self.screen(),
                self.active_tab().title(),
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

    pub(crate) fn move_help_selection(&mut self, delta: isize, item_count: usize) {
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

    fn dispatch_board_filter(&mut self, action: FilterAction) {
        match action {
            FilterAction::Quit => self.running = false,
            _ => {
                self.board_filter.dispatch(action);
            }
        }
    }

    fn contextual_global_action(&self, action: Action) -> Action {
        if matches!(action, Action::ReloadList | Action::ReloadNode)
            && self.active_tab() == ApplicationTab::Board
        {
            Action::ReloadBoard
        } else {
            action
        }
    }
}

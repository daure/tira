use super::*;

impl App {
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect, keybindings: &KeyBindings) {
        if self.is_help_open() {
            let scroll_delta = match mouse.kind {
                MouseEventKind::ScrollUp => Some(-1),
                MouseEventKind::ScrollDown => Some(1),
                _ => None,
            };
            if let Some(delta) = scroll_delta {
                let items = keybindings.help_items_for_context(
                    self.screen(),
                    self.active_tab().title(),
                    self.help_context(),
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
            && self.active_tab() == ApplicationTab::List
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
        if self.active_tab() == ApplicationTab::Board {
            self.handle_board_mouse(point, inner, scroll_delta, horizontal_delta);
            return;
        }
        if self.active_tab() == ApplicationTab::Timeline {
            self.handle_timeline_mouse(point, inner, scroll_delta, horizontal_delta);
            return;
        }
        if self.active_tab() != ApplicationTab::List {
            return;
        }
        self.handle_list_mouse(
            point,
            inner,
            scroll_delta,
            horizontal_delta,
            is_left_click,
            keybindings,
        );
    }

    fn handle_board_mouse(
        &mut self,
        point: (u16, u16),
        inner: Rect,
        scroll_delta: Option<isize>,
        horizontal_delta: Option<isize>,
    ) {
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
    }

    /// Wheel-scrolls the timeline rows (and shift/horizontal wheel pans the date
    /// axis), mirroring the List tab. The viewport height drops the toolbar and
    /// the two header rows; `scroll_viewport` clamps to the row count.
    fn handle_timeline_mouse(
        &mut self,
        _point: (u16, u16),
        inner: Rect,
        scroll_delta: Option<isize>,
        horizontal_delta: Option<isize>,
    ) {
        let viewport_height = inner.height.saturating_sub(3) as usize;
        if let Some(delta) = scroll_delta {
            self.timeline.scroll_viewport(delta, viewport_height);
        }
        if let Some(delta) = horizontal_delta {
            self.timeline
                .scroll_h(delta as i32 * TIMELINE_WHEEL_SCROLL_STEP);
        }
    }

    fn handle_list_mouse(
        &mut self,
        point: (u16, u16),
        inner: Rect,
        scroll_delta: Option<isize>,
        horizontal_delta: Option<isize>,
        is_left_click: bool,
        keybindings: &KeyBindings,
    ) {
        let [filter_row, content_area] = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Min(1),
            ])
            .areas(inner);
        let trigger_area = self.column_trigger_area(inner, keybindings);
        if contains_point(trigger_area, point) {
            if crate::ui::layout::toolbar_is_collapsed(inner.width) {
                self.open_dialog(DialogKind::Help);
            } else {
                self.toggle_dropdown(DropdownKind::JiraColumns);
            }
            return;
        }
        // The filter row's two cells partition it fully, so a point inside the
        // row but outside the (already handled) trigger lies in the filter cell.
        if contains_point(filter_row, point) {
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
        if let Some(delta) = horizontal_delta {
            // Shift/horizontal wheel pans the table; only the Table view has
            // scrollable columns.
            if matches!(self.filtered_tree_view_mode(), FilteredTreeViewMode::Table) {
                self.scroll_table_horizontal(delta as i32);
            }
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
        if let Some(Overlay::Quick(_)) = &self.overlay {
            return self.scroll_centered_dropdown(point, area, DropdownKind::QuickSwitcher, delta);
        }
        if let Some(Overlay::Theme(_)) = &self.overlay {
            return self.scroll_centered_dropdown(point, area, DropdownKind::ThemePicker, delta);
        }
        if let Some(Overlay::Project(_)) = &self.overlay {
            return self.scroll_centered_dropdown(
                point,
                area,
                DropdownKind::ProjectSwitcher,
                delta,
            );
        }
        if let Some(Overlay::Assignee(_)) = &self.overlay {
            return self.scroll_centered_dropdown(point, area, DropdownKind::AssigneePicker, delta);
        }
        if self.filtered_tree.is_column_dropdown_open() {
            return self.scroll_column_dropdown(point, area, delta);
        }
        false
    }

    fn centered_dropdown_dimensions(&self, kind: DropdownKind, area: Rect) -> Option<(u16, u16)> {
        match kind {
            DropdownKind::QuickSwitcher => self
                .quick_switcher()
                .map(|dropdown| dropdown_dimensions(dropdown, area, 34, 10)),
            DropdownKind::ThemePicker => self
                .theme_dropdown()
                .map(|dropdown| dropdown_dimensions(dropdown, area, 34, 12)),
            DropdownKind::ProjectSwitcher => self
                .project_dropdown()
                .map(|dropdown| dropdown_dimensions(dropdown, area, 32, 10)),
            DropdownKind::AssigneePicker => self
                .assignee_dropdown()
                .map(|dropdown| dropdown_dimensions(dropdown, area, 32, 10)),
            DropdownKind::BoardGroup => self
                .board_group_dropdown()
                .map(|dropdown| dropdown_dimensions(dropdown, area, 28, 6)),
            DropdownKind::JiraColumns => None,
        }
    }

    fn scroll_centered_dropdown(
        &mut self,
        point: (u16, u16),
        area: Rect,
        kind: DropdownKind,
        delta: isize,
    ) -> bool {
        let (width, rows) = self
            .centered_dropdown_dimensions(kind, area)
            .unwrap_or((0, 0));
        if width == 0 || rows == 0 {
            return false;
        }

        let rect = centered_rect(area, width, rows + 3);
        if !contains_point(rect, point) {
            return true;
        }
        self.scroll_dropdown(delta);
        true
    }

    fn scroll_column_dropdown(&mut self, point: (u16, u16), area: Rect, delta: isize) -> bool {
        if self.column_dropdown().is_none() {
            return false;
        }
        let rect = self.column_dropdown_rect(area);
        if !contains_point(rect, point) {
            return true;
        }
        self.filtered_tree.scroll_column_dropdown(delta);
        true
    }

    fn column_dropdown_rect(&self, area: Rect) -> Rect {
        self.column_dropdown()
            .map(|dropdown| {
                crate::components::jira::issue_list::column_dropdown_rect(area, dropdown)
            })
            .unwrap_or(area)
    }

    fn is_column_trigger_point(
        &self,
        inner: Rect,
        point: (u16, u16),
        keybindings: &KeyBindings,
    ) -> bool {
        !crate::ui::layout::toolbar_is_collapsed(inner.width)
            && contains_point(self.column_trigger_area(inner, keybindings), point)
    }

    fn column_trigger_area(&self, inner: Rect, keybindings: &KeyBindings) -> Rect {
        let [filter_row, _content_area] = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Min(1),
            ])
            .areas(inner);
        let trigger_width = if crate::ui::layout::toolbar_is_collapsed(inner.width) {
            keybindings.shortcuts_hint_width()
        } else {
            keybindings.column_trigger_width()
        };
        let [_filter_area, trigger_area] = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Min(1),
                ratatui::layout::Constraint::Length(trigger_width),
            ])
            .areas(filter_row);
        trigger_area
    }

    fn handle_open_dropdown_mouse(&mut self, point: (u16, u16), area: Rect) -> bool {
        if let Some(Overlay::Quick(_)) = &self.overlay {
            return self.handle_centered_dropdown_mouse(point, area, DropdownKind::QuickSwitcher);
        }
        if let Some(Overlay::Theme(_)) = &self.overlay {
            return self.handle_centered_dropdown_mouse(point, area, DropdownKind::ThemePicker);
        }
        if let Some(Overlay::Project(_)) = &self.overlay {
            return self.handle_centered_dropdown_mouse(point, area, DropdownKind::ProjectSwitcher);
        }
        if let Some(Overlay::Assignee(_)) = &self.overlay {
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
        let (width, rows) = self
            .centered_dropdown_dimensions(kind, area)
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
            if let Some(overlay) = &mut self.overlay {
                overlay.focus_filter();
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

    fn scroll_dropdown(&mut self, delta: isize) {
        if let Some(overlay) = &mut self.overlay {
            overlay.scroll_viewport(delta);
        }
    }

    fn click_dropdown_option(&mut self, kind: DropdownKind, row: usize, height: usize) {
        match kind {
            DropdownKind::QuickSwitcher => {
                let Some(index) = dropdown_index_at(self.quick_switcher(), row, height) else {
                    return;
                };
                if let Some(Overlay::Quick(dropdown)) = &mut self.overlay {
                    dropdown.set_selected_index(index);
                }
                self.commit_quick_switcher_index(index);
            }
            DropdownKind::ThemePicker => {
                let Some(index) = dropdown_index_at(self.theme_dropdown(), row, height) else {
                    return;
                };
                if let Some(Overlay::Theme(dropdown)) = &mut self.overlay {
                    dropdown.set_selected_index(index);
                }
                self.dispatch_theme_dropdown(
                    crate::components::generic::dropdown::DropdownAction::ToggleSelected,
                );
            }
            DropdownKind::ProjectSwitcher => {
                let Some(index) = dropdown_index_at(self.project_dropdown(), row, height) else {
                    return;
                };
                if let Some(Overlay::Project(dropdown)) = &mut self.overlay {
                    dropdown.set_selected_index(index);
                }
                self.dispatch_project_dropdown(
                    crate::components::generic::dropdown::DropdownAction::ToggleSelected,
                );
            }
            DropdownKind::AssigneePicker => {
                let Some(index) = dropdown_index_at(self.assignee_dropdown(), row, height) else {
                    return;
                };
                if let Some(Overlay::Assignee(dropdown)) = &mut self.overlay {
                    dropdown.set_selected_index(index);
                }
                self.dispatch_assignee_dropdown(
                    crate::components::generic::dropdown::DropdownAction::ToggleSelected,
                );
            }
            DropdownKind::BoardGroup => {
                let Some(index) = dropdown_index_at(self.board_group_dropdown(), row, height)
                else {
                    return;
                };
                if let Some(Overlay::BoardGroup(dropdown)) = &mut self.overlay {
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
        if self.column_dropdown().is_none() {
            return false;
        }
        let rect = self.column_dropdown_rect(area);
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

use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use std::{cell::Cell, time::Duration};

use super::{
    filter::{FilterAction, FilterEvent, FilterState},
    scroll_animator::ScrollAnimator,
};

const HALF_PAGE_STEP: isize = 10;
const DEFAULT_VIEWPORT_HEIGHT: usize = HALF_PAGE_STEP as usize * 2 + 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropdownAction {
    Filter(FilterAction),
    FocusFilter,
    ClearFilter,
    MoveUp,
    MoveDown,
    HalfPageUp,
    HalfPageDown,
    GoToEnd,
    GotoPrefix,
    ToggleSelected,
    Close,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropdownEvent {
    Toggled(usize),
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropdownOption<T> {
    pub value: T,
    pub label: String,
    pub selected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropdownVisibleOption<'a, T> {
    Option {
        index: usize,
        option: &'a DropdownOption<T>,
    },
    Separator,
    NoResults,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DropdownSelectionMode {
    Multiple,
    Single,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiSelectDropdownState<T> {
    filter: FilterState,
    options: Vec<DropdownOption<T>>,
    selected_index: usize,
    scroll: usize,
    last_viewport_height: Cell<usize>,
    pending_goto_prefix: bool,
    scroll_animator: ScrollAnimator,
    at_least_one_selection_required: bool,
    selection_mode: DropdownSelectionMode,
}

impl<T> MultiSelectDropdownState<T> {
    pub fn new(options: Vec<DropdownOption<T>>) -> Self {
        Self {
            filter: FilterState::default(),
            options,
            selected_index: 0,
            scroll: 0,
            last_viewport_height: Cell::new(DEFAULT_VIEWPORT_HEIGHT),
            pending_goto_prefix: false,
            scroll_animator: ScrollAnimator::new(),
            at_least_one_selection_required: false,
            selection_mode: DropdownSelectionMode::Multiple,
        }
    }

    pub fn require_at_least_one_selection(mut self) -> Self {
        self.at_least_one_selection_required = true;
        self
    }

    pub fn is_option_toggle_enabled(&self, index: usize) -> bool {
        !self.at_least_one_selection_required
            || !self
                .options
                .get(index)
                .is_some_and(|option| option.selected)
            || self.options.iter().filter(|option| option.selected).count() > 1
    }

    pub fn is_single_select(&self) -> bool {
        self.selection_mode == DropdownSelectionMode::Single
    }

    pub fn single_select(mut self) -> Self {
        self.selection_mode = DropdownSelectionMode::Single;
        self
    }

    pub fn focus_selected(mut self) -> Self {
        if let Some(index) = self.options.iter().position(|option| option.selected) {
            self.selected_index = index;
        }
        self
    }

    pub fn focus_filter(mut self) -> Self {
        self.filter.focus();
        self
    }

    pub fn tick(&mut self, dt: Duration) {
        self.scroll_animator.tick(dt);
    }

    pub fn is_animating(&self) -> bool {
        self.scroll_animator.is_animating()
    }

    pub fn filter_state(&self) -> &FilterState {
        &self.filter
    }

    pub fn filter(&self) -> &str {
        self.filter.value()
    }

    pub fn filter_cursor(&self) -> usize {
        self.filter.cursor()
    }

    pub fn options(&self) -> &[DropdownOption<T>] {
        &self.options
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn is_filter_focused(&self) -> bool {
        self.filter.is_focused()
    }

    pub fn visible_options(&self) -> Vec<(usize, &DropdownOption<T>)> {
        let filter = self.filter.value();
        if self.selection_mode == DropdownSelectionMode::Single {
            return self
                .options
                .iter()
                .enumerate()
                .filter(|(_, option)| option_matches_filter(option.label.as_str(), filter))
                .collect();
        }

        let mut selected = Vec::new();
        let mut unselected = Vec::new();
        for (index, option) in self.options.iter().enumerate() {
            if option_matches_filter(option.label.as_str(), filter) {
                if option.selected {
                    selected.push((index, option));
                } else {
                    unselected.push((index, option));
                }
            }
        }
        selected.extend(unselected);
        selected
    }

    pub fn visible_row_count(&self) -> usize {
        let rows = self.visible_options();
        if rows.is_empty() {
            1
        } else {
            rows.len() + usize::from(self.has_separator(&rows))
        }
    }

    pub fn visible_range(&self, height: usize) -> std::ops::Range<usize> {
        let row_count = self.visible_row_count();
        if row_count == 0 || height == 0 {
            return 0..0;
        }
        let viewport = height.min(row_count);
        self.last_viewport_height.set(viewport);
        let max_scroll = row_count.saturating_sub(viewport);
        self.scroll_animator
            .set_target(self.scroll.min(max_scroll) as f64);
        let start = (self.scroll_animator.current().round() as usize).min(max_scroll);
        start..(start + viewport).min(row_count)
    }

    pub fn visible_window(&self, height: usize) -> Vec<DropdownVisibleOption<'_, T>> {
        let rows = self.visible_options();
        if rows.is_empty() {
            return (height > 0)
                .then_some(DropdownVisibleOption::NoResults)
                .into_iter()
                .collect();
        }
        let selected_count = rows.iter().filter(|(_, option)| option.selected).count();
        let has_separator = self.has_separator(&rows);
        let mut window = Vec::new();

        for position in self.visible_range(height) {
            if has_separator && position == selected_count {
                window.push(DropdownVisibleOption::Separator);
                continue;
            }
            let row_position = position - usize::from(has_separator && position > selected_count);
            if let Some((index, option)) = rows.get(row_position) {
                window.push(DropdownVisibleOption::Option {
                    index: *index,
                    option,
                });
            }
        }

        window
    }

    pub fn dispatch(&mut self, action: DropdownAction) -> Option<DropdownEvent> {
        match action {
            DropdownAction::Filter(action) => {
                self.pending_goto_prefix = false;
                if matches!(
                    action,
                    FilterAction::Exit | FilterAction::Submit | FilterAction::Quit
                ) {
                    self.filter.clear_focus();
                    return None;
                }
                let event = self.filter.dispatch(action);
                if matches!(event, Some(FilterEvent::Changed)) {
                    self.select_first_visible_option();
                }
                None
            }
            DropdownAction::FocusFilter => {
                self.pending_goto_prefix = false;
                self.filter.focus();
                None
            }
            DropdownAction::ClearFilter => {
                self.pending_goto_prefix = false;
                self.filter.clear();
                None
            }
            DropdownAction::MoveUp => {
                if self.filter.is_focused() {
                    return None;
                }
                self.pending_goto_prefix = false;
                self.move_selection(-1);
                None
            }
            DropdownAction::MoveDown => {
                if self.filter.is_focused() {
                    return None;
                }
                self.pending_goto_prefix = false;
                self.move_selection(1);
                None
            }
            DropdownAction::HalfPageUp => {
                if self.filter.is_focused() {
                    return None;
                }
                self.pending_goto_prefix = false;
                self.move_selection(-HALF_PAGE_STEP);
                None
            }
            DropdownAction::HalfPageDown => {
                if self.filter.is_focused() {
                    return None;
                }
                self.pending_goto_prefix = false;
                self.move_selection(HALF_PAGE_STEP);
                None
            }
            DropdownAction::GoToEnd => {
                if self.filter.is_focused() {
                    return None;
                }
                self.go_to_end();
                None
            }
            DropdownAction::GotoPrefix => {
                if self.filter.is_focused() {
                    return None;
                }
                self.handle_goto_prefix();
                None
            }
            DropdownAction::ToggleSelected => {
                if self.filter.is_focused() {
                    return None;
                }
                self.pending_goto_prefix = false;
                let toggled_index = self.selected_index;
                if !self
                    .visible_options()
                    .iter()
                    .any(|(index, _)| *index == toggled_index)
                {
                    return None;
                }
                if self.selection_mode == DropdownSelectionMode::Single {
                    for option in &mut self.options {
                        option.selected = false;
                    }
                    self.options.get_mut(toggled_index)?.selected = true;
                    return Some(DropdownEvent::Toggled(toggled_index));
                }

                if !self.is_option_toggle_enabled(toggled_index) {
                    return None;
                }
                let current_position = self.selected_option_position();
                let was_selected = self.options.get(toggled_index)?.selected;
                let selected_count = self.selected_option_count();
                let has_separator =
                    selected_count > 0 && selected_count < self.visible_options().len();
                self.options.get_mut(toggled_index)?.selected = !was_selected;
                self.keep_selection_after_toggle(
                    current_position,
                    was_selected,
                    selected_count,
                    has_separator,
                );
                Some(DropdownEvent::Toggled(toggled_index))
            }
            DropdownAction::Close => {
                self.pending_goto_prefix = false;
                if self.filter.is_focused() || !self.filter.value().is_empty() {
                    self.filter.clear();
                    None
                } else {
                    Some(DropdownEvent::Closed)
                }
            }
            DropdownAction::None => {
                self.pending_goto_prefix = false;
                None
            }
        }
    }

    fn go_to_end(&mut self) {
        self.pending_goto_prefix = false;
        let rows = self.visible_options();
        if let Some((index, _)) = rows.last() {
            self.selected_index = *index;
            self.scroll_to_selection();
        } else {
            self.selected_index = 0;
            self.scroll = 0;
        }
    }

    fn go_to_start(&mut self) {
        self.pending_goto_prefix = false;
        let rows = self.visible_options();
        if let Some((index, _)) = rows.first() {
            self.selected_index = *index;
        } else {
            self.selected_index = 0;
        }
        self.scroll = 0;
    }

    fn handle_goto_prefix(&mut self) {
        if self.pending_goto_prefix {
            self.go_to_start();
        } else {
            self.pending_goto_prefix = true;
        }
    }

    fn move_selection(&mut self, delta: isize) {
        let rows = self.visible_options();
        if rows.is_empty() {
            self.selected_index = 0;
            self.scroll = 0;
            return;
        }
        let current_position = rows
            .iter()
            .position(|(index, _)| *index == self.selected_index)
            .unwrap_or(0);
        let next_position = current_position
            .saturating_add_signed(delta)
            .min(rows.len() - 1);
        self.selected_index = rows[next_position].0;
        self.scroll_to_selection();
    }

    fn scroll_to_selection(&mut self) {
        let rows = self.visible_options();
        let row_count = rows.len() + usize::from(self.has_separator(&rows));
        if row_count == 0 {
            self.scroll = 0;
            return;
        }

        let selected_count = rows.iter().filter(|(_, option)| option.selected).count();
        let has_separator = self.has_separator(&rows);
        let selected_position = rows
            .iter()
            .position(|(index, _)| *index == self.selected_index)
            .unwrap_or(0);
        let selected_position =
            selected_position + usize::from(has_separator && selected_position >= selected_count);
        let viewport = self.last_viewport_height.get().max(1).min(row_count);
        let middle = viewport / 2;
        let max_scroll = row_count.saturating_sub(viewport);
        self.scroll = selected_position.saturating_sub(middle).min(max_scroll);
    }

    fn selected_option_position(&self) -> usize {
        self.visible_options()
            .iter()
            .position(|(index, _)| *index == self.selected_index)
            .unwrap_or(0)
    }

    fn selected_option_count(&self) -> usize {
        self.visible_options()
            .iter()
            .filter(|(_, option)| option.selected)
            .count()
    }

    fn keep_selection_after_toggle(
        &mut self,
        current_position: usize,
        was_selected: bool,
        selected_count: usize,
        has_separator: bool,
    ) {
        let rows = self.visible_options();
        if rows.is_empty() {
            self.selected_index = 0;
            self.scroll = 0;
            return;
        }

        let is_before_separator =
            has_separator && was_selected && current_position + 1 == selected_count;
        let is_after_separator =
            has_separator && !was_selected && current_position == selected_count;
        let next_position = if is_before_separator {
            current_position.saturating_sub(1)
        } else if is_after_separator {
            (current_position + 1).min(rows.len() - 1)
        } else {
            current_position.min(rows.len() - 1)
        };
        self.selected_index = rows[next_position].0;
        self.scroll_to_selection();
    }

    fn select_first_visible_option(&mut self) {
        let rows = self.visible_options();
        if rows.is_empty() {
            self.selected_index = 0;
            self.scroll = 0;
            self.scroll_animator.snap_to(0.0);
        } else {
            self.selected_index = rows[0].0;
            self.scroll = 0;
            self.scroll_animator.snap_to(0.0);
        }
    }

    fn has_separator(&self, rows: &[(usize, &DropdownOption<T>)]) -> bool {
        let selected_count = rows.iter().filter(|(_, option)| option.selected).count();
        self.selection_mode == DropdownSelectionMode::Multiple
            && selected_count > 0
            && selected_count < rows.len()
    }
}

fn option_matches_filter(label: &str, filter: &str) -> bool {
    filter.is_empty()
        || SkimMatcherV2::default()
            .smart_case()
            .fuzzy_match(label, filter)
            .is_some()
}
#[cfg(test)]
mod tests {
    use super::*;

    fn option(label: &str, selected: bool) -> DropdownOption<()> {
        DropdownOption {
            value: (),
            label: label.to_owned(),
            selected,
        }
    }

    #[test]
    fn close_clears_focused_filter_before_closing() {
        let mut dropdown = MultiSelectDropdownState::new(vec![option("Work", true)]);

        dropdown.dispatch(DropdownAction::FocusFilter);
        dropdown.dispatch(DropdownAction::Filter(FilterAction::Text('w')));

        assert_eq!(dropdown.dispatch(DropdownAction::Close), None);
        assert_eq!(dropdown.filter(), "");
        assert!(!dropdown.is_filter_focused());
        assert_eq!(
            dropdown.dispatch(DropdownAction::Close),
            Some(DropdownEvent::Closed)
        );
    }

    #[test]
    fn at_least_one_selection_required_blocks_last_selected_toggle() {
        let mut dropdown =
            MultiSelectDropdownState::new(vec![option("Work", true), option("Summary", false)])
                .require_at_least_one_selection();

        assert!(!dropdown.is_option_toggle_enabled(0));
        assert_eq!(dropdown.dispatch(DropdownAction::ToggleSelected), None);
        assert!(dropdown.options()[0].selected);
    }

    #[test]
    fn at_least_one_selection_required_allows_toggle_when_more_selected() {
        let mut dropdown =
            MultiSelectDropdownState::new(vec![option("Work", true), option("Summary", true)])
                .require_at_least_one_selection();

        assert!(dropdown.is_option_toggle_enabled(0));
        assert_eq!(
            dropdown.dispatch(DropdownAction::ToggleSelected),
            Some(DropdownEvent::Toggled(0))
        );
        assert!(!dropdown.options()[0].selected);
    }

    #[test]
    fn movement_skips_filtered_out_options() {
        let mut dropdown = MultiSelectDropdownState::new(vec![
            option("Work", true),
            option("Summary", true),
            option("Work type", false),
        ]);

        dropdown.dispatch(DropdownAction::FocusFilter);
        dropdown.dispatch(DropdownAction::Filter(FilterAction::Text('w')));
        dropdown.dispatch(DropdownAction::Filter(FilterAction::Exit));
        dropdown.dispatch(DropdownAction::MoveDown);

        assert_eq!(dropdown.selected_index(), 2);
    }

    #[test]
    fn filter_change_selects_first_visible_option() {
        let mut dropdown = MultiSelectDropdownState::new(vec![
            option("Work", true),
            option("Summary", true),
            option("Assignee", false),
        ]);

        dropdown.dispatch(DropdownAction::MoveDown);
        dropdown.dispatch(DropdownAction::FocusFilter);
        dropdown.dispatch(DropdownAction::Filter(FilterAction::Text('a')));
        dropdown.dispatch(DropdownAction::Filter(FilterAction::Text('s')));
        dropdown.dispatch(DropdownAction::Filter(FilterAction::Text('s')));
        assert_eq!(dropdown.selected_index(), 2);
    }

    #[test]
    fn empty_filter_results_render_unselectable_message() {
        let mut dropdown = MultiSelectDropdownState::new(vec![option("Work", true)]);

        dropdown.dispatch(DropdownAction::FocusFilter);
        dropdown.dispatch(DropdownAction::Filter(FilterAction::Text('z')));

        assert_eq!(
            dropdown.visible_window(5),
            vec![DropdownVisibleOption::NoResults]
        );
        assert_eq!(dropdown.dispatch(DropdownAction::ToggleSelected), None);
    }

    #[test]
    fn g_g_and_uppercase_g_jump_to_start_and_end() {
        let mut dropdown = MultiSelectDropdownState::new(
            (0..4)
                .map(|index| option(&format!("Field {index}"), false))
                .collect(),
        );

        dropdown.dispatch(DropdownAction::GoToEnd);
        assert_eq!(dropdown.selected_index(), 3);

        dropdown.dispatch(DropdownAction::GotoPrefix);
        assert_eq!(dropdown.selected_index(), 3);

        dropdown.dispatch(DropdownAction::GotoPrefix);
        assert_eq!(dropdown.selected_index(), 0);
    }

    #[test]
    fn g_g_scrolls_toward_start_instead_of_snapping() {
        let mut dropdown = MultiSelectDropdownState::new(
            (0..30)
                .map(|index| option(&format!("Field {index}"), false))
                .collect(),
        );

        assert_eq!(dropdown.visible_range(5), 0..5);
        dropdown.dispatch(DropdownAction::GoToEnd);
        assert_eq!(dropdown.visible_range(5), 0..5);
        dropdown.tick(Duration::from_secs(2));
        assert_eq!(dropdown.visible_range(5), 25..30);

        dropdown.dispatch(DropdownAction::GotoPrefix);
        dropdown.dispatch(DropdownAction::GotoPrefix);

        assert_eq!(dropdown.selected_index(), 0);
        assert_eq!(dropdown.visible_range(5), 25..30);
        dropdown.tick(Duration::from_secs(2));
        assert_eq!(dropdown.visible_range(5), 0..5);
    }

    #[test]
    fn deselect_keeps_selection_on_same_visible_line() {
        let mut dropdown = MultiSelectDropdownState::new(vec![
            option("Work", true),
            option("Summary", true),
            option("Work type", true),
            option("Status", false),
        ]);

        dropdown.dispatch(DropdownAction::MoveDown);
        assert_eq!(dropdown.selected_index(), 1);

        dropdown.dispatch(DropdownAction::ToggleSelected);

        assert_eq!(dropdown.selected_index(), 2);
        assert!(!dropdown.options()[1].selected);
    }

    #[test]
    fn deselect_last_selected_item_moves_up_before_separator() {
        let mut dropdown = MultiSelectDropdownState::new(vec![
            option("Work", true),
            option("Summary", true),
            option("Work type", true),
            option("Status", false),
        ]);

        dropdown.dispatch(DropdownAction::MoveDown);
        dropdown.dispatch(DropdownAction::MoveDown);
        assert_eq!(dropdown.selected_index(), 2);

        dropdown.dispatch(DropdownAction::ToggleSelected);

        assert_eq!(dropdown.selected_index(), 1);
        assert!(!dropdown.options()[2].selected);
    }

    #[test]
    fn single_select_marks_only_selected_option_without_separator() {
        let mut dropdown = MultiSelectDropdownState::new(vec![
            option("KAN", true),
            option("OPS", false),
            option("WEB", false),
        ])
        .single_select();

        dropdown.dispatch(DropdownAction::MoveDown);
        assert_eq!(
            dropdown.dispatch(DropdownAction::ToggleSelected),
            Some(DropdownEvent::Toggled(1))
        );

        assert!(!dropdown.options()[0].selected);
        assert!(dropdown.options()[1].selected);
        assert!(!dropdown.options()[2].selected);
        assert!(
            !dropdown
                .visible_window(10)
                .iter()
                .any(|entry| matches!(entry, DropdownVisibleOption::Separator))
        );
    }

    #[test]
    fn select_first_unselected_item_moves_down_after_separator() {
        let mut dropdown = MultiSelectDropdownState::new(vec![
            option("Work", true),
            option("Summary", true),
            option("Work type", false),
            option("Status", false),
        ]);

        dropdown.dispatch(DropdownAction::MoveDown);
        dropdown.dispatch(DropdownAction::MoveDown);
        assert_eq!(dropdown.selected_index(), 2);

        dropdown.dispatch(DropdownAction::ToggleSelected);

        assert_eq!(dropdown.selected_index(), 3);
        assert!(dropdown.options()[2].selected);
    }

    #[test]
    fn single_select_emits_toggle_for_already_selected_option() {
        let mut dropdown =
            MultiSelectDropdownState::new(vec![option("KAN", true), option("OPS", false)])
                .single_select();

        assert_eq!(
            dropdown.dispatch(DropdownAction::ToggleSelected),
            Some(DropdownEvent::Toggled(0))
        );
        assert!(dropdown.options()[0].selected);
        assert!(!dropdown.options()[1].selected);
    }

    #[test]
    fn selected_options_render_before_separator_and_unselected_options() {
        let dropdown = MultiSelectDropdownState::new(vec![
            option("Work", true),
            option("Project", false),
            option("Summary", true),
        ]);
        let visible = dropdown.visible_window(10);

        assert!(matches!(
            visible[0],
            DropdownVisibleOption::Option { index: 0, .. }
        ));
        assert!(matches!(
            visible[1],
            DropdownVisibleOption::Option { index: 2, .. }
        ));
        assert!(matches!(visible[2], DropdownVisibleOption::Separator));
        assert!(matches!(
            visible[3],
            DropdownVisibleOption::Option { index: 1, .. }
        ));
    }

    #[test]
    fn single_step_navigation_keeps_scroll_until_selection_reaches_middle() {
        let mut dropdown = MultiSelectDropdownState::new(
            (0..20)
                .map(|index| option(&format!("Field {index}"), false))
                .collect(),
        );

        assert_eq!(dropdown.visible_range(6), 0..6);
        dropdown.dispatch(DropdownAction::MoveDown);
        dropdown.dispatch(DropdownAction::MoveDown);
        dropdown.dispatch(DropdownAction::MoveDown);

        assert_eq!(dropdown.selected_index(), 3);
        assert_eq!(dropdown.visible_range(6), 0..6);

        dropdown.dispatch(DropdownAction::MoveDown);
        assert_eq!(dropdown.visible_range(6), 0..6);
        dropdown.tick(Duration::from_secs(2));

        assert_eq!(dropdown.selected_index(), 4);
        assert_eq!(dropdown.visible_range(6), 1..7);
    }

    #[test]
    fn half_page_navigation_scrolls_selected_item_into_view() {
        let mut dropdown = MultiSelectDropdownState::new(
            (0..20)
                .map(|index| option(&format!("Field {index}"), false))
                .collect(),
        );

        assert_eq!(dropdown.visible_range(5), 0..5);
        dropdown.dispatch(DropdownAction::HalfPageDown);

        assert_eq!(dropdown.selected_index(), 10);
        assert_eq!(dropdown.visible_range(5), 0..5);
        dropdown.tick(Duration::from_secs(2));
        assert_eq!(dropdown.visible_range(5), 8..13);
    }
}

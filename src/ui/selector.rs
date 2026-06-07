use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Clear, List, ListItem},
};

use crate::{
    components::generic::{
        dropdown::{DropdownVisibleOption, MultiSelectDropdownState},
        filter,
    },
    ui::theme::Theme,
};

use super::{project_switcher, style};

pub trait HasShortcut {
    fn shortcut(&self, keybindings: &crate::KeyBindings) -> Option<String>;
}

pub fn render_single_select<T>(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &'static str,
    dropdown: &MultiSelectDropdownState<T>,
    theme: &Theme,
    keybindings: &crate::KeyBindings,
    min_width: u16,
    max_visible_rows: u16,
    show_cursor: bool,
) where
    T: HasShortcut,
 {
    let longest_option = dropdown
        .options()
        .iter()
        .map(|option| option.label.chars().count())
        .max()
        .unwrap_or(0) as u16;
    let width = area.width.min((longest_option + 6).max(min_width));
    let visible_rows = dropdown.visible_row_count().min(max_visible_rows as usize) as u16;
    let height = area.height.min((visible_rows + 3).max(5));
    if width < 20 || height < 5 {
        return;
    }

    let dropdown_area = project_switcher::centered_rect(area, width, height);
    let block = project_switcher::dropdown_block(title, theme);
    let inner = block.inner(dropdown_area);

    frame.render_widget(Clear, dropdown_area);
    frame.render_widget(block, dropdown_area);

    let [_, padded_inner, _] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(inner);
    let [filter_area, options_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .areas(padded_inner);
    let [filter_icon_area, filter_text_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .areas(filter_area);

    frame.render_widget(
        filter::render_icon(dropdown.filter_state(), theme),
        filter_icon_area,
    );
    frame.render_widget(
        filter::render_text(dropdown.filter_state(), theme),
        filter_text_area,
    );

    let options = dropdown
        .visible_window(options_area.height as usize)
        .into_iter()
        .filter_map(|entry| match entry {
            DropdownVisibleOption::Separator => None,
            DropdownVisibleOption::NoResults => Some(project_switcher::no_results_item(theme)),
            DropdownVisibleOption::Option { index, option } => {
                let is_focused = index == dropdown.selected_index();
                let row_style = style::selected_row_style(theme, is_focused);
                let label_style =
                    style::dropdown_option_label_style(theme, option.selected, is_focused);
                let mut spans = style::single_select_dropdown_spans(
                    theme,
                    option.label.as_str(),
                    dropdown.filter(),
                    option.selected,
                    is_focused,
                    options_area.width as usize,
                    label_style,
                );
                if let Some(shortcut) = option.value.shortcut(keybindings) {
                    let total_width = options_area.width as usize;
                    let label_width = option.label.chars().count() + 2;
                    let shortcut_width = shortcut.chars().count();
                    let gap = total_width.saturating_sub(label_width + shortcut_width);
                    if gap > 0 {
                        spans.push(Span::styled(" ".repeat(gap), row_style));
                        spans.push(Span::styled(shortcut, row_style.fg(theme.muted_fg())));
                    }
                }
                Some(ListItem::new(Line::from(spans)).style(row_style))
            }
        });
    frame.render_widget(List::new(options), options_area);

    if show_cursor && dropdown.is_filter_focused() {
        let cursor_x = filter_text_area.x + dropdown.filter_cursor() as u16;
        frame.set_cursor_position(ratatui::layout::Position::new(cursor_x, filter_text_area.y));
    }
}

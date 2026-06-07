use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::Line,
    widgets::{Clear, List, ListItem},
};

use crate::{
    App,
    components::generic::{dropdown::DropdownVisibleOption, filter},
};

use super::{project_switcher, style};

pub fn render(frame: &mut Frame<'_>, status_area: Rect, app: &App) {
    let Some(dropdown) = app.theme_dropdown() else {
        return;
    };
    let longest_option = dropdown
        .options()
        .iter()
        .map(|option| option.label.chars().count())
        .max()
        .unwrap_or(0) as u16;
    let width = status_area.width.min((longest_option + 6).max(28));
    let visible_rows = dropdown.visible_row_count().min(6) as u16;
    let height = (visible_rows + 3).max(5);
    if width < 20 || status_area.y < height {
        return;
    }

    let dropdown_area = Rect {
        x: status_area.x + status_area.width.saturating_sub(width + 1),
        y: status_area.y.saturating_sub(height),
        width,
        height,
    };
    let block = project_switcher::dropdown_block("Theme", app.theme());
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
        filter::render_icon(dropdown.filter_state(), app.theme()),
        filter_icon_area,
    );
    frame.render_widget(
        filter::render_text(dropdown.filter_state(), app.theme()),
        filter_text_area,
    );

    let options = dropdown
        .visible_window(options_area.height as usize)
        .into_iter()
        .filter_map(|entry| match entry {
            DropdownVisibleOption::Separator => None,
            DropdownVisibleOption::NoResults => {
                Some(project_switcher::no_results_item(app.theme()))
            }
            DropdownVisibleOption::Option { index, option } => {
                let is_focused = index == dropdown.selected_index();
                let row_style = style::selected_row_style(app.theme(), is_focused);
                let label_style =
                    style::dropdown_option_label_style(app.theme(), option.selected, is_focused);
                let spans = style::single_select_dropdown_spans(
                    app.theme(),
                    option.label.as_str(),
                    dropdown.filter(),
                    option.selected,
                    is_focused,
                    options_area.width as usize,
                    label_style,
                );
                Some(ListItem::new(Line::from(spans)).style(row_style))
            }
        });
    frame.render_widget(List::new(options), options_area);

    if dropdown.is_filter_focused() {
        let cursor_x = filter_text_area.x + dropdown.filter_cursor() as u16;
        frame.set_cursor_position(ratatui::layout::Position::new(cursor_x, filter_text_area.y));
    }
}

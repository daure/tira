use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table},
};

use crate::{
    App, JiraIssueColumn, TreeRow,
    components::generic::{
        dropdown::DropdownVisibleOption, filter, filtered_tree::FilteredTreeViewMode,
    },
    ui::{layout, scrollbar, style},
};

const NERD_COLLAPSED_ICON: &str = "";
const NERD_EXPANDED_ICON: &str = "";

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let [filter_area, content_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .areas(area);
    let [content_main, scrollbar_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .areas(content_area);

    render_filter(frame, filter_area, app);

    match app.filtered_tree_view_mode() {
        FilteredTreeViewMode::List => render_filtered_tree_list(frame, content_main, app),
        FilteredTreeViewMode::Table => render_filtered_tree_table(frame, content_main, app),
    }

    let scrollbar_viewport_height = match app.filtered_tree_view_mode() {
        FilteredTreeViewMode::List => content_main.height,
        FilteredTreeViewMode::Table => content_main.height.saturating_sub(1),
    };
    let scrollbar_render_area = match app.filtered_tree_view_mode() {
        FilteredTreeViewMode::List => scrollbar_area,
        FilteredTreeViewMode::Table => Rect {
            x: scrollbar_area.x,
            y: scrollbar_area.y.saturating_add(1),
            width: scrollbar_area.width,
            height: scrollbar_area.height.saturating_sub(1),
        },
    };
    scrollbar::render(frame, scrollbar_render_area, scrollbar_viewport_height, app);
    render_column_dropdown(frame, area, app);
}

fn render_filtered_tree_list(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let rows = app.visible_issue_rows();
    if rows.is_empty() {
        render_empty_list(frame, area);
        return;
    }
    let visible_range = app.visible_issue_range(area.height as usize);
    let items = visible_range.clone().map(|row_index| {
        let row = &rows[row_index];
        let item = &app.issues()[row.item_index];
        let row_style = style::selected_row_style(row_index == app.selected_issue_index());
        let mut spans = tree_control_spans(row);
        spans.push(Span::raw(" "));
        spans.extend(style::code_cell_spans(item, app.filter(), row_style));
        ListItem::new(Line::from(spans))
    });

    frame.render_widget(List::new(items), area);
}

fn render_filtered_tree_table(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let rows = app.visible_issue_rows();
    if rows.is_empty() {
        render_empty_list(frame, area);
        return;
    }

    let has_expandable = rows.iter().any(|row| row.expandable);
    let columns = app.visible_issue_columns();
    let code_header = "Work";
    let tree_prefix_width = if has_expandable {
        rows.iter().map(|row| row.depth * 2 + 2).max().unwrap_or(2) as u16
    } else {
        0
    };
    let first_column = columns.first();
    let code_width = layout::max_column_width(&rows, code_header, |row| {
        let prefix = usize::from(matches!(first_column, Some(JiraIssueColumn::IssueKey)))
            * tree_prefix_width as usize;
        prefix + 2 + app.issues()[row.item_index].id.chars().count()
    });
    let type_width = layout::max_column_width(&rows, "Work type", |row| {
        let prefix = usize::from(matches!(first_column, Some(JiraIssueColumn::IssueType)))
            * tree_prefix_width as usize;
        prefix + app.issues()[row.item_index].kind.chars().count()
    });
    let status_width = layout::max_column_width(&rows, "Status", |row| {
        let prefix = usize::from(matches!(first_column, Some(JiraIssueColumn::Status)))
            * tree_prefix_width as usize;
        prefix + app.issues()[row.item_index].status.chars().count()
    });

    let column_widths = columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let prefix = usize::from(index == 0) * tree_prefix_width as usize;
            match column {
                JiraIssueColumn::IssueKey => code_width,
                JiraIssueColumn::IssueType => type_width,
                JiraIssueColumn::Status => status_width,
                JiraIssueColumn::Field { id, label } => {
                    layout::max_column_width(&rows, label.as_str(), |row| {
                        prefix
                            + app.issues()[row.item_index]
                                .field_values
                                .get(id)
                                .map_or(0, |value| value.chars().count())
                    })
                }
                JiraIssueColumn::Summary => 0,
            }
        })
        .collect::<Vec<_>>();
    let fixed_width = columns
        .iter()
        .zip(column_widths.iter())
        .filter_map(|(column, width)| {
            (!matches!(column, JiraIssueColumn::Summary)).then_some(*width)
        })
        .sum::<u16>();
    let spacing = columns.len().saturating_sub(1) as u16;
    let description_width = area.width.saturating_sub(fixed_width + spacing) as usize;

    let table_rows = app
        .visible_issue_range(area.height.saturating_sub(1) as usize)
        .map(|row_index| {
            let row = &rows[row_index];
            let item = &app.issues()[row.item_index];
            let row_style = style::selected_row_style(row_index == app.selected_issue_index());
            let cells = columns
                .iter()
                .enumerate()
                .map(|(column_index, column)| {
                    let is_first = column_index == 0;
                    let spans = match column {
                        JiraIssueColumn::IssueKey => {
                            style::code_cell_spans(item, app.filter(), row_style)
                        }
                        JiraIssueColumn::Summary => {
                            let truncated =
                                layout::truncate_with_ellipsis(&item.label, description_width);
                            style::highlighted_spans_owned(&truncated, app.filter(), row_style)
                        }
                        JiraIssueColumn::IssueType => {
                            style::highlighted_spans(&item.kind, app.filter(), row_style)
                        }
                        JiraIssueColumn::Status => {
                            style::highlighted_spans(&item.status, app.filter(), row_style)
                        }
                        JiraIssueColumn::Field { id, .. } => {
                            item.field_values.get(id).map_or_else(Vec::new, |value| {
                                style::highlighted_spans(value, app.filter(), row_style)
                            })
                        }
                    };
                    Cell::from(Line::from(with_tree_prefix(
                        spans,
                        row,
                        is_first && has_expandable,
                    )))
                })
                .collect::<Vec<_>>();
            Row::new(cells).style(row_style)
        });

    let header = Row::new(columns.iter().enumerate().map(|(index, column)| {
        let label = match column {
            JiraIssueColumn::IssueKey => code_header,
            JiraIssueColumn::Summary => "Summary",
            JiraIssueColumn::IssueType => "Work type",
            JiraIssueColumn::Status => "Status",
            JiraIssueColumn::Field { label, .. } => label.as_str(),
        };
        if index == 0 && has_expandable {
            format!("  {label}")
        } else {
            label.to_owned()
        }
    }))
    .style(
        Style::default()
            .fg(Color::Gray)
            .add_modifier(Modifier::BOLD),
    );
    let widths = columns
        .iter()
        .zip(column_widths.iter())
        .map(|(column, width)| match column {
            JiraIssueColumn::Summary => Constraint::Fill(1),
            _ => Constraint::Length(*width),
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Table::new(table_rows, widths)
            .header(header)
            .column_spacing(1),
        area,
    );
}

fn render_empty_list(frame: &mut Frame<'_>, area: Rect) {
    let empty = Paragraph::new("No issues found.")
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(empty, area);
}
fn render_column_dropdown(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let Some(dropdown) = app.column_dropdown() else {
        return;
    };
    let longest_option = dropdown
        .options()
        .iter()
        .map(|option| option.label.chars().count())
        .max()
        .unwrap_or(0) as u16;
    let width = area.width.min((longest_option + 6).max(20));
    let height = area.height.min(16);
    if width < 20 || height < 5 {
        return;
    }

    let dropdown_area = Rect {
        x: area.x + area.width.saturating_sub(width + 1),
        y: area.y + 1,
        width,
        height,
    };
    let block = dropdown_block("Columns");
    let inner = block.inner(dropdown_area);

    frame.render_widget(Clear, dropdown_area);
    frame.render_widget(block, dropdown_area);

    let [_, padded_inner] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .areas(inner);
    let [content_area, _] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .areas(padded_inner);
    let [filter_area, options_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .areas(content_area);
    let scrollbar_area = Rect {
        x: dropdown_area.x + dropdown_area.width.saturating_sub(1),
        y: inner.y,
        width: 1,
        height: inner.height,
    };
    let [filter_icon_area, filter_text_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .areas(filter_area);

    frame.render_widget(
        filter::render_icon(dropdown.filter_state()),
        filter_icon_area,
    );
    frame.render_widget(
        filter::render_text(dropdown.filter_state()),
        filter_text_area,
    );

    let separator = "─".repeat(options_area.width as usize);
    let options = dropdown
        .visible_window(options_area.height as usize)
        .into_iter()
        .map(|entry| match entry {
            DropdownVisibleOption::Separator => ListItem::new(Line::from(Span::styled(
                separator.clone(),
                Style::default().fg(Color::DarkGray),
            ))),
            DropdownVisibleOption::NoResults => no_results_item(),
            DropdownVisibleOption::Option { index, option } => {
                let is_focused = index == dropdown.selected_index();
                let row_style = style::selected_row_style(is_focused);
                let label_style = style::dropdown_option_label_style(option.selected, is_focused);
                let icon = if option.selected { "" } else { "" };
                let icon_style = if dropdown.is_option_toggle_enabled(index) {
                    Style::default().fg(Color::Rgb(100, 150, 240))
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                let mut spans = vec![Span::styled(icon, icon_style), Span::raw(" ")];
                spans.extend(style::highlighted_spans_owned(
                    option.label.as_str(),
                    dropdown.filter(),
                    label_style,
                ));
                ListItem::new(Line::from(spans)).style(row_style)
            }
        });

    frame.render_widget(List::new(options), options_area);
    scrollbar::render_range(
        frame,
        scrollbar_area,
        dropdown.visible_row_count(),
        dropdown.visible_range(options_area.height as usize),
    );

    if dropdown.is_filter_focused() {
        let cursor_x = filter_text_area.x + dropdown.filter_cursor() as u16;
        frame.set_cursor_position(ratatui::layout::Position::new(cursor_x, filter_text_area.y));
    }
}

fn render_filter(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let trigger_width = 15;
    let [filter_area, trigger_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(trigger_width)])
        .areas(area);
    let [icon_area, text_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .areas(filter_area);

    frame.render_widget(filter::render_icon(app.filter_state()), icon_area);
    frame.render_widget(filter::render_text(app.filter_state()), text_area);
    frame.render_widget(column_trigger(), trigger_area);

    if app.is_filter_focused() {
        let cursor_x = text_area.x + app.filter_cursor() as u16;
        frame.set_cursor_position(ratatui::layout::Position::new(cursor_x, text_area.y));
    }
}

fn column_trigger() -> Paragraph<'static> {
    Paragraph::new(Line::from(vec![
        Span::styled("c", Style::default().fg(Color::Rgb(100, 150, 240))),
        Span::raw("olumns"),
    ]))
    .alignment(Alignment::Right)
}

fn with_tree_prefix<'a>(
    mut spans: Vec<Span<'a>>,
    row: &TreeRow,
    include_prefix: bool,
) -> Vec<Span<'a>> {
    if !include_prefix {
        return spans;
    }

    let mut prefixed = tree_control_spans(row)
        .into_iter()
        .map(|span| Span::styled(span.content.into_owned(), span.style))
        .collect::<Vec<_>>();
    prefixed.push(Span::raw(" "));
    prefixed.append(&mut spans);
    prefixed
}

fn tree_control_spans(row: &TreeRow) -> Vec<Span<'static>> {
    let indent = "  ".repeat(row.depth);
    let indicator = if row.expandable {
        if row.expanded {
            NERD_EXPANDED_ICON
        } else {
            NERD_COLLAPSED_ICON
        }
    } else {
        " "
    };
    vec![
        Span::raw(indent),
        Span::styled(indicator, Style::default().fg(Color::DarkGray)),
    ]
}

fn dropdown_block(title: &'static str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                title,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]))
}

fn no_results_item() -> ListItem<'static> {
    ListItem::new(Line::from(Span::styled(
        "No results",
        Style::default().fg(Color::DarkGray),
    )))
}

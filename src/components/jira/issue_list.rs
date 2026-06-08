use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table},
};

use crate::{
    App, JiraIssueColumn, KeyBindings, TreeRow,
    components::generic::{
        avatar, dropdown::DropdownVisibleOption, filter, filtered_tree::FilteredTreeViewMode,
        label, priority,
    },
    ui::{layout, scrollbar, style, theme::prefers_plain_icons},
};

const NERD_COLLAPSED_ICON: &str = "";
const NERD_EXPANDED_ICON: &str = "";

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &App, keybindings: &KeyBindings) {
    let [filter_area, content_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .areas(area);
    let [content_main, _, scrollbar_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(content_area);

    render_filter(frame, filter_area, app, keybindings);

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
        render_empty_list(frame, area, app);
        return;
    }
    let visible_range = app.visible_issue_range(area.height as usize);
    let items = visible_range.clone().map(|row_index| {
        let row = &rows[row_index];
        let item = &app.issues()[row.item_index];
        let row_style =
            style::selected_row_style(app.theme(), row_index == app.selected_issue_index());
        let mut spans = tree_control_spans(app.theme(), row, app.spinner_glyph());
        spans.push(Span::raw(" "));
        spans.extend(style::code_cell_spans(
            app.theme(),
            item,
            app.highlight_term(),
            row_style,
        ));
        let list_item = ListItem::new(Line::from(spans));
        if row.reloading {
            // Stale rows under a subtree being refreshed: greyed until fresh.
            list_item.style(Style::default().add_modifier(Modifier::DIM))
        } else {
            list_item
        }
    });

    frame.render_widget(List::new(items), area);
}

fn render_filtered_tree_table(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let rows = app.visible_issue_rows();
    if rows.is_empty() {
        render_empty_list(frame, area, app);
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
                    let header_label = if id == "priority" { "" } else { label.as_str() };
                    layout::max_column_width(&rows, header_label, |row| {
                        prefix
                            + app.issues()[row.item_index]
                                .field_values
                                .get(id)
                                .map_or(0, |value| field_display_width(id, value))
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
            let row_style =
                style::selected_row_style(app.theme(), row_index == app.selected_issue_index());
            let row_style = if row.reloading {
                row_style.add_modifier(Modifier::DIM)
            } else {
                row_style
            };
            let cells = columns
                .iter()
                .enumerate()
                .map(|(column_index, column)| {
                    let is_first = column_index == 0;
                    let spans = match column {
                        JiraIssueColumn::IssueKey => style::code_cell_spans(
                            app.theme(),
                            item,
                            app.highlight_term(),
                            row_style,
                        ),
                        JiraIssueColumn::Summary => {
                            let truncated =
                                layout::truncate_with_ellipsis(&item.label, description_width);
                            style::highlighted_spans_owned(
                                app.theme(),
                                &truncated,
                                app.highlight_term(),
                                row_style,
                            )
                        }
                        JiraIssueColumn::IssueType => style::highlighted_spans(
                            app.theme(),
                            &item.kind,
                            app.highlight_term(),
                            row_style,
                        ),
                        JiraIssueColumn::Status => style::highlighted_spans(
                            app.theme(),
                            &item.status,
                            app.highlight_term(),
                            row_style,
                        ),
                        JiraIssueColumn::Field { id, .. } => item
                            .field_values
                            .get(id)
                            .map_or_else(Vec::new, |value| field_spans(app, id, value, row_style)),
                    };
                    Cell::from(Line::from(with_tree_prefix(
                        app.theme(),
                        spans,
                        row,
                        is_first && has_expandable,
                        app.spinner_glyph(),
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
            JiraIssueColumn::Field { id, label } if id == "priority" => "",
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
            .fg(app.theme().muted_fg())
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

fn render_empty_list(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let empty = Paragraph::new("No issues found.")
        .alignment(Alignment::Center)
        .style(Style::default().fg(app.theme().muted_fg()));
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
    let block = dropdown_block("Columns", app.theme());
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
        filter::render_icon(dropdown.filter_state(), app.theme()),
        filter_icon_area,
    );
    frame.render_widget(
        filter::render_text(dropdown.filter_state(), app.theme()),
        filter_text_area,
    );

    let visible_window = dropdown.visible_window(options_area.height as usize);
    let options = visible_window.iter().map(|entry| match *entry {
        DropdownVisibleOption::Separator => ListItem::new(Line::default()),
        DropdownVisibleOption::NoResults => no_results_item(app.theme()),
        DropdownVisibleOption::Option { index, option } => {
            let is_focused = index == dropdown.selected_index();
            let row_style = style::selected_row_style(app.theme(), is_focused);
            let label_style =
                style::dropdown_option_label_style(app.theme(), option.selected, is_focused);
            let icon = if option.selected {
                if prefers_plain_icons() { "[x]" } else { "" }
            } else if prefers_plain_icons() {
                "[ ]"
            } else {
                ""
            };
            let icon_style = if dropdown.is_option_toggle_enabled(index) {
                Style::default().fg(app.theme().accent_fg())
            } else {
                Style::default().fg(app.theme().muted_fg())
            };
            let mut spans = vec![Span::styled(icon, icon_style), Span::raw(" ")];
            spans.extend(style::highlighted_spans_owned(
                app.theme(),
                option.label.as_str(),
                dropdown.filter(),
                label_style,
            ));
            ListItem::new(Line::from(spans)).style(row_style)
        }
    });

    frame.render_widget(List::new(options), options_area);

    let visible_range = dropdown.visible_range(options_area.height as usize);
    let thumb_range = scrollbar::thumb_range(
        dropdown.visible_row_count(),
        visible_range.clone(),
        scrollbar_area.height as usize,
    );
    let separator_width = dropdown_area.width.saturating_sub(1);
    for (row, entry) in visible_window.iter().enumerate() {
        if matches!(entry, DropdownVisibleOption::Separator) && separator_width > 0 {
            let line = if separator_width == 1 {
                String::from("├")
            } else {
                format!(
                    "├{}",
                    "─".repeat(separator_width.saturating_sub(1) as usize)
                )
            };
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    line,
                    Style::reset().fg(app.theme().border_fg()),
                ))),
                Rect {
                    x: dropdown_area.x,
                    y: options_area.y.saturating_add(row as u16),
                    width: separator_width,
                    height: 1,
                },
            );
        }
    }
    scrollbar::render_range(
        frame,
        scrollbar_area,
        dropdown.visible_row_count(),
        visible_range,
        app.theme(),
    );
    for (row, entry) in visible_window.iter().enumerate() {
        if matches!(entry, DropdownVisibleOption::Separator) && !thumb_range.contains(&(row + 1)) {
            let style = Style::reset().fg(app.theme().border_fg());
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled("┤", style))),
                Rect {
                    x: dropdown_area.x + dropdown_area.width.saturating_sub(1),
                    y: options_area.y.saturating_add(row as u16),
                    width: 1,
                    height: 1,
                },
            );
        }
    }

    if dropdown.is_filter_focused() {
        let cursor_x = filter_text_area.x + dropdown.filter_cursor() as u16;
        frame.set_cursor_position(ratatui::layout::Position::new(cursor_x, filter_text_area.y));
    }
}
fn render_filter(frame: &mut Frame<'_>, area: Rect, app: &App, keybindings: &KeyBindings) {
    let trigger_width = 9u16.saturating_add(keybindings.open_columns_label().len() as u16);
    let [filter_area, trigger_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(trigger_width)])
        .areas(area);
    let [icon_area, text_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .areas(filter_area);

    frame.render_widget(
        filter::render_icon(app.filter_state(), app.theme()),
        icon_area,
    );
    frame.render_widget(
        filter::render_text(app.filter_state(), app.theme()),
        text_area,
    );
    frame.render_widget(column_trigger(keybindings, app.theme()), trigger_area);

    if app.is_filter_focused() {
        let cursor_x = text_area.x + app.filter_cursor() as u16;
        frame.set_cursor_position(ratatui::layout::Position::new(cursor_x, text_area.y));
    }
}

fn column_trigger(
    keybindings: &KeyBindings,
    theme: &crate::ui::theme::Theme,
) -> Paragraph<'static> {
    Paragraph::new(Line::from(vec![
        Span::styled(
            keybindings.open_columns_label(),
            Style::default().fg(theme.accent_fg()),
        ),
        Span::raw(" columns"),
    ]))
    .alignment(Alignment::Right)
}

fn field_spans(app: &App, field_id: &str, value: &str, base_style: Style) -> Vec<Span<'static>> {
    match field_id {
        "priority" => priority::spans(app.theme(), value, app.highlight_term(), base_style, true),
        "assignee" | "reporter" => avatar::bubble_only_spans(app.theme(), value),
        "labels" => label::spans(app.theme(), value, app.highlight_term(), base_style),
        _ => style::highlighted_spans_owned(app.theme(), value, app.highlight_term(), base_style),
    }
}

fn field_display_width(field_id: &str, value: &str) -> usize {
    match field_id {
        "priority" => priority::display_width(value, true),
        "assignee" | "reporter" => avatar::bubble_width(value),
        "labels" => label::display_width(value),
        _ => value.chars().count(),
    }
}

fn with_tree_prefix<'a>(
    theme: &crate::ui::theme::Theme,
    mut spans: Vec<Span<'a>>,
    row: &TreeRow,
    include_prefix: bool,
    spinner: &str,
) -> Vec<Span<'a>> {
    if !include_prefix {
        return spans;
    }

    let mut prefixed = tree_control_spans(theme, row, spinner)
        .into_iter()
        .map(|span| Span::styled(span.content.into_owned(), span.style))
        .collect::<Vec<_>>();
    prefixed.push(Span::raw(" "));
    prefixed.append(&mut spans);
    prefixed
}

fn tree_control_spans(
    theme: &crate::ui::theme::Theme,
    row: &TreeRow,
    spinner: &str,
) -> Vec<Span<'static>> {
    let indent = "  ".repeat(row.depth);
    let indicator = if row.loading {
        // The spinner glyph is owned by the App and advances each tick; clone
        // it into the span so the returned spans stay 'static.
        return vec![
            Span::raw(indent),
            Span::styled(spinner.to_owned(), Style::default().fg(theme.border_fg())),
        ];
    } else if row.expandable {
        if row.expanded {
            expanded_icon()
        } else {
            collapsed_icon()
        }
    } else {
        " "
    };
    vec![
        Span::raw(indent),
        Span::styled(indicator, Style::default().fg(theme.border_fg())),
    ]
}

fn collapsed_icon() -> &'static str {
    if prefers_plain_icons() {
        ">"
    } else {
        NERD_COLLAPSED_ICON
    }
}

fn expanded_icon() -> &'static str {
    if prefers_plain_icons() {
        "v"
    } else {
        NERD_EXPANDED_ICON
    }
}

fn dropdown_block(title: &'static str, theme: &crate::ui::theme::Theme) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_fg()))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                title,
                Style::default()
                    .fg(theme.accent_fg())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]))
}

fn no_results_item(theme: &crate::ui::theme::Theme) -> ListItem<'static> {
    ListItem::new(Line::from(Span::styled(
        "No results",
        Style::default().fg(theme.muted_fg()),
    )))
}

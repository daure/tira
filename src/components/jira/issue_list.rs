use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    App, JiraIssueColumn, KeyBindings, TreeRow,
    components::generic::{
        avatar, dropdown::{DropdownVisibleOption, MultiSelectDropdownState}, filter, filtered_tree::FilteredTreeViewMode,
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

    render_filter(frame, filter_area, app, keybindings);

    let view_mode = app.filtered_tree_view_mode();
    let table = (view_mode == FilteredTreeViewMode::Table)
        .then(|| table_layout(app))
        .flatten();
    // content_main loses the gap column and the vertical-scrollbar column.
    let columns_viewport = content_area.width.saturating_sub(2);
    let h_scrolling = table
        .as_ref()
        .is_some_and(|layout| layout.strip_width > columns_viewport);

    // A full-width bottom row carries the horizontal scrollbar; the vertical
    // scrollbar then stops one row short and sits on top of it, matching the
    // board.
    let (body_area, hbar_area) = if h_scrolling && content_area.height > 1 {
        let [body, bar] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .areas(content_area);
        (body, Some(bar))
    } else {
        (content_area, None)
    };

    let [content_main, _, scrollbar_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(body_area);

    let h_offset = match view_mode {
        FilteredTreeViewMode::List => {
            render_filtered_tree_list(frame, content_main, app);
            None
        }
        FilteredTreeViewMode::Table => match &table {
            None => {
                render_empty_list(frame, content_main, app);
                None
            }
            Some(layout) => render_table(frame, content_main, app, layout, h_scrolling),
        },
    };

    let scrollbar_viewport_height = match view_mode {
        FilteredTreeViewMode::List => content_main.height,
        FilteredTreeViewMode::Table => content_main.height.saturating_sub(1),
    };
    let scrollbar_render_area = match view_mode {
        FilteredTreeViewMode::List => scrollbar_area,
        FilteredTreeViewMode::Table => Rect {
            x: scrollbar_area.x,
            y: scrollbar_area.y.saturating_add(1),
            width: scrollbar_area.width,
            height: scrollbar_area.height.saturating_sub(1),
        },
    };
    scrollbar::render(frame, scrollbar_render_area, scrollbar_viewport_height, app);

    if let (Some(bar), Some(offset), Some(layout)) = (hbar_area, h_offset, &table) {
        let viewport = content_main.width as usize;
        scrollbar::render_range_horizontal(
            frame,
            bar,
            layout.strip_width as usize,
            offset as usize..offset as usize + viewport,
            app.theme(),
        );
    }

    render_column_dropdown(frame, area, app);
}

/// Rows and natural per-column widths for the issue table, plus the total strip
/// width that decides whether horizontal scrolling is needed. `None` when there
/// are no rows to show.
struct TableLayout {
    rows: Vec<TreeRow>,
    widths: Vec<u16>,
    strip_width: u16,
    has_expandable: bool,
}

fn table_layout(app: &App) -> Option<TableLayout> {
    let rows = app.visible_issue_rows();
    if rows.is_empty() {
        return None;
    }
    let has_expandable = rows.iter().any(|row| row.expandable);
    let columns = app.visible_issue_columns();
    let tree_prefix_width = if has_expandable {
        rows.iter().map(|row| row.depth * 2 + 2).max().unwrap_or(2) as u16
    } else {
        0
    };
    let widths = compute_column_widths(app, &rows, columns, tree_prefix_width);
    let spacing = columns.len().saturating_sub(1) as u16;
    let strip_width = widths.iter().sum::<u16>() + spacing;
    Some(TableLayout {
        rows,
        widths,
        strip_width,
        has_expandable,
    })
}

/// Renders the issue table into `area`. When `scrolling`, composes full-width
/// rows and slices them to the animated horizontal offset (returned for the
/// scrollbar); otherwise lays the columns out with ratatui's Table so the
/// summary fills the leftover width.
fn render_table(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    layout: &TableLayout,
    scrolling: bool,
) -> Option<u16> {
    let columns = app.visible_issue_columns();

    if scrolling {
        let content_width = area.width;
        let max_offset = layout.strip_width.saturating_sub(content_width);
        let h_offset = app.resolve_table_h_offset(max_offset);

        // The summary column renders at its full natural width here (no Fill);
        // its own width bounds truncation so the strip matches `strip_width`.
        let description_width = columns
            .iter()
            .zip(layout.widths.iter())
            .find_map(|(column, width)| {
                matches!(column, JiraIssueColumn::Summary).then_some(*width as usize)
            })
            .unwrap_or(0);

        let header_style = Style::default()
            .fg(app.theme().muted_fg())
            .add_modifier(Modifier::BOLD);

        // The first column (Work) stays pinned at the left edge while the rest
        // scroll; `sticky_width` covers it plus the trailing column gap.
        let sticky_width =
            (layout.widths.first().copied().unwrap_or(0) + 1).min(content_width);

        let header_cells = header_cell_spans(columns, layout.has_expandable);
        let header_sticky = header_cells.first().cloned().unwrap_or_default();
        let mut lines = vec![compose_strip_line(header_cells, &layout.widths, header_style)];
        let mut sticky_lines = vec![padded_cell(header_sticky, sticky_width as usize, header_style)];

        for row_index in app.visible_issue_range(area.height.saturating_sub(1) as usize) {
            let row = &layout.rows[row_index];
            let row_style = row_table_style(app, row, row_index);
            let cells = columns
                .iter()
                .enumerate()
                .map(|(column_index, column)| {
                    column_cell_spans(
                        app,
                        row,
                        column,
                        column_index,
                        description_width,
                        layout.has_expandable,
                        row_style,
                    )
                })
                .collect::<Vec<_>>();
            let sticky = cells.first().cloned().unwrap_or_default();
            lines.push(compose_strip_line(cells, &layout.widths, row_style));
            sticky_lines.push(padded_cell(sticky, sticky_width as usize, row_style));
        }

        let sliced = lines
            .into_iter()
            .map(|line| layout::slice_line(line, h_offset as usize, content_width as usize))
            .collect::<Vec<_>>();
        frame.render_widget(Paragraph::new(sliced), area);

        // Overlay the pinned Work column on top of the scrolled strip.
        if sticky_width > 0 {
            let sticky_area = Rect {
                width: sticky_width,
                ..area
            };
            frame.render_widget(Paragraph::new(sticky_lines), sticky_area);
        }
        return Some(h_offset);
    }

    // Fits the viewport: keep the animated offset at rest so returning to a wide
    // enough viewport doesn't leave the strip panned.
    app.resolve_table_h_offset(0);
    let spacing = columns.len().saturating_sub(1) as u16;
    let fixed_width = columns
        .iter()
        .zip(layout.widths.iter())
        .filter_map(|(column, width)| {
            (!matches!(column, JiraIssueColumn::Summary)).then_some(*width)
        })
        .sum::<u16>();
    let description_width = area.width.saturating_sub(fixed_width + spacing) as usize;
    let table_rows = app
        .visible_issue_range(area.height.saturating_sub(1) as usize)
        .map(|row_index| {
            build_table_row(
                app,
                &layout.rows,
                columns,
                row_index,
                description_width,
                layout.has_expandable,
            )
        });
    let header = build_header(app, columns, layout.has_expandable);
    let widths = columns
        .iter()
        .zip(layout.widths.iter())
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
    None
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

/// Assembles per-column span groups into one full-width line: each column is
/// padded with `base_style` blanks to its width, joined by a single-cell gap so
/// columns align exactly as the non-scrolling Table lays them out.
fn compose_strip_line<'a>(
    cells: Vec<Vec<Span<'a>>>,
    column_widths: &[u16],
    base_style: Style,
) -> Line<'a> {
    let mut spans = Vec::new();
    for (index, (cell, width)) in cells.into_iter().zip(column_widths.iter()).enumerate() {
        if index > 0 {
            spans.push(Span::styled(" ", base_style));
        }
        let mut used = 0usize;
        for mut span in cell {
            used += span.content.width();
            // Layer the row/header style under each span so the selection
            // background (and header colour) covers cells whose own style only
            // sets a foreground — e.g. the priority icon.
            span.style = base_style.patch(span.style);
            spans.push(span);
        }
        let pad = (*width as usize).saturating_sub(used);
        if pad > 0 {
            spans.push(Span::styled(" ".repeat(pad), base_style));
        }
    }
    Line::from(spans)
}

/// Pads a single column's spans with `base_style` blanks to exactly `width`
/// cells (slicing if it somehow overruns), used to render the pinned column.
fn padded_cell(spans: Vec<Span<'_>>, width: usize, base_style: Style) -> Line<'_> {
    let mut out = Vec::new();
    let mut used = 0usize;
    for mut span in spans {
        used += span.content.width();
        span.style = base_style.patch(span.style);
        out.push(span);
    }
    if used > width {
        return layout::slice_line(Line::from(out), 0, width);
    }
    if used < width {
        out.push(Span::styled(" ".repeat(width - used), base_style));
    }
    Line::from(out)
}

/// Header label spans per column, mirroring `build_header`'s first-column
/// indent for the tree controls.
fn header_cell_spans(columns: &[JiraIssueColumn], has_expandable: bool) -> Vec<Vec<Span<'static>>> {
    columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let label = column.header_label();
            let text = if index == 0 && has_expandable {
                format!("  {label}")
            } else {
                label.to_owned()
            };
            vec![Span::raw(text)]
        })
        .collect()
}

fn compute_column_widths(
    app: &App,
    rows: &[TreeRow],
    columns: &[JiraIssueColumn],
    tree_prefix_width: u16,
) -> Vec<u16> {
    let first_column = columns.first();
    let prefix_for = |is_first: bool| usize::from(is_first) * tree_prefix_width as usize;
    let code_width = layout::max_column_width(rows, "Work", |row| {
        prefix_for(matches!(first_column, Some(JiraIssueColumn::IssueKey)))
            + 2
            + app.issues()[row.item_index].id.chars().count()
    });
    let type_width = layout::max_column_width(rows, "Work type", |row| {
        prefix_for(matches!(first_column, Some(JiraIssueColumn::IssueType)))
            + app.issues()[row.item_index].kind.chars().count()
    });
    let status_width = layout::max_column_width(rows, "Status", |row| {
        prefix_for(matches!(first_column, Some(JiraIssueColumn::Status)))
            + app.issues()[row.item_index].status.chars().count()
    });

    columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let prefix = prefix_for(index == 0);
            match column {
                JiraIssueColumn::IssueKey => code_width,
                JiraIssueColumn::IssueType => type_width,
                JiraIssueColumn::Status => status_width,
                JiraIssueColumn::Field { id, .. } => {
                    layout::max_column_width(rows, column.header_label(), |row| {
                        prefix
                            + app.issues()[row.item_index]
                                .field_values
                                .get(id)
                                .map_or(0, |value| field_display_width(id, value))
                    })
                }
                JiraIssueColumn::Summary => layout::max_column_width(rows, "Summary", |row| {
                    prefix + app.issues()[row.item_index].label.chars().count()
                })
                .min(SUMMARY_MAX_WIDTH),
            }
        })
        .collect::<Vec<_>>()
}

/// Upper bound (in cells) on the summary column's natural width, so one long
/// summary can't blow the horizontal strip out to an unusable width. Longer
/// summaries are truncated with an ellipsis when the table scrolls.
const SUMMARY_MAX_WIDTH: u16 = 60;

fn build_table_row<'a>(
    app: &'a App,
    rows: &'a [TreeRow],
    columns: &[JiraIssueColumn],
    row_index: usize,
    description_width: usize,
    has_expandable: bool,
) -> Row<'a> {
    let row = &rows[row_index];
    let row_style = row_table_style(app, row, row_index);
    let cells = columns
        .iter()
        .enumerate()
        .map(|(column_index, column)| {
            Cell::from(Line::from(column_cell_spans(
                app,
                row,
                column,
                column_index,
                description_width,
                has_expandable,
                row_style,
            )))
        })
        .collect::<Vec<_>>();
    Row::new(cells).style(row_style)
}

/// The base style for a table row: selection highlight plus a dim modifier for
/// rows under a subtree being reloaded.
fn row_table_style(app: &App, row: &TreeRow, row_index: usize) -> Style {
    let row_style = style::selected_row_style(app.theme(), row_index == app.selected_issue_index());
    if row.reloading {
        row_style.add_modifier(Modifier::DIM)
    } else {
        row_style
    }
}

/// Spans for one column's cell, including the tree-control prefix on the first
/// column. `description_width` bounds the (truncated) summary column.
fn column_cell_spans<'a>(
    app: &'a App,
    row: &'a TreeRow,
    column: &JiraIssueColumn,
    column_index: usize,
    description_width: usize,
    has_expandable: bool,
    row_style: Style,
) -> Vec<Span<'a>> {
    let item = &app.issues()[row.item_index];
    let spans = match column {
        JiraIssueColumn::IssueKey => {
            style::code_cell_spans(app.theme(), item, app.highlight_term(), row_style)
        }
        JiraIssueColumn::Summary => {
            let truncated = layout::truncate_with_ellipsis(&item.label, description_width);
            style::highlighted_spans_owned(app.theme(), &truncated, app.highlight_term(), row_style)
        }
        JiraIssueColumn::IssueType => {
            style::highlighted_spans(app.theme(), &item.kind, app.highlight_term(), row_style)
        }
        JiraIssueColumn::Status => {
            style::highlighted_spans(app.theme(), &item.status, app.highlight_term(), row_style)
        }
        JiraIssueColumn::Field { id, .. } => item
            .field_values
            .get(id)
            .map_or_else(Vec::new, |value| field_spans(app, id, value, row_style)),
    };
    with_tree_prefix(
        app.theme(),
        spans,
        row,
        column_index == 0 && has_expandable,
        app.spinner_glyph(),
    )
}

fn build_header(app: &App, columns: &[JiraIssueColumn], has_expandable: bool) -> Row<'static> {
    let cells = columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let label = column.header_label();
            if index == 0 && has_expandable {
                format!("  {label}")
            } else {
                label.to_owned()
            }
        })
        .collect::<Vec<_>>();
    Row::new(cells).style(
        Style::default()
            .fg(app.theme().muted_fg())
            .add_modifier(Modifier::BOLD),
    )
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

    let layout = dropdown_layout(dropdown_area, inner);

    frame.render_widget(
        filter::render_icon(dropdown.filter_state(), app.theme()),
        layout.filter_icon_area,
    );
    frame.render_widget(
        filter::render_text(dropdown.filter_state(), app.theme()),
        layout.filter_text_area,
    );

    render_dropdown_options(frame, layout.options_area, app, dropdown);
    render_dropdown_separators(frame, dropdown_area, &layout, app, dropdown);

    if dropdown.is_filter_focused() {
        let cursor_x = layout.filter_text_area.x + dropdown.filter_cursor() as u16;
        frame.set_cursor_position(ratatui::layout::Position::new(
            cursor_x,
            layout.filter_text_area.y,
        ));
    }
}

struct DropdownLayout {
    filter_icon_area: Rect,
    filter_text_area: Rect,
    options_area: Rect,
    scrollbar_area: Rect,
}

fn dropdown_layout(dropdown_area: Rect, inner: Rect) -> DropdownLayout {
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

    DropdownLayout {
        filter_icon_area,
        filter_text_area,
        options_area,
        scrollbar_area,
    }
}

fn render_dropdown_options(
    frame: &mut Frame<'_>,
    options_area: Rect,
    app: &App,
    dropdown: &MultiSelectDropdownState<JiraIssueColumn>,
) {
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
                if prefers_plain_icons() { "[x]" } else { "" }
            } else if prefers_plain_icons() {
                "[ ]"
            } else {
                ""
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
}

fn render_dropdown_separators(
    frame: &mut Frame<'_>,
    dropdown_area: Rect,
    layout: &DropdownLayout,
    app: &App,
    dropdown: &MultiSelectDropdownState<JiraIssueColumn>,
) {
    let options_area = layout.options_area;
    let scrollbar_area = layout.scrollbar_area;
    let visible_window = dropdown.visible_window(options_area.height as usize);
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
    frame.render_widget(column_trigger(app.theme()), trigger_area);

    if app.is_filter_focused() {
        let cursor_x = text_area.x + app.filter_cursor() as u16;
        frame.set_cursor_position(ratatui::layout::Position::new(cursor_x, text_area.y));
    }
}

fn column_trigger(theme: &crate::ui::theme::Theme) -> Paragraph<'static> {
    Paragraph::new(Line::from(vec![
        Span::styled("c", Style::default().fg(theme.accent_fg())),
        Span::styled("olumns", Style::default().fg(theme.muted_fg())),
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

use std::collections::BTreeMap;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthStr;

use crate::{
    App, BoardGrouping, KeyBindings,
    app::{board_group_key, board_grouped_lanes, board_issue_column},
    components::{
        generic::{avatar, filter, priority},
        jira::work_item_key,
    },
    services::jira::{BoardData, BoardSwimlaneSummary, IssueSummary},
    ui::{
        layout::truncate_with_ellipsis,
        scrollbar,
        theme::{Theme, prefers_plain_icons},
    },
};

const NERD_COLLAPSED_ICON: &str = "";
const NERD_EXPANDED_ICON: &str = "";

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &App, keybindings: &KeyBindings) {
    let theme = app.theme();
    let [top_area, content_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .areas(area);
    let group_width = (app.board_grouping().label().len() as u16 + 9).max(16);
    let [filter_area, group_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(group_width)])
        .areas(top_area);
    render_filter(frame, filter_area, app, keybindings);
    render_group_trigger(frame, group_area, app);

    let [main_content_area, _, scrollbar_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(content_area);
    let warning = app.board().error();
    let board_area = if let Some(message) = warning {
        let [warning_area, board_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .areas(main_content_area);
        let text = format!("Board endpoint failed; showing status fallback: {message}");
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                truncate_with_ellipsis(&text, warning_area.width as usize),
                Style::default().fg(theme.warning_fg()),
            ))),
            warning_area,
        );
        board_area
    } else {
        main_content_area
    };

    let Some(data) = app.board().data() else {
        let message = warning.unwrap_or("Jira board has not loaded yet.");
        let body = Paragraph::new(Line::from(Span::styled(
            truncate_with_ellipsis(message, board_area.width as usize),
            Style::default().fg(theme.warning_fg()),
        )));
        frame.render_widget(body, board_area);
        return;
    };
    let search = app.board_filter();

    let issues_by_key = data
        .issues
        .iter()
        .map(|issue| (issue.key.as_str(), issue))
        .collect::<BTreeMap<_, _>>();
    let grouped_lanes = board_grouped_lanes(data, app.board_grouping());
    let visible_lanes = grouped_lanes
        .iter()
        .filter(|lane| {
            lane.issue_keys.iter().any(|key| {
                issues_by_key.get(key.as_str()).is_some_and(|issue| {
                    board_issue_column(data, issue) < data.columns.len()
                        && board_issue_matches_filter(issue, search)
                })
            })
        })
        .collect::<Vec<_>>();

    if visible_lanes.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                if search.is_empty() {
                    "No board issues"
                } else {
                    "No board issues match search"
                }
                .to_owned(),
                Style::default().fg(theme.muted_fg()),
            ))),
            board_area,
        );
        return;
    }

    let column_count = data.columns.len().max(1);
    let constraints = (0..column_count)
        .map(|_| Constraint::Ratio(1, column_count as u32))
        .collect::<Vec<_>>();
    let columns_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(board_area);

    let widths = columns_layout
        .iter()
        .map(|r| r.width as usize)
        .collect::<Vec<_>>();
    app.board().column_widths.replace(widths);

    let rendered = generate_rendered_board(
        app,
        data,
        &issues_by_key,
        &visible_lanes,
        theme,
        &columns_layout,
        search,
    );

    let selected_key_or_group = app
        .selected_board_issue_key()
        .map(String::from)
        .or_else(|| app.selected_board_group().map(board_group_key));

    let mut sel_y_start = 0;
    let mut sel_y_end = 0;
    if let Some(target_key) = selected_key_or_group
        && let Some(item) = rendered.layout.iter().find(|item| item.key == *target_key)
    {
        sel_y_start = item.y_start;
        sel_y_end = item.y_end;
    }

    let total_lines = rendered.lines.len();
    let mut scroll_offset = app.board().scroll_offset.get();
    let viewport_height = board_area.height as usize;
    if sel_y_start < scroll_offset {
        scroll_offset = sel_y_start;
    } else if sel_y_end > scroll_offset + viewport_height {
        scroll_offset = sel_y_end.saturating_sub(viewport_height);
    }
    scroll_offset = scroll_offset.min(total_lines.saturating_sub(viewport_height));
    app.board().scroll_offset.set(scroll_offset);

    let mut visible_lines = rendered
        .lines
        .iter()
        .skip(scroll_offset)
        .take(viewport_height)
        .cloned()
        .collect::<Vec<_>>();
    for (index, sticky_heading) in sticky_headings(&rendered.headings, scroll_offset)
        .into_iter()
        .take(viewport_height)
        .enumerate()
    {
        if let Some(line) = visible_lines.get_mut(index) {
            *line = sticky_heading;
        }
    }

    frame.render_widget(Paragraph::new(visible_lines), board_area);

    if total_lines > viewport_height {
        scrollbar::render_range(
            frame,
            scrollbar_area,
            total_lines,
            scroll_offset..scroll_offset + viewport_height,
            theme,
        );
    }
}

struct RenderedBoard {
    lines: Vec<Line<'static>>,
    layout: Vec<BoardLayoutItem>,
    headings: Vec<BoardHeading>,
}

struct BoardHeading {
    y: usize,
    level: usize,
    line: Line<'static>,
}

struct BoardLayoutItem {
    key: String,
    y_start: usize,
    y_end: usize,
}

fn generate_rendered_board(
    app: &App,
    data: &BoardData,
    issues_by_key: &BTreeMap<&str, &IssueSummary>,
    visible_lanes: &[&BoardSwimlaneSummary],
    theme: &Theme,
    columns_layout: &[Rect],
    search: &str,
) -> RenderedBoard {
    let mut rendered = RenderedBoard {
        lines: Vec::new(),
        layout: Vec::new(),
        headings: Vec::new(),
    };
    let selected_key = app.selected_board_issue_key();
    let selected_group = app.selected_board_group();
    let grouping = app.board_grouping();
    let original_visible_lanes = data
        .swimlanes
        .iter()
        .filter(|lane| !filtered_lane_issue_keys(lane, issues_by_key, search).is_empty())
        .collect::<Vec<_>>();

    if grouping == BoardGrouping::Assignee && original_visible_lanes.len() > 1 {
        let grouped_lanes = board_grouped_lanes(data, BoardGrouping::Assignee);
        for swimlane in original_visible_lanes {
            let swimlane_keys = filtered_lane_issue_keys(swimlane, issues_by_key, search);
            let swimlane_heading = board_heading_line(
                &swimlane.name,
                swimlane_keys.len(),
                false,
                false,
                theme,
                columns_layout,
            );
            push_heading(&mut rendered, swimlane_heading, 0, None);

            for group in &grouped_lanes {
                let group_keys = group
                    .issue_keys
                    .iter()
                    .filter(|key| {
                        swimlane_keys
                            .iter()
                            .any(|swimlane_key| swimlane_key == *key)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if group_keys.is_empty() {
                    continue;
                }

                let collapsed = app.is_board_group_collapsed(&group.name);
                let selected = selected_group == Some(group.name.as_str());
                let group_heading = board_heading_line(
                    &group.name,
                    group_keys.len(),
                    collapsed,
                    selected,
                    theme,
                    columns_layout,
                );
                push_heading(
                    &mut rendered,
                    group_heading,
                    1,
                    Some(board_group_key(&group.name)),
                );
                if collapsed {
                    continue;
                }

                let section = BoardSwimlaneSummary {
                    id: group.id.clone(),
                    name: group.name.clone(),
                    issue_keys: group_keys,
                };
                render_columns_block(
                    &mut rendered,
                    data,
                    issues_by_key,
                    &section,
                    true,
                    selected_key,
                    theme,
                    columns_layout,
                    search,
                );
            }
        }
        return rendered;
    }

    for lane in visible_lanes {
        let lane_issues = filtered_lane_issue_keys(lane, issues_by_key, search);
        let show_header =
            grouping == BoardGrouping::Assignee || visible_lanes.len() > 1 || lane.name != "Issues";
        if show_header {
            let collapsed = app.is_board_group_collapsed(&lane.name);
            let selected = selected_group == Some(lane.name.as_str());
            let header_line = board_heading_line(
                &lane.name,
                lane_issues.len(),
                collapsed,
                selected,
                theme,
                columns_layout,
            );
            push_heading(
                &mut rendered,
                header_line,
                0,
                Some(board_group_key(&lane.name)),
            );
            if collapsed {
                continue;
            }
        }

        render_columns_block(
            &mut rendered,
            data,
            issues_by_key,
            lane,
            show_header,
            selected_key,
            theme,
            columns_layout,
            search,
        );
    }

    rendered
}

fn filtered_lane_issue_keys(
    lane: &BoardSwimlaneSummary,
    issues_by_key: &BTreeMap<&str, &IssueSummary>,
    search: &str,
) -> Vec<String> {
    lane.issue_keys
        .iter()
        .filter(|key| {
            issues_by_key
                .get(key.as_str())
                .is_some_and(|issue| board_issue_matches_filter(issue, search))
        })
        .cloned()
        .collect()
}

fn board_heading_line(
    name: &str,
    count: usize,
    collapsed: bool,
    selected: bool,
    theme: &Theme,
    columns_layout: &[Rect],
) -> Line<'static> {
    let marker = if collapsed {
        collapsed_icon()
    } else {
        expanded_icon()
    };
    let suffix = if count == 1 {
        "work item"
    } else {
        "work items"
    };
    let header_text = format!(" {marker} {name} ({count} {suffix}) ");
    let text_len = header_text.chars().count();
    let fill_char = if selected { "═" } else { "─" };
    let border_style = if selected {
        Style::default()
            .fg(theme.accent_fg())
            .bg(theme.selected_bg())
    } else {
        Style::default().fg(theme.border_fg())
    };
    let text_style = if selected {
        Style::default()
            .fg(theme.selected_fg())
            .bg(theme.selected_bg())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.subtle_fg())
    };
    let total_width = columns_layout.iter().map(|r| r.width).sum::<u16>();
    let filler_len = (total_width as usize).saturating_sub(text_len);
    Line::from(vec![
        Span::styled(header_text, text_style),
        Span::styled(fill_char.repeat(filler_len), border_style),
    ])
}

fn push_heading(
    rendered: &mut RenderedBoard,
    line: Line<'static>,
    level: usize,
    layout_key: Option<String>,
) {
    let y_start = rendered.lines.len();
    rendered.lines.push(line.clone());
    let y_end = rendered.lines.len();
    if let Some(key) = layout_key {
        rendered.layout.push(BoardLayoutItem {
            key,
            y_start,
            y_end,
        });
    }
    rendered.headings.push(BoardHeading {
        y: y_start,
        level,
        line,
    });
}

fn render_columns_block(
    rendered: &mut RenderedBoard,
    data: &BoardData,
    issues_by_key: &BTreeMap<&str, &IssueSummary>,
    lane: &BoardSwimlaneSummary,
    show_header: bool,
    selected_key: Option<&str>,
    theme: &Theme,
    columns_layout: &[Rect],
    search: &str,
) {
    let mut max_inner_len = 1;
    let mut columns_card_lines = Vec::new();
    for (c_idx, _column) in data.columns.iter().enumerate() {
        let column_width = columns_layout[c_idx].width;
        let col_issues = lane_column_issues(data, issues_by_key, lane, c_idx, search);
        let mut col_card_lines = Vec::new();
        let mut card_positions = Vec::new();
        let mut current_local_y = 0;
        for issue in &col_issues {
            let card_selected = selected_key == Some(issue.key.as_str());
            let card_width = column_width.saturating_sub(4);
            let lines_for_card = issue_card_lines(issue, card_selected, theme, card_width, search);
            let card_h = lines_for_card.len();
            card_positions.push((issue.key.clone(), current_local_y, current_local_y + card_h));
            for line in lines_for_card {
                let mut spans = vec![Span::styled(" ", Style::default())];
                spans.extend(line.spans);
                spans.push(Span::styled(" ", Style::default()));
                col_card_lines.push(Line::from(spans));
            }
            current_local_y += card_h;
        }
        if col_issues.is_empty() {
            let no_issues_text = "No issues";
            let pad = (column_width as usize).saturating_sub(no_issues_text.chars().count() + 2);
            col_card_lines.push(Line::from(Span::styled(
                format!("{no_issues_text}{}", " ".repeat(pad)),
                Style::default().fg(theme.muted_fg()),
            )));
        }
        max_inner_len = max_inner_len.max(col_card_lines.len());
        columns_card_lines.push((col_card_lines, card_positions));
    }

    let block_height = max_inner_len + 2;
    let columns_y_start = rendered.lines.len();
    let header_y_start = if show_header {
        columns_y_start.saturating_sub(1)
    } else {
        columns_y_start
    };
    for (_, card_positions) in columns_card_lines.iter() {
        for (key, local_start, local_end) in card_positions {
            let card_y_start = columns_y_start + 1 + local_start;
            let card_y_end = columns_y_start + 1 + local_end;
            let y_start = if *local_start == 0 {
                header_y_start
            } else {
                card_y_start
            };
            let y_end = if *local_end >= max_inner_len {
                columns_y_start + block_height
            } else {
                card_y_end
            };
            rendered.layout.push(BoardLayoutItem {
                key: key.clone(),
                y_start,
                y_end,
            });
        }
    }

    let mut col_lines_list = Vec::new();
    for (c_idx, (col_card_lines, _)) in columns_card_lines.into_iter().enumerate() {
        let column_width = columns_layout[c_idx].width;
        let col_issues = lane_column_issues(data, issues_by_key, lane, c_idx, search);
        let column_selected =
            selected_key.is_some_and(|key| col_issues.iter().any(|issue| issue.key == key));

        let (left_border, right_border, top_left, top_right, bottom_left, bottom_right, fill_char) =
            if column_selected {
                ("║", "║", "╔", "╗", "╚", "╝", "═")
            } else {
                ("│", "│", "┌", "┐", "└", "┘", "─")
            };
        let border_style = Style::default().fg(if column_selected {
            theme.accent_fg()
        } else {
            theme.border_fg()
        });

        let mut col_lines = Vec::with_capacity(block_height);

        let column = &data.columns[c_idx];
        let count = col_issues.len();
        let checkmark = if column.name.to_lowercase() == "done" && count > 0 {
            " ✓"
        } else {
            ""
        };
        let title_text = format!(" {}{} {} ", column.name.to_uppercase(), checkmark, count);
        let title_len = title_text.chars().count();
        let fill_count = (column_width as usize).saturating_sub(title_len + 2);
        let top_border_str = format!(
            "{top_left}{title_text}{}{top_right}",
            fill_char.repeat(fill_count)
        );
        col_lines.push(Line::from(Span::styled(top_border_str, border_style)));

        let mut padded_card_lines = col_card_lines;
        while padded_card_lines.len() < max_inner_len {
            padded_card_lines.push(Line::from(Span::styled(
                " ".repeat((column_width as usize).saturating_sub(2)),
                Style::default(),
            )));
        }

        for line in padded_card_lines {
            let mut spans = vec![Span::styled(left_border.to_owned(), border_style)];
            spans.extend(line.spans);
            spans.push(Span::styled(right_border.to_owned(), border_style));
            col_lines.push(Line::from(spans));
        }

        let bottom_border_str = format!(
            "{bottom_left}{}{bottom_right}",
            fill_char.repeat((column_width as usize).saturating_sub(2))
        );
        col_lines.push(Line::from(Span::styled(bottom_border_str, border_style)));

        col_lines_list.push(col_lines);
    }

    if let Some(first_column_lines) = col_lines_list.first() {
        for (row_index, _) in first_column_lines.iter().enumerate().take(block_height) {
            let mut joined_spans = Vec::new();
            for column_lines in col_lines_list.iter().take(data.columns.len()) {
                joined_spans.extend(column_lines[row_index].spans.clone());
            }
            let line = Line::from(joined_spans);
            if row_index == 0 {
                rendered.headings.push(BoardHeading {
                    y: rendered.lines.len(),
                    level: 2,
                    line: line.clone(),
                });
            }
            rendered.lines.push(line);
        }
    }
}

fn sticky_headings(headings: &[BoardHeading], scroll_offset: usize) -> Vec<Line<'static>> {
    let mut sticky = Vec::new();
    let mut min_y = 0;
    for level in 0..=2 {
        if let Some(heading) = headings
            .iter()
            .take_while(|heading| heading.y <= scroll_offset)
            .filter(|heading| heading.level == level && heading.y >= min_y)
            .last()
        {
            min_y = heading.y;
            sticky.push(heading.line.clone());
        }
    }
    sticky
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

fn render_filter(frame: &mut Frame<'_>, area: Rect, app: &App, _keybindings: &KeyBindings) {
    let [icon_area, text_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .areas(area);

    frame.render_widget(
        filter::render_icon(app.board_filter_state(), app.theme()),
        icon_area,
    );
    frame.render_widget(
        filter::render_text(app.board_filter_state(), app.theme()),
        text_area,
    );

    if app.is_board_filter_focused() {
        let cursor_x = text_area.x + app.board_filter_cursor() as u16;
        frame.set_cursor_position(ratatui::layout::Position::new(cursor_x, text_area.y));
    }
}

fn render_group_trigger(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = app.theme();
    let label = app.board_grouping().label();
    let text = Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(
            "G",
            Style::default()
                .fg(theme.accent_fg())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("roup: ", Style::default().fg(theme.muted_fg())),
        Span::styled(
            label.to_owned(),
            Style::default().fg(theme.selected_alt_fg()),
        ),
    ]);
    frame.render_widget(Paragraph::new(text), area);
}

fn lane_column_issues<'a>(
    data: &BoardData,
    issues_by_key: &BTreeMap<&str, &'a IssueSummary>,
    lane: &crate::services::jira::BoardSwimlaneSummary,
    column_index: usize,
    search: &str,
) -> Vec<&'a IssueSummary> {
    lane.issue_keys
        .iter()
        .filter_map(|key| issues_by_key.get(key.as_str()).copied())
        .filter(|issue| {
            board_issue_column(data, issue) == column_index
                && board_issue_matches_filter(issue, search)
        })
        .collect()
}

fn board_issue_matches_filter(issue: &IssueSummary, search: &str) -> bool {
    let search = search.trim();
    if search.is_empty() {
        return true;
    }
    let search = search.to_ascii_lowercase();
    issue.key.to_ascii_lowercase().contains(&search)
        || issue.summary.to_ascii_lowercase().contains(&search)
        || issue.status.to_ascii_lowercase().contains(&search)
        || issue.issue_type.to_ascii_lowercase().contains(&search)
        || displayed_field_matches(issue, "epic_summary", &search)
        || displayed_field_matches(issue, "labels", &search)
        || displayed_field_matches(issue, "dueDate", &search)
        || displayed_field_matches(issue, "priorityName", &search)
        || assignee_matches(issue, &search)
}

fn displayed_field_matches(issue: &IssueSummary, field: &str, search: &str) -> bool {
    issue
        .field_values
        .get(field)
        .is_some_and(|value| value.to_ascii_lowercase().contains(search))
}

fn assignee_matches(issue: &IssueSummary, search: &str) -> bool {
    issue.field_values.get("assignee").is_some_and(|assignee| {
        let assignee = assignee.to_ascii_lowercase();
        let initials = avatar::initials(&assignee).to_ascii_lowercase();
        assignee.contains(search) || initials.contains(search)
    })
}

fn issue_card_lines(
    issue: &IssueSummary,
    selected: bool,
    theme: &Theme,
    width: u16,
    search: &str,
) -> Vec<Line<'static>> {
    let width = width as usize;
    if width < 8 {
        return vec![Line::from(Span::styled(
            truncate_with_ellipsis(&issue.key, width),
            Style::default().fg(theme.accent_fg()),
        ))];
    }

    let border_style = card_border_style(selected, theme);
    let content_style = card_content_style(selected, theme);
    let inner_width = width.saturating_sub(2);
    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled(
        card_top_border(width, selected),
        border_style,
    )));

    for summary_line in wrapped_lines(&issue.summary, inner_width)
        .into_iter()
        .take(3)
    {
        lines.push(card_highlighted_content_line(
            &summary_line,
            search,
            inner_width,
            selected,
            border_style,
            content_style,
            theme,
        ));
    }

    if let Some(epic) = issue.field_values.get("epic_summary") {
        let epic_icon = work_item_key::icon("Epic");
        let epic = format!(
            "{epic_icon} {}",
            truncate_with_ellipsis(
                epic,
                inner_width.saturating_sub(epic_icon.chars().count() + 1)
            )
        );
        lines.push(card_highlighted_content_line(
            &epic,
            search,
            inner_width,
            selected,
            border_style,
            content_style
                .fg(theme.accent_fg())
                .add_modifier(Modifier::BOLD),
            theme,
        ));
    }

    if let Some(labels) = issue.field_values.get("labels") {
        let labels = labels
            .split(", ")
            .filter(|label| !label.is_empty())
            .map(|label| format!("[{label}]"))
            .collect::<Vec<_>>()
            .join("");
        if !labels.is_empty() {
            let labels = truncate_with_ellipsis(&labels, inner_width);
            lines.push(card_highlighted_content_line(
                &labels,
                search,
                inner_width,
                selected,
                border_style,
                content_style,
                theme,
            ));
        }
    }

    if let Some(due_date) = issue.field_values.get("dueDate") {
        let due = format!(" {due_date}");
        let due = truncate_with_ellipsis(&due, inner_width);
        lines.push(card_highlighted_content_line(
            &due,
            search,
            inner_width,
            selected,
            border_style,
            content_style.fg(theme.muted_fg()),
            theme,
        ));
    }

    lines.push(card_bottom_border(issue, width, selected, theme, search));
    lines
}

fn card_highlighted_content_line(
    text: &str,
    search: &str,
    inner_width: usize,
    selected: bool,
    border_style: Style,
    content_style: Style,
    theme: &Theme,
) -> Line<'static> {
    let mut spans = crate::ui::style::highlighted_spans_owned(theme, text, search, content_style);
    apply_background(&mut spans, content_style);
    card_content_spans(spans, inner_width, selected, border_style, content_style)
}

fn card_content_spans(
    spans: Vec<Span<'static>>,
    inner_width: usize,
    selected: bool,
    border_style: Style,
    content_style: Style,
) -> Line<'static> {
    let left = if selected { "║" } else { "│" };
    let right = if selected { "║" } else { "│" };
    let used = spans
        .iter()
        .map(|span| display_width(span.content.as_ref()))
        .sum::<usize>();
    let pad = inner_width.saturating_sub(used);
    let mut line_spans = Vec::with_capacity(spans.len() + 3);
    line_spans.push(Span::styled(left, border_style));
    line_spans.extend(spans);
    line_spans.push(Span::styled(" ".repeat(pad), content_style));
    line_spans.push(Span::styled(right, border_style));
    Line::from(line_spans)
}

fn card_top_border(width: usize, selected: bool) -> String {
    let (left, fill, right) = if selected {
        ('╔', '═', '╗')
    } else {
        ('┌', '─', '┐')
    };
    bordered_line(left, fill, right, width)
}

fn card_bottom_border(
    issue: &IssueSummary,
    width: usize,
    selected: bool,
    theme: &Theme,
    search: &str,
) -> Line<'static> {
    let (left, fill, right) = if selected {
        ("╚", "═", "╝")
    } else {
        ("└", "─", "┘")
    };
    let border_style = card_border_style(selected, theme);
    let content_style = card_content_style(selected, theme);
    let priority_name = issue
        .field_values
        .get("priorityName")
        .map(String::as_str)
        .unwrap_or("");
    let assignee = issue.field_values.get("assignee").map(String::as_str);
    let work_icon = work_item_key::icon(&issue.issue_type);
    let work_key_left_pad = " ";
    let key_segment = format!(" {}", issue.key);
    let work_key_right_pad = " ";
    let priority_left_pad = " ";
    let priority_right_pad = " ";
    let assignee_right_pad = " ";
    let avatar_width = assignee.map_or(0, avatar::bubble_width);
    let assignee_segment_width = if assignee.is_some() {
        avatar_width + display_width(assignee_right_pad)
    } else {
        0
    };
    let priority_width = display_width(priority::icon(priority_name));
    let fixed_width = display_width(left)
        + display_width(work_key_left_pad)
        + display_width(work_icon)
        + display_width(&key_segment)
        + display_width(work_key_right_pad)
        + display_width(priority_left_pad)
        + priority_width
        + display_width(priority_right_pad)
        + assignee_segment_width
        + display_width(right);
    let filler = width.saturating_sub(fixed_width);
    let mut priority_spans = priority::spans(
        theme,
        priority_name,
        "",
        content_style.fg(theme.muted_fg()),
        true,
    );
    apply_background(&mut priority_spans, content_style);

    let mut spans = vec![
        Span::styled(left.to_owned(), border_style),
        Span::styled(work_key_left_pad, content_style),
        Span::styled(
            work_icon.to_owned(),
            content_style.fg(theme.issue_type_fg(&issue.issue_type)),
        ),
    ];
    spans.push(Span::styled(" ", content_style));
    let mut key_spans = crate::ui::style::highlighted_spans_owned(
        theme,
        &issue.key,
        search,
        content_style.fg(theme.accent_fg()),
    );
    apply_background(&mut key_spans, content_style);
    spans.extend(key_spans);
    spans.push(Span::styled(work_key_right_pad, content_style));
    spans.push(Span::styled(fill.repeat(filler), border_style));
    spans.push(Span::styled(priority_left_pad, content_style));
    spans.extend(priority_spans);
    spans.push(Span::styled(priority_right_pad, content_style));
    if let Some(assignee) = assignee {
        let mut avatar_spans = highlighted_avatar_spans(theme, assignee, search, content_style);
        apply_background(&mut avatar_spans, content_style);
        spans.extend(avatar_spans);
        spans.push(Span::styled(assignee_right_pad, content_style));
    }
    spans.push(Span::styled(right.to_owned(), border_style));
    Line::from(spans)
}

fn highlighted_avatar_spans(
    theme: &Theme,
    assignee: &str,
    search: &str,
    content_style: Style,
) -> Vec<Span<'static>> {
    let mut spans = avatar::bubble_only_spans(theme, assignee);
    let search = search.trim().to_ascii_lowercase();
    if search.is_empty() {
        return spans;
    }
    let initials = avatar::initials(assignee).to_ascii_lowercase();
    let assignee = assignee.to_ascii_lowercase();
    if assignee.contains(&search) || initials.contains(&search) {
        for span in &mut spans {
            span.style = span.style.fg(theme.highlight_fg()).bg(theme.highlight_bg());
        }
    } else {
        apply_background(&mut spans, content_style);
    }
    spans
}

fn apply_background(spans: &mut [Span<'static>], base_style: Style) {
    let Some(bg) = base_style.bg else {
        return;
    };
    for span in spans {
        if span.style.bg.is_none() {
            span.style = span.style.bg(bg);
        }
    }
}

fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

fn bordered_line(left: char, fill: char, right: char, width: usize) -> String {
    if width <= 1 {
        return left.to_string();
    }
    format!("{left}{}{right}", fill.to_string().repeat(width - 2))
}

fn card_border_style(selected: bool, theme: &Theme) -> Style {
    let style = Style::default().fg(if selected {
        theme.accent_fg()
    } else {
        theme.border_fg()
    });
    if selected {
        style.bg(theme.selected_bg())
    } else {
        style
    }
}

fn card_content_style(selected: bool, theme: &Theme) -> Style {
    let style = Style::default().fg(if selected {
        theme.selected_fg()
    } else {
        theme.selected_alt_fg()
    });
    if selected {
        style.bg(theme.selected_bg())
    } else {
        style
    }
}

fn wrapped_lines(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let next_len = if current.is_empty() {
            display_width(word)
        } else {
            display_width(&current) + 1 + display_width(word)
        };
        if next_len > width && !current.is_empty() {
            lines.push(current);
            current = word.to_owned();
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

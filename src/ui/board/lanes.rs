use std::collections::BTreeMap;

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::{
    App,
    app::{board_empty_cell_key, board_group_key, board_grouped_lanes, board_issue_column},
    services::jira::{BoardData, BoardSwimlaneSummary, IssueSummary},
    ui::{layout::truncate_with_ellipsis, theme::Theme},
};

use super::card::issue_card_lines;
use super::filter::board_issue_matches_filter;
use super::heading::board_heading_line;
use super::layout::ColumnLayout;

pub(super) struct RenderedBoard {
    pub(super) lines: Vec<Line<'static>>,
    pub(super) layout: Vec<BoardLayoutItem>,
    pub(super) headings: Vec<BoardHeading>,
}

pub(super) struct BoardHeading {
    pub(super) y: usize,
    pub(super) level: usize,
    pub(super) line: Line<'static>,
}

pub(super) struct BoardLayoutItem {
    pub(super) key: String,
    pub(super) y_start: usize,
    pub(super) y_end: usize,
}

pub(super) fn generate_rendered_board(
    app: &App,
    data: &BoardData,
    issues_by_key: &BTreeMap<&str, &IssueSummary>,
    visible_lanes: &[&BoardSwimlaneSummary],
    theme: &Theme,
    columns: &ColumnLayout,
    search: &str,
) -> RenderedBoard {
    let mut rendered = RenderedBoard {
        lines: Vec::new(),
        layout: Vec::new(),
        headings: Vec::new(),
    };
    let selected_key = app.selected_board_issue_key();
    let selected_group = app.selected_board_group();
    let selected_empty = app.selected_board_empty_cell();
    let grouping = app.board_grouping();
    // Headings span the full strip; the horizontal slice trims them to view.
    let header_width = columns.strip_width();
    // Only the ungrouped board shows the column WIP maximum (count/max); when
    // grouped, each lane's count is a slice of the column, not the whole.
    let show_max = !grouping.is_grouped();
    let original_visible_lanes = data
        .swimlanes
        .iter()
        .filter(|lane| !filtered_lane_issue_keys(lane, issues_by_key, search).is_empty())
        .collect::<Vec<_>>();

    if grouping.is_grouped() && original_visible_lanes.len() > 1 {
        let grouped_lanes = board_grouped_lanes(data, grouping);
        for swimlane in original_visible_lanes {
            let swimlane_keys = filtered_lane_issue_keys(swimlane, issues_by_key, search);
            let swimlane_heading = board_heading_line(
                &swimlane.name,
                swimlane_keys.len(),
                false,
                false,
                theme,
                header_width,
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
                    header_width,
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
                    selected_empty,
                    theme,
                    columns,
                    show_max,
                    search,
                );
            }
        }
        return rendered;
    }

    for lane in visible_lanes {
        let lane_issues = filtered_lane_issue_keys(lane, issues_by_key, search);
        let show_header = grouping.is_grouped() || visible_lanes.len() > 1 || lane.name != "Issues";
        if show_header {
            let collapsed = app.is_board_group_collapsed(&lane.name);
            let selected = selected_group == Some(lane.name.as_str());
            let header_line = board_heading_line(
                &lane.name,
                lane_issues.len(),
                collapsed,
                selected,
                theme,
                header_width,
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
            selected_empty,
            theme,
            columns,
            show_max,
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
    selected_empty: Option<(&str, usize)>,
    theme: &Theme,
    columns: &ColumnLayout,
    show_max: bool,
    search: &str,
) {
    // Is column `c_idx` the focused empty cell of this lane?
    let empty_selected = |c_idx: usize| selected_empty == Some((lane.name.as_str(), c_idx));
    // Render every column at full width into one strip; the caller slices it to
    // the viewport, so partial edge columns (the "peek") come for free.
    //
    // Bucket the lane's issues by column in a single pass. Doing it once (rather
    // than filtering the whole lane per column, twice) keeps redraws cheap even
    // at the 60fps animation cadence.
    let mut column_issues: Vec<Vec<&IssueSummary>> = vec![Vec::new(); data.columns.len()];
    for key in &lane.issue_keys {
        if let Some(issue) = issues_by_key.get(key.as_str()).copied()
            && board_issue_matches_filter(issue, search)
        {
            let c = board_issue_column(data, issue);
            if let Some(bucket) = column_issues.get_mut(c) {
                bucket.push(issue);
            }
        }
    }

    let mut max_inner_len = 1;
    let mut columns_card_lines = Vec::new();
    for (c_idx, col_issues) in column_issues.iter().enumerate() {
        let column_width = columns.rects[c_idx].width;
        let mut col_card_lines = Vec::new();
        let mut card_positions = Vec::new();
        let mut current_local_y = 0;
        for issue in col_issues {
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
            // Brighten the placeholder when this empty column is focused so the
            // selection is visible even though there's no card.
            let style = if empty_selected(c_idx) {
                Style::default()
                    .fg(theme.accent_fg())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.muted_fg())
            };
            col_card_lines.push(Line::from(Span::styled(
                format!("{no_issues_text}{}", " ".repeat(pad)),
                style,
            )));
        }
        max_inner_len = max_inner_len.max(col_card_lines.len());
        columns_card_lines.push((c_idx, col_card_lines, card_positions));
    }

    let block_height = max_inner_len + 2;
    let columns_y_start = rendered.lines.len();
    let header_y_start = if show_header {
        columns_y_start.saturating_sub(1)
    } else {
        columns_y_start
    };
    for (c_idx, _, card_positions) in columns_card_lines.iter() {
        if card_positions.is_empty() {
            // Empty column: anchor the focusable region to the top of the block
            // (the header through the "No issues" line). Spanning the whole
            // block — which can be far taller than the viewport when a sibling
            // column is long — makes the vertical auto-scroll oscillate.
            rendered.layout.push(BoardLayoutItem {
                key: board_empty_cell_key(&lane.name, *c_idx),
                y_start: header_y_start,
                y_end: columns_y_start + 2,
            });
            continue;
        }
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
    for (c_idx, col_card_lines, _) in columns_card_lines.into_iter() {
        let column_width = columns.rects[c_idx].width;
        let col_issues = &column_issues[c_idx];
        let column_selected = empty_selected(c_idx)
            || selected_key.is_some_and(|key| col_issues.iter().any(|issue| issue.key == key));

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
        // Keep the column title within the column width: truncate the name but
        // always preserve the trailing checkmark/count, otherwise an overlong
        // title overflows and breaks the alignment of every column to its right.
        // On the ungrouped board, show `count/max` when the column has a WIP
        // maximum so it reads like Jira's column constraint.
        let count_text = match column.max {
            Some(max) if show_max => format!("{count}/{max}"),
            _ => count.to_string(),
        };
        let inner = column_width.saturating_sub(2) as usize;
        let suffix = format!("{checkmark} {count_text} ");
        let name_budget = inner.saturating_sub(1 + suffix.chars().count());
        let name = truncate_with_ellipsis(&column.name.to_uppercase(), name_budget);
        let title_text = format!(" {name}{suffix}");
        let title_len = title_text.chars().count();
        let fill_count = inner.saturating_sub(title_len);
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

    // Transpose the per-column line lists into joined rows. Move each cell's
    // spans out (rather than cloning) since every cell is consumed exactly once.
    if col_lines_list.is_empty() {
        return;
    }
    for row_index in 0..block_height {
        let mut joined_spans = Vec::new();
        for column_lines in col_lines_list.iter_mut() {
            joined_spans.append(&mut column_lines[row_index].spans);
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

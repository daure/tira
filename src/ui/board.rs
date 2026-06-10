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
    App, KeyBindings,
    app::{board_empty_cell_key, board_group_key, board_grouped_lanes, board_issue_column},
    components::{
        generic::{avatar, filter, label, priority, tree::fuzzy_matches},
        jira::work_item_key,
    },
    services::jira::{BoardData, BoardSwimlaneSummary, IssueSummary},
    ui::{
        layout::truncate_spans_with_ellipsis,
        layout::truncate_with_ellipsis,
        scrollbar,
        theme::{Theme, prefers_plain_icons},
    },
};

const NERD_COLLAPSED_ICON: &str = "";
const NERD_EXPANDED_ICON: &str = "";

/// Minimum readable width for a board column. Below this we start scrolling
/// horizontally instead of squeezing more columns in.
const MIN_COL_WIDTH: u16 = 34;
/// Upper bound so columns don't grow absurdly wide when only a few are shown.
const MAX_COL_WIDTH: u16 = 52;
/// Sliver of the neighbouring column left visible at rest on each side that has
/// more columns, so the board reads as "more this way". Reserved on both sides
/// so the column width stays constant as the strip glides (no resizing mid
/// scroll); the slivers simply widen/narrow as the horizontal offset animates.
const PEEK_WIDTH: u16 = 8;

/// The run of board columns that sit fully on screen at rest. Drives the scroll
/// target (so the selected card stays visible) and the persisted column offset.
#[derive(Clone, Copy)]
struct ColumnWindow {
    /// Index of the leftmost fully-visible column.
    start: usize,
}

impl ColumnWindow {
    fn more_left(&self) -> bool {
        self.start > 0
    }
}

/// All horizontal layout decisions for one frame of the board: the width of
/// every column and which columns sit fully on screen at rest (the window).
///
/// The board is drawn as a single strip [`strip_width`](Self::strip_width) wide
/// (every column, full width) and then sliced to the viewport at an animated
/// cell offset. Rendering the whole strip — rather than only the visible
/// columns — is what lets the horizontal scroll glide smoothly: partial columns
/// at the edges (the "peek") fall out of the slice for free, at any sub-column
/// offset, instead of being special-cased.
///
/// This is a *pure* value computed by [`ColumnLayout::compute`] so the
/// non-trivial geometry can be unit-tested without a terminal.
struct ColumnLayout {
    /// Width/position of every column, indexed by board column index. Uniform
    /// width while scrolling; equal-ratio fill when everything fits.
    rects: Vec<Rect>,
    window: ColumnWindow,
    /// Whether there are more columns than fit (drives the horizontal scrollbar
    /// and whether a non-zero scroll offset is possible).
    scrolling: bool,
}

impl ColumnLayout {
    /// Columns that fit at the minimum readable width. At least one, so a very
    /// narrow terminal still shows a single (clipped) column rather than panic.
    fn max_visible(board_width: u16) -> usize {
        (board_width.max(1) / MIN_COL_WIDTH).max(1) as usize
    }

    /// Whether the board must scroll horizontally at this width. Cheap enough to
    /// call before [`compute`](Self::compute) so the caller can reserve a row
    /// for the scrollbar before the (height-reduced) area is handed back in.
    fn will_scroll(board_width: u16, column_count: usize) -> bool {
        column_count.max(1) > Self::max_visible(board_width)
    }

    /// Derive the full layout. `selected_col` keeps the window scrolled so the
    /// selected card stays visible (mirroring the vertical auto-scroll), and
    /// `stored_scroll` is the previous column offset so the view is stable
    /// across frames when the selection doesn't move.
    fn compute(
        board_area: Rect,
        column_count: usize,
        selected_col: Option<usize>,
        stored_scroll: usize,
    ) -> Self {
        let column_count = column_count.max(1);
        let board_width = board_area.width.max(1);
        let max_visible = Self::max_visible(board_width);

        if column_count <= max_visible {
            // Everything fits: equal-ratio fill, one window over all columns.
            let rects = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(
                    (0..column_count)
                        .map(|_| Constraint::Ratio(1, column_count as u32))
                        .collect::<Vec<_>>(),
                )
                .split(board_area)
                .to_vec();
            return Self {
                rects,
                window: ColumnWindow { start: 0 },
                scrolling: false,
            };
        }

        let visible_count = max_visible.min(column_count).max(1);

        // Slide the window to keep the selected column inside it.
        let mut start = stored_scroll.min(column_count - visible_count);
        if let Some(sel) = selected_col {
            let sel = sel.min(column_count - 1);
            if sel < start {
                start = sel;
            } else if sel >= start + visible_count {
                start = sel + 1 - visible_count;
            }
        }
        let window = ColumnWindow { start };

        // Reserve a peek on *both* sides up front so the column width is the
        // same at every scroll position (columns must not resize as you glide).
        // The scroll offset later decides which slivers are actually revealed.
        let reserve = 2 * PEEK_WIDTH;
        let full_area = board_width.saturating_sub(reserve);
        let col_width = (full_area / visible_count as u16).clamp(MIN_COL_WIDTH, MAX_COL_WIDTH);
        let rects = (0..column_count)
            .map(|_| Rect {
                x: board_area.x,
                y: board_area.y,
                width: col_width,
                height: board_area.height,
            })
            .collect::<Vec<_>>();

        Self {
            rects,
            window,
            scrolling: true,
        }
    }

    /// Total width of the rendered strip (sum of all column widths).
    fn strip_width(&self) -> u16 {
        self.rects.iter().map(|r| r.width).sum()
    }

    /// Cell offset of column `idx`'s left edge within the strip.
    fn col_left(&self, idx: usize) -> u16 {
        self.rects[..idx.min(self.rects.len())]
            .iter()
            .map(|r| r.width)
            .sum()
    }

    /// The horizontal offset (in cells) the strip should rest at: the window's
    /// left column, pulled back by a peek so the previous column shows a sliver
    /// when there's more to the left. Clamped so the slice never runs past the
    /// strip. This is the *target* the animator glides toward.
    fn target_offset(&self, board_width: u16) -> u16 {
        if !self.scrolling {
            return 0;
        }
        let left = self.col_left(self.window.start);
        let raw = left.saturating_sub(if self.window.more_left() {
            PEEK_WIDTH
        } else {
            0
        });
        let max_off = self.strip_width().saturating_sub(board_width);
        raw.min(max_off)
    }
}

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &App, keybindings: &KeyBindings) {
    let theme = app.theme();
    let [top_area, content_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .areas(area);
    let group_width = (app.board_grouping().label().len() as u16 + 9).max(16);
    let details_text = details_trigger_text(app);
    let details_width = details_text.chars().count() as u16 + 2;
    let [filter_area, details_area, group_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(details_width),
            Constraint::Length(group_width),
        ])
        .areas(top_area);
    render_filter(frame, filter_area, app, keybindings);
    render_details_trigger(frame, details_area, app, &details_text);
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
    // Reserve a bottom row for the horizontal scrollbar only when scrolling.
    // Checked before computing the layout so the layout sees the reduced height.
    let scrolling = ColumnLayout::will_scroll(board_area.width, column_count);
    let (board_area, hscroll_area, scrollbar_area) = if scrolling && board_area.height > 1 {
        let [content, _bar] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .areas(board_area);
        // The horizontal scrollbar spans the full board width (including the
        // vertical scrollbar's column); the vertical scrollbar stops one row
        // above it so the two never collide in the bottom-right corner.
        let hbar = Rect {
            x: content_area.x,
            y: board_area.y + board_area.height - 1,
            width: content_area.width,
            height: 1,
        };
        let vbar = Rect {
            height: scrollbar_area.height.saturating_sub(1),
            ..scrollbar_area
        };
        (content, Some(hbar), vbar)
    } else {
        (board_area, None, scrollbar_area)
    };

    // The column to keep on screen. A focused card → its column; a focused
    // empty column → that column. A focused group/swimlane header → `None`: keep
    // the current horizontal offset (the header label is rendered sticky on the
    // left, so it stays visible without panning the board back).
    let selected_col = if let Some((_, column)) = app.selected_board_empty_cell() {
        Some(column.min(column_count - 1))
    } else {
        app.selected_board_issue_key().and_then(|key| {
            data.issues
                .iter()
                .find(|issue| issue.key == key)
                .map(|issue| board_issue_column(data, issue).min(column_count - 1))
        })
    };
    let columns = ColumnLayout::compute(
        board_area,
        column_count,
        selected_col,
        app.board().col_scroll_offset.get(),
    );
    app.board().col_scroll_offset.set(columns.window.start);
    app.board()
        .column_widths
        .replace(columns.rects.iter().map(|r| r.width as usize).collect());

    let rendered = generate_rendered_board(
        app,
        data,
        &issues_by_key,
        &visible_lanes,
        theme,
        &columns,
        search,
    );

    let selected_key_or_group = app.selected_board_raw_key().map(String::from);

    let mut sel_y_start = 0;
    let mut sel_y_end = 0;
    if let Some(target_key) = selected_key_or_group
        && let Some(item) = rendered.layout.iter().find(|item| item.key == *target_key)
    {
        sel_y_start = item.y_start;
        sel_y_end = item.y_end;
    }

    let total_lines = rendered.lines.len();
    let viewport_height = board_area.height as usize;

    // Vertical: while the user is wheel-scrolling (manual), show their offset;
    // otherwise keep the selection in view. Either way glide toward the target.
    let max_v = total_lines.saturating_sub(viewport_height);
    let mut scroll_offset = app.board().scroll_offset.get();
    if !app.board().manual_v_scroll.get() {
        if sel_y_start < scroll_offset {
            scroll_offset = sel_y_start;
        } else if sel_y_end > scroll_offset + viewport_height {
            scroll_offset = sel_y_end.saturating_sub(viewport_height);
        }
    }
    scroll_offset = scroll_offset.min(max_v);
    app.board().scroll_offset.set(scroll_offset);
    let v_anim = &app.board().v_scroll;
    v_anim.set_target(scroll_offset as f64);
    let v_offset = (v_anim.current().round() as usize).min(max_v);

    // Horizontal: glide the strip's cell offset toward the target so columns
    // slide smoothly; partial columns at the edges are the "peek". The target is
    // the user's manual pan while wheel-scrolling, else the selection-following
    // snap position.
    let strip_width = columns.strip_width();
    let max_h = strip_width.saturating_sub(board_area.width);
    let h_target = if app.board().manual_h_scroll.get() {
        let manual = app.board().manual_h_offset.get().min(max_h);
        app.board().manual_h_offset.set(manual);
        manual
    } else {
        columns.target_offset(board_area.width)
    };
    let h_anim = &app.board().h_scroll;
    h_anim.set_target(f64::from(h_target));
    let h_offset = (h_anim.current().round() as u16).min(max_h);

    let mut visible_lines = rendered
        .lines
        .iter()
        .skip(v_offset)
        .take(viewport_height)
        .cloned()
        .collect::<Vec<_>>();

    // Group/swimlane headers (heading levels 0 and 1) are pinned to the left
    // edge ("horizontally sticky") so their label stays readable when the board
    // is scrolled right; column-title rows (level 2) and cards scroll normally.
    let header_levels: std::collections::HashMap<usize, usize> = rendered
        .headings
        .iter()
        .map(|heading| (heading.y, heading.level))
        .collect();
    let mut sticky_left = (0..visible_lines.len())
        .map(|i| {
            header_levels
                .get(&(v_offset + i))
                .is_some_and(|level| *level < 2)
        })
        .collect::<Vec<_>>();

    for (index, (level, sticky_heading)) in sticky_headings(&rendered.headings, v_offset)
        .into_iter()
        .take(viewport_height)
        .enumerate()
    {
        if let Some(line) = visible_lines.get_mut(index) {
            *line = sticky_heading;
            sticky_left[index] = level < 2;
        }
    }

    // Slice every visible line to the horizontal viewport. Sticky headers slice
    // from the start so their label stays at the left; everything else slices at
    // the animated horizontal offset so columns line up.
    if columns.scrolling {
        for (i, line) in visible_lines.iter_mut().enumerate() {
            let start = if sticky_left[i] { 0 } else { h_offset as usize };
            *line = slice_line(std::mem::take(line), start, board_area.width as usize);
        }
    }

    frame.render_widget(Paragraph::new(visible_lines), board_area);

    if total_lines > viewport_height {
        scrollbar::render_range(
            frame,
            scrollbar_area,
            total_lines,
            v_offset..v_offset + viewport_height,
            theme,
        );
    }

    if let Some(bar) = hscroll_area {
        // Cell-based thumb so it tracks the actual (animated, possibly manual)
        // horizontal offset rather than snapped column boundaries.
        let viewport = (board_area.width as usize).min(strip_width as usize);
        let start = h_offset as usize;
        scrollbar::render_range_horizontal(
            frame,
            bar,
            strip_width as usize,
            start..start + viewport,
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

fn board_heading_line(
    name: &str,
    count: usize,
    collapsed: bool,
    selected: bool,
    theme: &Theme,
    header_width: u16,
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
    let filler_len = (header_width as usize).saturating_sub(text_len);
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

/// Returns the stacked sticky headings (with their level) for the row at the
/// top of the viewport: the current swimlane, group and column-title context.
fn sticky_headings(headings: &[BoardHeading], scroll_offset: usize) -> Vec<(usize, Line<'static>)> {
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
            sticky.push((level, heading.line.clone()));
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

fn details_trigger_text(app: &App) -> String {
    match app
        .board()
        .data()
        .and_then(|data| data.sprint.as_ref())
        .and_then(|sprint| sprint.days_left_label())
    {
        Some(days_left) => format!("details: {days_left}"),
        None => String::from("details"),
    }
}

fn render_details_trigger(frame: &mut Frame<'_>, area: Rect, app: &App, text: &str) {
    let theme = app.theme();
    let (hotkey, rest) = text.split_at(1);
    let line = Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(
            hotkey.to_owned(),
            Style::default()
                .fg(theme.accent_fg())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(rest.to_owned(), Style::default().fg(theme.muted_fg())),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_group_trigger(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = app.theme();
    let label = app.board_grouping().label();
    let text = Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled("g", Style::default().fg(theme.muted_fg())),
        Span::styled(
            "r",
            Style::default()
                .fg(theme.accent_fg())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("oup: ", Style::default().fg(theme.muted_fg())),
        Span::styled(
            label.to_owned(),
            Style::default().fg(theme.selected_alt_fg()),
        ),
    ]);
    frame.render_widget(Paragraph::new(text), area);
}

fn board_issue_matches_filter(issue: &IssueSummary, search: &str) -> bool {
    let search = search.trim();
    if search.is_empty() {
        return true;
    }
    fuzzy_matches(&issue.key, search)
        || fuzzy_matches(&issue.summary, search)
        || fuzzy_matches(&issue.status, search)
        || fuzzy_matches(&issue.issue_type, search)
        || displayed_field_matches(issue, "epic_summary", search)
        || displayed_field_matches(issue, "labels", search)
        || displayed_field_matches(issue, "dueDate", search)
        || displayed_field_matches(issue, "priorityName", search)
        || assignee_matches(issue, search)
}

fn displayed_field_matches(issue: &IssueSummary, field: &str, search: &str) -> bool {
    issue
        .field_values
        .get(field)
        .is_some_and(|value| fuzzy_matches(value, search))
}

fn assignee_matches(issue: &IssueSummary, search: &str) -> bool {
    issue.field_values.get("assignee").is_some_and(|assignee| {
        let initials = avatar::initials(assignee);
        fuzzy_matches(assignee, search) || fuzzy_matches(&initials, search)
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

    if let Some(labels) = issue.field_values.get("labels")
        && label::has_labels(labels)
    {
        let mut spans = truncate_spans_with_ellipsis(
            label::spans(theme, labels, search, content_style),
            inner_width,
        );
        apply_background(&mut spans, content_style);
        lines.push(card_content_spans(
            spans,
            inner_width,
            selected,
            border_style,
            content_style,
        ));
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

/// Returns the sub-slice of a line covering display columns
/// `[start, start + width)`, preserving span styles and adding no ellipsis.
/// This is how the full board strip is trimmed to the horizontal viewport;
/// fractional column boundaries (mid-glide) simply land inside a span.
fn slice_line(line: Line<'static>, start: usize, width: usize) -> Line<'static> {
    clip_head(clip_tail(line, start), width)
}

/// Keeps the leading `width` display columns of a line.
fn clip_head(line: Line<'static>, width: usize) -> Line<'static> {
    let mut out = Vec::new();
    let mut used = 0usize;
    for span in line.spans {
        if used >= width {
            break;
        }
        let span_width = display_width(span.content.as_ref());
        if used + span_width <= width {
            used += span_width;
            out.push(span);
        } else {
            let remaining = width - used;
            let mut partial = String::new();
            let mut acc = 0usize;
            for ch in span.content.chars() {
                let ch_width = display_width(ch.encode_utf8(&mut [0; 4]));
                if acc + ch_width > remaining {
                    break;
                }
                acc += ch_width;
                partial.push(ch);
            }
            if !partial.is_empty() {
                out.push(Span::styled(partial, span.style));
            }
            break;
        }
    }
    Line::from(out)
}

/// Drops the leading `skip` display columns from a line, keeping the rest.
fn clip_tail(line: Line<'static>, skip: usize) -> Line<'static> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    for span in line.spans {
        let span_width = display_width(span.content.as_ref());
        let span_end = pos + span_width;
        if span_end <= skip {
            pos = span_end;
            continue;
        }
        if pos >= skip {
            out.push(span);
        } else {
            let drop_cols = skip - pos;
            let mut acc = 0usize;
            let mut kept = String::new();
            for ch in span.content.chars() {
                let ch_width = display_width(ch.encode_utf8(&mut [0; 4]));
                if acc < drop_cols {
                    acc += ch_width;
                    continue;
                }
                kept.push(ch);
            }
            if !kept.is_empty() {
                out.push(Span::styled(kept, span.style));
            }
        }
        pos = span_end;
    }
    Line::from(out)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn area(width: u16) -> Rect {
        Rect {
            x: 0,
            y: 0,
            width,
            height: 40,
        }
    }

    fn issue(key: &str, summary: &str) -> IssueSummary {
        IssueSummary {
            key: key.to_owned(),
            summary: summary.to_owned(),
            status: "To Do".to_owned(),
            issue_type: "Task".to_owned(),
            parent_key: None,
            has_children: false,
            field_values: BTreeMap::new(),
        }
    }

    #[test]
    fn filter_matches_non_contiguous_subsequence() {
        let issue = issue("KAN-1", "Improve navigation flow");
        // "imnav" is not a substring but is a fuzzy subsequence of the summary.
        assert!(board_issue_matches_filter(&issue, "imnav"));
        // Characters out of order never match.
        assert!(!board_issue_matches_filter(&issue, "vanimprove"));
    }

    #[test]
    fn filter_matches_assignee_initials() {
        let mut issue = issue("KAN-2", "Unrelated summary");
        issue
            .field_values
            .insert("assignee".to_owned(), "Marlo Vlietstra".to_owned());
        // The avatar shows "MV"; searching the initials still matches.
        assert!(board_issue_matches_filter(&issue, "mv"));
        // The full name fuzzy-matches too.
        assert!(board_issue_matches_filter(&issue, "marlo"));
    }

    #[test]
    fn empty_filter_matches_everything() {
        let issue = issue("KAN-3", "anything");
        assert!(board_issue_matches_filter(&issue, ""));
        assert!(board_issue_matches_filter(&issue, "   "));
    }

    #[test]
    fn fits_without_scrolling_keeps_one_window_and_no_offset() {
        // 4 columns at 200 cells: ~50 each, comfortably above MIN.
        let layout = ColumnLayout::compute(area(200), 4, Some(0), 0);
        assert!(!ColumnLayout::will_scroll(200, 4));
        assert!(!layout.scrolling);
        assert_eq!(layout.window.start, 0);
        assert_eq!(layout.rects.len(), 4);
        // Ratio fill spans the whole board; nothing to scroll.
        assert_eq!(layout.strip_width(), 200);
        assert_eq!(layout.target_offset(200), 0);
    }

    #[test]
    fn scrolls_when_columns_exceed_min_width() {
        // 6 columns at 150 cells: only 150/34 = 4 fit.
        assert!(ColumnLayout::will_scroll(150, 6));
        let layout = ColumnLayout::compute(area(150), 6, Some(0), 0);
        assert!(layout.scrolling);
        // Selection at the far left: window pinned to start, strip rests at 0.
        assert_eq!(layout.window.start, 0);
        assert!(layout.strip_width() > 150, "strip wider than the viewport");
        assert_eq!(layout.target_offset(150), 0);
    }

    #[test]
    fn window_follows_selection_and_reveals_a_left_peek_in_the_middle() {
        // Selecting column 4 (with 4 visible) scrolls to window starting at 1.
        let layout = ColumnLayout::compute(area(150), 6, Some(4), 0);
        assert_eq!(layout.window.start, 1);
        // The strip rests pulled back by one peek so column 0 shows a sliver.
        let col_width = layout.rects[0].width;
        assert_eq!(layout.target_offset(150), col_width - PEEK_WIDTH);
    }

    #[test]
    fn far_right_selection_clamps_to_the_end_of_the_strip() {
        let layout = ColumnLayout::compute(area(150), 6, Some(5), 0);
        // Window scrolled so the last column is visible.
        assert_eq!(layout.window.start, 2);
        let max_off = layout.strip_width() - 150;
        assert_eq!(layout.target_offset(150), max_off);
    }

    #[test]
    fn target_offset_always_stays_within_the_strip() {
        for sel in 0..6 {
            let layout = ColumnLayout::compute(area(150), 6, Some(sel), 0);
            let max_off = layout.strip_width().saturating_sub(150);
            assert!(
                layout.target_offset(150) <= max_off,
                "selection {sel} offset out of bounds"
            );
        }
    }

    #[test]
    fn slice_line_returns_the_requested_window() {
        let line = Line::from(vec![Span::raw("abcdefghij".to_owned())]);
        let sliced = slice_line(line, 3, 4);
        let text: String = sliced.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "defg");
    }

    #[test]
    fn slice_line_preserves_per_span_styles_across_the_cut() {
        let line = Line::from(vec![
            Span::raw("ab".to_owned()),
            Span::raw("cdef".to_owned()),
        ]);
        let sliced = slice_line(line, 1, 3);
        let text: String = sliced.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "bcd");
    }
}

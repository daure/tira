use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use std::collections::BTreeMap;
use unicode_width::UnicodeWidthStr;

use crate::{
    App, KeyBindings,
    app::TimelineState,
    components::{
        generic::{
            filter,
            tree::{TreeRow, TreeState},
        },
        jira::work_item_key,
    },
    domain::{
        date::{civil_from_days, days_from_civil, next_month, today_days},
        models::{SprintState, TimelineEpicStats, TimelineSprint},
    },
    ui::{
        chrome,
        layout::truncate_with_ellipsis,
        scrollbar, style,
        theme::{Theme, prefers_plain_icons},
    },
};

/// Header rows above the body: month names and sprint pills. The today marker
/// is a full-height line rather than its own row.
const HEADER_ROWS: u16 = 2;
/// Width of the right-aligned epic completion-percentage column.
const PCT_COL_WIDTH: usize = 4;
/// Space between the epic percentage column and the timeline canvas.
const PCT_TRAILING_GAP: usize = 1;
/// Rolling axis window: how far back and forward from today it spans.
const WINDOW_BACK_DAYS: i64 = 180;
const WINDOW_FWD_DAYS: i64 = 365;
/// Floor on cells-per-day so sprint pills stay legible; above this the axis
/// stretches to fill the viewport.
const MIN_PX_PER_DAY: f64 = 1.0;
/// Glyph for the today marker.
const TODAY_LINE: char = '┊';
const POWERLINE_ROUND_LEFT: char = '\u{e0b6}';
const POWERLINE_ROUND_RIGHT: char = '\u{e0b4}';
const POWERLINE_ARROW_RIGHT: char = '\u{e0b0}';
const TIMELINE_BAR: char = '━';
const NERD_COLLAPSED_ICON: &str = "\u{f460}";
const NERD_EXPANDED_ICON: &str = "\u{f47c}";

/// One drawn terminal cell on the scrollable canvas.
#[derive(Clone, Copy)]
struct GridCell {
    ch: char,
    style: Style,
}

impl GridCell {
    fn blank() -> Self {
        Self {
            ch: ' ',
            style: Style::default(),
        }
    }
}

/// Maps day-numbers to canvas columns. The canvas is one continuous strip whose
/// width derives from the date window; months, sprints, today and bars are all
/// drawn by converting their dates through `x`.
struct Axis {
    start_day: i64,
    end_day: i64,
    px_per_day: f64,
    width: usize,
}

impl Axis {
    fn new(start_day: i64, end_day: i64, viewport: usize) -> Self {
        let days = (end_day - start_day).max(1);
        let px_per_day = (viewport as f64 / days as f64).max(MIN_PX_PER_DAY);
        let width = (days as f64 * px_per_day).ceil() as usize;
        Self {
            start_day,
            end_day,
            px_per_day,
            width,
        }
    }

    fn x(&self, day: i64) -> i32 {
        (((day - self.start_day) as f64) * self.px_per_day).round() as i32
    }

    fn clamp_x(&self, x: i32) -> usize {
        x.clamp(0, self.width as i32) as usize
    }
}

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &App, keybindings: &KeyBindings) {
    let theme = app.theme();
    let [toolbar_area, content_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .areas(area);
    render_toolbar(frame, toolbar_area, app, keybindings, theme);

    let state = app.timeline();
    let Some(data) = state.data() else {
        render_message(frame, content_area, theme, empty_message(state));
        return;
    };
    if data.epics.is_empty() {
        render_message(frame, content_area, theme, "No epics on this board.");
        return;
    }

    let tree = state.tree();
    let rows = tree.rows();
    if rows.is_empty() {
        render_message(
            frame,
            content_area,
            theme,
            "No timeline items match the search.",
        );
        return;
    }
    let label_width = (content_area.width / 2).clamp(28, 52);
    let [label_area, grid_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(label_width), Constraint::Min(1)])
        .areas(content_area);

    // Reserve the rightmost grid column for the vertical scrollbar plus a gap
    // column before it, matching the List/Board tabs and keeping bars off the
    // scrollbar.
    let grid_main_width = grid_area.width.saturating_sub(2);
    let today = today_days();
    let axis = Axis::new(
        today - WINDOW_BACK_DAYS,
        today + WINDOW_FWD_DAYS,
        grid_main_width as usize,
    );
    let today_x = axis.x(today);
    let scrolls_h = axis.width > grid_main_width as usize;
    let body_height = grid_area
        .height
        .saturating_sub(HEADER_ROWS + u16::from(scrolls_h)) as usize;
    let range = tree.visible_range(body_height);
    let item_spans = state.item_spans();
    let epic_stats = state.epic_stats();

    let selected_span = selected_timeline_span(&rows, tree, item_spans);
    let selected_range = selected_span.and_then(|span| axis_range(&axis, span));
    let h_offset = state.resolve_h_offset(
        grid_main_width,
        axis.width.min(u16::MAX as usize) as u16,
        today_x,
        selected_range,
    );

    render_label_header(frame, label_area, theme);
    render_grid_header(
        frame,
        grid_area,
        grid_main_width,
        &axis,
        &data.sprints,
        selected_span,
        today_x,
        h_offset,
        theme,
    );

    let body_y = grid_area.y + HEADER_ROWS;
    let mut label_lines = Vec::new();
    let mut grid_lines = Vec::new();
    for row_index in range.clone() {
        let Some(row) = rows.get(row_index) else {
            break;
        };
        let item = &tree.items()[row.item_index];
        let selected = row_index == tree.selected_row();
        let stats = epic_stats.get(item.id.as_str()).copied();
        label_lines.push(label_line(
            theme,
            row,
            item,
            label_width as usize,
            stats,
            selected,
            state.filter(),
        ));

        let is_epic = item.kind == "Epic";
        let cells = bar_row(
            &axis,
            is_epic,
            &item.kind,
            item_spans.get(item.id.as_str()).copied(),
            selected,
            today_x,
            theme,
        );
        let mut line = slice_line(&cells, h_offset, grid_main_width);
        if selected {
            line = line.style(Style::default().bg(theme.selected_bg()));
        }
        grid_lines.push(line);
    }

    frame.render_widget(
        Paragraph::new(label_lines),
        Rect {
            y: body_y,
            height: body_height as u16,
            ..label_area
        },
    );
    frame.render_widget(
        Paragraph::new(grid_lines),
        Rect {
            x: grid_area.x,
            y: body_y,
            width: grid_main_width,
            height: body_height as u16,
        },
    );

    let vbar = Rect {
        x: grid_area.x + grid_area.width - 1,
        y: body_y,
        width: 1,
        height: body_height as u16,
    };
    scrollbar::render_range(frame, vbar, rows.len(), range, theme);

    if scrolls_h {
        let hbar = Rect {
            x: grid_area.x,
            y: body_y + body_height as u16,
            width: grid_area.width,
            height: 1,
        };
        let start = h_offset as usize;
        scrollbar::render_range_horizontal(
            frame,
            hbar,
            axis.width,
            start..start + grid_main_width as usize,
            theme,
        );
    }
}

fn render_toolbar(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    keybindings: &KeyBindings,
    theme: &Theme,
) {
    let [filter_area, _spacer_area, hint_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(24),
            Constraint::Min(1),
            Constraint::Length(keybindings.shortcuts_hint_width()),
        ])
        .areas(area);
    render_filter(frame, filter_area, app);
    frame.render_widget(chrome::shortcuts_hint(keybindings, theme), hint_area);
}

fn render_filter(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let [icon_area, text_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .areas(area);

    frame.render_widget(
        filter::render_icon(app.timeline_filter_state(), app.theme()),
        icon_area,
    );
    frame.render_widget(
        filter::render_text(app.timeline_filter_state(), app.theme()),
        text_area,
    );

    if app.is_timeline_filter_focused() {
        let cursor_x = text_area.x + app.timeline_filter_cursor() as u16;
        frame.set_cursor_position(ratatui::layout::Position::new(cursor_x, text_area.y));
    }
}

fn render_message(frame: &mut Frame<'_>, area: Rect, theme: &Theme, message: &str) {
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            truncate_with_ellipsis(message, area.width as usize),
            Style::default().fg(theme.muted_fg()),
        ))),
        area,
    );
}

fn empty_message(state: &TimelineState) -> &'static str {
    if state.is_loading() {
        "Loading timeline…"
    } else if state.error().is_some() {
        "Timeline could not be loaded; see notifications."
    } else {
        "Timeline has not loaded yet."
    }
}

fn render_label_header(frame: &mut Frame<'_>, label_area: Rect, theme: &Theme) {
    let area = Rect {
        x: label_area.x.saturating_add(2),
        width: label_area.width.saturating_sub(2),
        height: 1,
        ..label_area
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Work",
            Style::default()
                .fg(theme.subtle_fg())
                .add_modifier(Modifier::BOLD),
        ))),
        area,
    );
}

#[allow(clippy::too_many_arguments)]
fn render_grid_header(
    frame: &mut Frame<'_>,
    grid_area: Rect,
    grid_main_width: u16,
    axis: &Axis,
    sprints: &[TimelineSprint],
    selected_span: Option<(i64, i64)>,
    today_x: i32,
    h_offset: u16,
    theme: &Theme,
) {
    let month = month_row(axis, selected_span, theme);
    let mut sprint = sprint_row(axis, sprints, selected_span, theme);
    // The today marker is a thin line through blank cells, so it never
    // overwrites a month label, sprint pill, or range bar.
    overlay_today_line(&mut sprint, today_x, theme);
    for (index, cells) in [month, sprint].iter().enumerate() {
        frame.render_widget(
            Paragraph::new(slice_line(cells, h_offset, grid_main_width)),
            Rect {
                x: grid_area.x,
                y: grid_area.y + index as u16,
                width: grid_main_width,
                height: 1,
            },
        );
    }
}

fn overlay_today_line(cells: &mut [GridCell], today_x: i32, theme: &Theme) {
    if today_x < 0 {
        return;
    }
    let x = today_x as usize;
    if cells.get(x).is_some_and(|cell| cell.ch == ' ') {
        put_cell(
            cells,
            x,
            TODAY_LINE,
            Style::default().fg(theme.warning_fg()),
        );
    }
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

fn month_row(axis: &Axis, selected_span: Option<(i64, i64)>, theme: &Theme) -> Vec<GridCell> {
    let mut cells = blank_row(axis.width);
    let (mut year, mut month, _) = civil_from_days(axis.start_day);
    loop {
        let month_start = days_from_civil(year, month, 1);
        if month_start >= axis.end_day {
            break;
        }
        let (next_year, next_m) = next_month(year, month);
        let month_end = days_from_civil(next_year, next_m, 1);
        let x0 = axis.clamp_x(axis.x(month_start.max(axis.start_day)));
        let x1 = axis.clamp_x(axis.x(month_end.min(axis.end_day)));
        if x0 > 0 && x0 < axis.width {
            put_cell(&mut cells, x0, '│', Style::default().fg(theme.border_fg()));
        }
        let span = x1.saturating_sub(x0);
        if span > 1 {
            let full = format!("{} {year}", MONTHS[(month - 1) as usize]);
            let label = if full.chars().count() < span {
                full
            } else {
                MONTHS[(month - 1) as usize].to_owned()
            };
            let color = if overlaps_selected(selected_span, month_start, month_end) {
                theme.success_fg()
            } else {
                theme.subtle_fg()
            };
            put_span_pill(
                &mut cells,
                x0,
                span,
                &label,
                color,
                (POWERLINE_ROUND_LEFT, POWERLINE_ROUND_RIGHT),
                false,
                theme,
            );
        }
        year = next_year;
        month = next_m;
    }
    cells
}

fn sprint_row(
    axis: &Axis,
    sprints: &[TimelineSprint],
    selected_span: Option<(i64, i64)>,
    theme: &Theme,
) -> Vec<GridCell> {
    let mut cells = blank_row(axis.width);
    for sprint in sprints {
        let (Some(start), Some(end)) = (sprint.start_day, sprint.end_day) else {
            continue;
        };
        if end <= axis.start_day || start >= axis.end_day {
            continue;
        }
        let x0 = axis.clamp_x(axis.x(start));
        let x1 = axis.clamp_x(axis.x(end));
        let span = x1.saturating_sub(x0);
        if span < 2 {
            continue;
        }
        let color = if overlaps_selected(selected_span, start, end) {
            darker(theme.success_fg())
        } else {
            sprint_color(sprint.state, theme)
        };
        let label = sprint.short_label();
        if span >= label.chars().count() + 2 {
            put_span_pill(
                &mut cells,
                x0,
                span,
                &label,
                color,
                (' ', POWERLINE_ARROW_RIGHT),
                true,
                theme,
            );
        } else {
            let clipped = truncate_with_ellipsis(&label, span);
            put_str(&mut cells, x0, &clipped, Style::default().fg(color));
        }
    }
    cells
}

fn put_span_pill(
    cells: &mut [GridCell],
    x: usize,
    width: usize,
    label: &str,
    color: Color,
    caps: (char, char),
    fill_left_cap: bool,
    theme: &Theme,
) {
    if width < 3 {
        return;
    }
    let label = truncate_with_ellipsis(label, width.saturating_sub(2));
    let label_width = label.chars().count();
    let cap_style = Style::default().fg(color);
    let fill_style = Style::default().fg(theme.highlight_fg()).bg(color);
    let left_cap_style = if fill_left_cap { fill_style } else { cap_style };
    put_cell(cells, x, caps.0, left_cap_style);
    for fill_x in x + 1..x + width - 1 {
        put_cell(cells, fill_x, ' ', fill_style);
    }
    let label_start = x + 1 + width.saturating_sub(2 + label_width) / 2;
    put_str(cells, label_start, &label, fill_style);
    put_cell(cells, x + width - 1, caps.1, cap_style);
}

fn darker(color: Color) -> Color {
    match color {
        Color::Rgb(red, green, blue) => Color::Rgb(red / 2, green / 2, blue / 2),
        other => other,
    }
}

fn sprint_color(state: SprintState, theme: &Theme) -> Color {
    match state {
        SprintState::Active => theme.muted_fg(),
        SprintState::Closed => theme.muted_fg(),
        SprintState::Future => theme.muted_fg(),
    }
}

fn selected_timeline_span(
    rows: &[TreeRow],
    tree: &TreeState,
    item_spans: &BTreeMap<String, (i64, i64)>,
) -> Option<(i64, i64)> {
    let row = rows.get(tree.selected_row())?;
    let item = &tree.items()[row.item_index];
    item_spans.get(item.id.as_str()).copied()
}

fn axis_range(axis: &Axis, (start, end): (i64, i64)) -> Option<(u16, u16)> {
    if axis.x(end) < 0 || axis.x(start) > axis.width as i32 {
        return None;
    }
    let x0 = axis.clamp_x(axis.x(start));
    let x1 = axis.clamp_x(axis.x(end)).max(x0 + 1);
    Some((
        x0.min(u16::MAX as usize) as u16,
        x1.min(u16::MAX as usize) as u16,
    ))
}

fn overlaps_selected(selected_span: Option<(i64, i64)>, start: i64, end: i64) -> bool {
    selected_span
        .is_some_and(|(selected_start, selected_end)| start < selected_end && end > selected_start)
}

/// Builds one body row's canvas: the today line, then the row's bar drawn over
/// it across the sprint date range its work occupies.
fn bar_row(
    axis: &Axis,
    is_epic: bool,
    kind: &str,
    span: Option<(i64, i64)>,
    is_selected: bool,
    today_x: i32,
    theme: &Theme,
) -> Vec<GridCell> {
    let mut cells = blank_row(axis.width);
    overlay_today_line(&mut cells, today_x, theme);

    let Some((start, end)) = span else {
        return cells;
    };
    // An epic whose sprints fall entirely outside the visible window would
    // otherwise clamp to a misleading 1-cell stub at an edge; draw nothing.
    if axis.x(end) < 0 || axis.x(start) > axis.width as i32 {
        return cells;
    }
    let x0 = axis.clamp_x(axis.x(start));
    let x1 = axis.clamp_x(axis.x(end)).max(x0 + 1);
    // Colour the range by the work item's type (epics in the epic colour),
    // while using centered timeline glyphs so adjacent rows do not visually merge.
    let color = if is_selected {
        theme.success_fg()
    } else if !is_epic {
        theme.accent_fg()
    } else {
        theme.issue_type_fg(kind)
    };
    let style = Style::default().fg(color);
    let span = x1.saturating_sub(x0);
    if span <= 1 {
        put_cell(&mut cells, x0, '●', style);
    } else {
        put_cell(&mut cells, x0, '●', style);
        for cell in cells.iter_mut().take(x1 - 1).skip(x0 + 1) {
            *cell = GridCell {
                ch: TIMELINE_BAR,
                style,
            };
        }
        put_cell(&mut cells, x1 - 1, '●', style);
    }
    cells
}

fn label_line(
    theme: &Theme,
    row: &TreeRow,
    item: &crate::components::generic::tree::TreeItem,
    label_width: usize,
    stats: Option<TimelineEpicStats>,
    is_selected: bool,
    filter: &str,
) -> Line<'static> {
    let is_epic = item.kind == "Epic";
    let indent = "  ".repeat(row.depth);
    let chevron = chevron(row);
    let icon = work_item_key::icon(&item.kind);

    let percent_width = PCT_COL_WIDTH + PCT_TRAILING_GAP;
    let prefix_width =
        indent.width() + chevron.width() + 1 + icon.width() + 1 + item.id.width() + 1;
    let summary_budget = label_width
        .saturating_sub(1 + percent_width)
        .saturating_sub(prefix_width)
        .max(1);
    let summary = truncate_with_ellipsis(&item.label, summary_budget);
    let content_width = prefix_width + summary.width();
    let pad = label_width
        .saturating_sub(PCT_COL_WIDTH)
        .saturating_sub(PCT_TRAILING_GAP)
        .saturating_sub(content_width);

    let base_style = style::selected_row_style(theme, is_selected);
    let chevron_style = if is_selected {
        base_style
    } else {
        Style::default().fg(theme.border_fg())
    };
    let icon_style = base_style.fg(theme.issue_type_fg(&item.kind));
    let key_style = base_style.fg(theme.key_fg());
    let summary_style = if is_selected {
        base_style
    } else if is_epic {
        Style::default().fg(theme.selected_alt_fg())
    } else {
        Style::default().fg(theme.muted_fg())
    };
    let percent_cell = match (is_epic, stats) {
        (true, Some(stats)) => format!("{:>PCT_COL_WIDTH$}", percent_text(stats)),
        _ => " ".repeat(PCT_COL_WIDTH),
    };
    let percent_style = stats
        .filter(|_| is_epic)
        .map_or_else(Style::default, |stats| percent_style(stats, theme));

    let mut spans = vec![
        Span::styled(indent, base_style),
        Span::styled(chevron, chevron_style),
        Span::styled(" ", base_style),
        Span::styled(icon, icon_style),
        Span::styled(" ", base_style),
    ];
    spans.extend(style::highlighted_spans_owned(
        theme, &item.id, filter, key_style,
    ));
    spans.push(Span::styled(" ", base_style));
    spans.extend(style::highlighted_spans_owned(
        theme,
        &summary,
        filter,
        summary_style,
    ));
    spans.push(Span::styled(" ".repeat(pad), base_style));
    spans.push(Span::styled(percent_cell, percent_style));
    spans.push(Span::styled(" ".repeat(PCT_TRAILING_GAP), Style::default()));
    Line::from(spans)
}

fn chevron(row: &TreeRow) -> &'static str {
    if !row.expandable {
        " "
    } else if row.expanded {
        if prefers_plain_icons() {
            "v"
        } else {
            NERD_EXPANDED_ICON
        }
    } else if prefers_plain_icons() {
        ">"
    } else {
        NERD_COLLAPSED_ICON
    }
}

/// The completion-percentage text for an epic, or an em dash when it has no
/// child issues to measure.
fn percent_text(stats: TimelineEpicStats) -> String {
    if stats.total() == 0 {
        String::from("—")
    } else {
        format!("{}%", stats.percent_done())
    }
}

fn percent_style(stats: TimelineEpicStats, theme: &Theme) -> Style {
    let percent = stats.percent_done();
    let color = if stats.total() == 0 || percent == 0 {
        theme.muted_fg()
    } else if percent <= 33 {
        theme.warning_fg()
    } else if percent <= 66 {
        theme.accent_fg()
    } else if percent < 100 {
        theme.key_fg()
    } else {
        theme.success_fg()
    };
    Style::default().fg(color)
}

fn blank_row(width: usize) -> Vec<GridCell> {
    vec![GridCell::blank(); width]
}

fn put_cell(cells: &mut [GridCell], x: usize, ch: char, style: Style) {
    if let Some(cell) = cells.get_mut(x) {
        *cell = GridCell { ch, style };
    }
}

fn put_str(cells: &mut [GridCell], x: usize, text: &str, style: Style) {
    for (offset, ch) in text.chars().enumerate() {
        put_cell(cells, x + offset, ch, style);
    }
}

/// Slices a canvas row to the viewport at `offset` and coalesces equal-styled
/// runs into spans.
fn slice_line(cells: &[GridCell], offset: u16, width: u16) -> Line<'static> {
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut current_style: Option<Style> = None;
    for index in 0..width as usize {
        let cell = cells
            .get(offset as usize + index)
            .copied()
            .unwrap_or_else(GridCell::blank);
        if current_style == Some(cell.style) {
            current.push(cell.ch);
        } else {
            if let Some(style) = current_style {
                spans.push(Span::styled(std::mem::take(&mut current), style));
            }
            current.push(cell.ch);
            current_style = Some(cell.style);
        }
    }
    if let Some(style) = current_style {
        spans.push(Span::styled(current, style));
    }
    Line::from(spans)
}

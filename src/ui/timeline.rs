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
    components::{generic::tree::TreeRow, jira::work_item_key},
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
/// Rolling axis window: how far back and forward from today it spans.
const WINDOW_BACK_DAYS: i64 = 365;
const WINDOW_FWD_DAYS: i64 = 180;
/// Floor on cells-per-day so sprint pills stay legible; above this the axis
/// stretches to fill the viewport.
const MIN_PX_PER_DAY: f64 = 1.0;
/// Glyphs for the today marker: a triangle at the top, a line down the rest.
const TODAY_TRIANGLE: char = '▼';
const TODAY_LINE: char = '┊';
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
    render_toolbar(frame, toolbar_area, keybindings, theme);

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
    let h_offset = state.resolve_h_offset(
        grid_main_width,
        axis.width.min(u16::MAX as usize) as u16,
        today_x,
    );
    let range = tree.visible_range(body_height);

    render_label_header(frame, label_area, theme);
    render_grid_header(
        frame,
        grid_area,
        grid_main_width,
        &axis,
        &data.sprints,
        today_x,
        h_offset,
        theme,
    );

    let sprint_dates = state.sprint_dates();
    let item_sprints = state.item_sprints();
    let epic_stats = state.epic_stats();

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
        label_lines.push(label_line(theme, row, item, label_width as usize, stats, selected));

        let ids = item_sprints
            .get(item.id.as_str())
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let is_epic = item.kind == "Epic";
        let cells = bar_row(&axis, is_epic, &item.kind, ids, today_x, sprint_dates, theme);
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

fn render_toolbar(frame: &mut Frame<'_>, area: Rect, keybindings: &KeyBindings, theme: &Theme) {
    let [title_area, hint_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(keybindings.shortcuts_hint_width()),
        ])
        .areas(area);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " Timeline — epics scheduled by sprint",
            Style::default().fg(theme.muted_fg()),
        ))),
        title_area,
    );
    frame.render_widget(chrome::shortcuts_hint(keybindings, theme), hint_area);
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
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Epic / progress",
            Style::default()
                .fg(theme.subtle_fg())
                .add_modifier(Modifier::BOLD),
        ))),
        Rect {
            height: 1,
            ..label_area
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn render_grid_header(
    frame: &mut Frame<'_>,
    grid_area: Rect,
    grid_main_width: u16,
    axis: &Axis,
    sprints: &[TimelineSprint],
    today_x: i32,
    h_offset: u16,
    theme: &Theme,
) {
    let mut month = month_row(axis, theme);
    let mut sprint = sprint_row(axis, sprints, theme);
    // The today marker is a full-height line: a triangle caps it on the top
    // header row, a thin line runs down the rest but only through blank cells,
    // so it never overwrites a pill or bar.
    overlay_today_triangle(&mut month, today_x, theme);
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

fn overlay_today_triangle(cells: &mut [GridCell], today_x: i32, theme: &Theme) {
    if today_x >= 0 && (today_x as usize) < cells.len() {
        put_cell(
            cells,
            today_x as usize,
            TODAY_TRIANGLE,
            Style::default()
                .fg(theme.warning_fg())
                .add_modifier(Modifier::BOLD),
        );
    }
}

fn overlay_today_line(cells: &mut [GridCell], today_x: i32, theme: &Theme) {
    if today_x < 0 {
        return;
    }
    let x = today_x as usize;
    if cells.get(x).is_some_and(|cell| cell.ch == ' ') {
        put_cell(cells, x, TODAY_LINE, Style::default().fg(theme.warning_fg()));
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

fn month_row(axis: &Axis, theme: &Theme) -> Vec<GridCell> {
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
            let label = truncate_with_ellipsis(&label, span.saturating_sub(1));
            let start = x0 + span.saturating_sub(label.chars().count()) / 2;
            put_str(
                &mut cells,
                start,
                &label,
                Style::default()
                    .fg(theme.accent_fg())
                    .add_modifier(Modifier::BOLD),
            );
        }
        year = next_year;
        month = next_m;
    }
    cells
}

fn sprint_row(axis: &Axis, sprints: &[TimelineSprint], theme: &Theme) -> Vec<GridCell> {
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
        let style = Style::default().fg(sprint_color(sprint.state, theme));
        let label = sprint.short_label();
        if span >= label.chars().count() + 2 {
            put_cell(&mut cells, x0, '(', style);
            put_cell(&mut cells, x1 - 1, ')', style);
            let start = x0 + (span - label.chars().count()) / 2;
            put_str(&mut cells, start, &label, style);
        } else {
            let clipped = truncate_with_ellipsis(&label, span);
            put_str(&mut cells, x0, &clipped, style);
        }
    }
    cells
}

fn sprint_color(state: SprintState, theme: &Theme) -> Color {
    match state {
        SprintState::Active => theme.success_fg(),
        SprintState::Closed => theme.muted_fg(),
        SprintState::Future => theme.subtle_fg(),
    }
}

/// Builds one body row's canvas: the today line, then the row's bar drawn over
/// it across the sprint date range its work occupies.
fn bar_row(
    axis: &Axis,
    is_epic: bool,
    kind: &str,
    sprint_ids: &[i64],
    today_x: i32,
    sprint_dates: &BTreeMap<i64, (i64, i64)>,
    theme: &Theme,
) -> Vec<GridCell> {
    let mut cells = blank_row(axis.width);
    overlay_today_line(&mut cells, today_x, theme);

    let Some((start, end)) = span_of(sprint_ids, sprint_dates) else {
        return cells;
    };
    // An epic whose sprints fall entirely outside the visible window would
    // otherwise clamp to a misleading 1-cell stub at an edge; draw nothing.
    if axis.x(end) < 0 || axis.x(start) > axis.width as i32 {
        return cells;
    }
    let x0 = axis.clamp_x(axis.x(start));
    let x1 = axis.clamp_x(axis.x(end)).max(x0 + 1);
    // Colour the bar by the work item's type (epics in the epic colour), which
    // is distinct from the accent-coloured scrollbar so they never blend at the
    // right edge.
    let glyph = if is_epic { '█' } else { '▓' };
    let style = Style::default().fg(theme.issue_type_fg(kind));
    for cell in cells.iter_mut().take(x1).skip(x0) {
        *cell = GridCell { ch: glyph, style };
    }
    cells
}

fn span_of(ids: &[i64], sprint_dates: &BTreeMap<i64, (i64, i64)>) -> Option<(i64, i64)> {
    let mut span: Option<(i64, i64)> = None;
    for id in ids {
        if let Some(&(start, end)) = sprint_dates.get(id) {
            span = Some(match span {
                Some((s, e)) => (s.min(start), e.max(end)),
                None => (start, end),
            });
        }
    }
    span
}

fn label_line(
    theme: &Theme,
    row: &TreeRow,
    item: &crate::components::generic::tree::TreeItem,
    label_width: usize,
    stats: Option<TimelineEpicStats>,
    is_selected: bool,
) -> Line<'static> {
    let is_epic = item.kind == "Epic";
    let indent = "  ".repeat(row.depth);
    let chevron = chevron(row);
    let icon = work_item_key::icon(&item.kind);

    let prefix_width =
        indent.width() + chevron.width() + 1 + icon.width() + 1 + item.id.width() + 1;
    let summary_budget = label_width
        .saturating_sub(1 + PCT_COL_WIDTH)
        .saturating_sub(prefix_width)
        .max(1);
    let summary = truncate_with_ellipsis(&item.label, summary_budget);
    let content_width = prefix_width + summary.width();
    let pad = label_width
        .saturating_sub(PCT_COL_WIDTH)
        .saturating_sub(content_width);

    let summary_style = if is_epic {
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

    let spans = vec![
        Span::raw(indent),
        Span::styled(chevron, Style::default().fg(theme.border_fg())),
        Span::raw(" "),
        Span::styled(icon, Style::default().fg(theme.issue_type_fg(&item.kind))),
        Span::raw(" "),
        Span::styled(item.id.clone(), Style::default().fg(theme.key_fg())),
        Span::raw(" "),
        Span::styled(summary, summary_style),
        Span::raw(" ".repeat(pad)),
        Span::styled(percent_cell, percent_style),
    ];
    Line::from(spans).style(style::selected_row_style(theme, is_selected))
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
    let color = if stats.total() == 0 || stats.done == 0 {
        theme.muted_fg()
    } else if stats.percent_done() >= 100 {
        theme.success_fg()
    } else {
        theme.accent_fg()
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

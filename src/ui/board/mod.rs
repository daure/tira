use std::collections::{BTreeMap, HashMap};

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    App, KeyBindings,
    app::{board_grouped_lanes, board_issue_column},
    ui::{layout::truncate_with_ellipsis, scrollbar},
};

mod card;
mod filter;
mod heading;
mod lanes;
mod layout;
mod text;
mod toolbar;

use filter::board_issue_matches_filter;
use heading::sticky_headings;
use lanes::{RenderedBoard, generate_rendered_board};
use layout::ColumnLayout;
use crate::ui::layout::slice_line;
use toolbar::{details_trigger_text, render_details_trigger, render_filter, render_group_trigger};

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
    let v_offset = resolve_vertical_offset(app, viewport_height, total_lines, sel_y_start, sel_y_end);

    let strip_width = columns.strip_width();
    let h_offset = resolve_horizontal_offset(app, &columns, board_area.width, strip_width);

    let (mut visible_lines, sticky_left) =
        apply_sticky_headers(&rendered, v_offset, viewport_height);
    slice_to_viewport(
        &mut visible_lines,
        &sticky_left,
        &columns,
        h_offset,
        board_area.width,
    );

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

/// Resolve the vertical scroll offset (in lines) for this frame and advance the
/// glide animator toward it. While the user is wheel-scrolling (manual), their
/// stored offset is kept; otherwise the offset follows the selection into view.
/// Returns the animated offset to render at.
///
/// Writes `scroll_offset` and the vertical animator target via interior
/// mutability — must run once per frame, before reading `v_scroll`.
fn resolve_vertical_offset(
    app: &App,
    viewport_height: usize,
    total_lines: usize,
    sel_y_start: usize,
    sel_y_end: usize,
) -> usize {
    let max_v = total_lines.saturating_sub(viewport_height);
    let board = app.board();
    // The follow logic is committed only when the viewport is at least as tall
    // as the height it was last committed at. A shorter viewport — e.g. zellij
    // rendering a stacked (hidden) pane at near-zero height — would otherwise
    // drag `scroll_offset` to the bottom to "keep the selection visible" in a
    // 1-row strip; on restore the selection is then anchored to the bottom. By
    // skipping the follow logic (and not persisting) while shrunk, the stored
    // position is preserved and restored intact. A real interaction clears the
    // commitment (see `BoardState::dispatch`/`scroll_viewport`), so a genuine
    // resize-down re-commits and follows at the new size.
    let committed = board.committed_v_viewport.get();
    let is_committed = committed.is_none_or(|height| viewport_height >= height);

    if is_committed {
        let mut scroll_offset = board.scroll_offset.get();
        if !board.manual_v_scroll.get() {
            if sel_y_start < scroll_offset {
                scroll_offset = sel_y_start;
            } else if sel_y_end > scroll_offset + viewport_height {
                scroll_offset = sel_y_end.saturating_sub(viewport_height);
            }
        }
        scroll_offset = scroll_offset.min(max_v);
        board.scroll_offset.set(scroll_offset);
        board.committed_v_viewport.set(Some(viewport_height));
    }

    let scroll_offset = board.scroll_offset.get().min(max_v);
    let v_anim = &board.v_scroll;
    v_anim.set_target(scroll_offset as f64);
    // When the committed viewport height changes (resize, including the
    // shrink/restore zellij does for stacked panes), snap to the target instead
    // of gliding so there is no scrollbar "bounce".
    if board.last_v_viewport.get() != Some(viewport_height) {
        board.last_v_viewport.set(Some(viewport_height));
        v_anim.snap_to(scroll_offset as f64);
    }
    (v_anim.current().round() as usize).min(max_v)
}

/// Resolve the horizontal scroll offset (in cells) for this frame and advance
/// the glide animator toward it. The target is the user's manual pan while
/// wheel-scrolling, else the selection-following snap position; partial columns
/// at the edges are the "peek". Returns the animated offset to slice at.
///
/// Writes `manual_h_offset` (when manual) and the horizontal animator target via
/// interior mutability — must run once per frame, before reading `h_scroll`.
fn resolve_horizontal_offset(
    app: &App,
    columns: &ColumnLayout,
    board_width: u16,
    strip_width: u16,
) -> u16 {
    let max_h = strip_width.saturating_sub(board_width);
    let h_target = if app.board().manual_h_scroll.get() {
        let manual = app.board().manual_h_offset.get().min(max_h);
        app.board().manual_h_offset.set(manual);
        manual
    } else {
        columns.target_offset(board_width)
    };
    let h_anim = &app.board().h_scroll;
    h_anim.set_target(f64::from(h_target));
    // Snap on resize, mirroring the vertical handling above.
    if app.board().last_h_dims.get() != Some((board_width, strip_width)) {
        app.board().last_h_dims.set(Some((board_width, strip_width)));
        h_anim.snap_to(f64::from(h_target));
    }
    (h_anim.current().round() as u16).min(max_h)
}

/// Build the viewport's lines and, alongside, which of them are pinned to the
/// left edge. Group/swimlane headers (heading levels 0 and 1) are
/// "horizontally sticky" so their label stays readable when the board is
/// scrolled right; column-title rows (level 2) and cards scroll normally. The
/// stacked sticky headings overwrite the top rows of the viewport.
fn apply_sticky_headers(
    rendered: &RenderedBoard,
    v_offset: usize,
    viewport_height: usize,
) -> (Vec<Line<'static>>, Vec<bool>) {
    let mut visible_lines = rendered
        .lines
        .iter()
        .skip(v_offset)
        .take(viewport_height)
        .cloned()
        .collect::<Vec<_>>();

    let header_levels: HashMap<usize, usize> = rendered
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

    (visible_lines, sticky_left)
}

/// Slice every visible line to the horizontal viewport. Sticky headers slice
/// from the start so their label stays at the left; everything else slices at
/// the animated horizontal offset so columns line up.
fn slice_to_viewport(
    visible_lines: &mut [Line<'static>],
    sticky_left: &[bool],
    columns: &ColumnLayout,
    h_offset: u16,
    board_width: u16,
) {
    if columns.scrolling {
        for (i, line) in visible_lines.iter_mut().enumerate() {
            let start = if sticky_left[i] { 0 } else { h_offset as usize };
            *line = slice_line(std::mem::take(line), start, board_width as usize);
        }
    }
}

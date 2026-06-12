use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::{
    App, KeyBindings, components::generic::dialog::Dialog,
    components::generic::notification::NotificationKind, keymap::HelpItem, keymap::HelpScope,
    services::jira::CommandLogEntry, services::jira::SprintSummary, ui::theme::Theme,
};

use super::{chrome, layout, scrollbar};

pub fn render_command_log_dialog(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let entries = app.command_log_entries();
    let width = area.width.saturating_sub(area.width / 10);
    // The path cell wraps within the content column: dialog border (2) +
    // dialog padding (2) + scrollbar column (1) are reserved.
    let content_width = width.saturating_sub(5).max(1);

    let lines: Vec<Line<'static>> = entries
        .iter()
        .flat_map(|entry| command_log_entry_lines(entry, app, content_width))
        .collect();
    let total = lines.len();

    // Grow with the content up to 80% of the screen height.
    let max_height = (area.height * 4 / 5).max(3);
    let height = (total as u16)
        .saturating_add(2)
        .min(max_height)
        .min(area.height.saturating_sub(2))
        .max(3);

    let inner = Dialog::new("Command log", width, height)
        .border_style(Style::default().fg(app.theme().border_fg()))
        .y_offset(area.height.saturating_sub(height) / 2)
        .render(frame, area);

    let content_area = Rect {
        width: inner.width.saturating_sub(1),
        ..inner
    };
    let scrollbar_area = Rect {
        x: inner.x + inner.width.saturating_sub(1),
        y: inner.y,
        width: 1,
        height: inner.height,
    };

    let viewport = inner.height as usize;
    let max_scroll = total.saturating_sub(viewport);
    let scroll = if app.command_log_follows_tail() {
        max_scroll
    } else {
        app.command_log_scroll().min(max_scroll)
    };
    app.cache_command_log_layout(scroll, total, viewport);

    frame.render_widget(
        Paragraph::new(lines).scroll((scroll as u16, 0)),
        content_area,
    );

    if total > viewport {
        scrollbar::render_range(
            frame,
            scrollbar_area,
            total,
            scroll..scroll + viewport,
            app.theme(),
        );
    }
}

pub fn render_sprint_details_dialog(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = app.theme();
    let width = area.width.saturating_sub(4).min(64).max(24);
    let text_width = width.saturating_sub(4) as usize;

    let board = app.board().data();
    let board_name = board.map(|data| data.name.as_str()).unwrap_or("Board");
    let sprint = board.and_then(|data| data.sprint.as_ref());

    let lines = sprint
        .map(|sprint| sprint_details_lines(sprint, text_width, app))
        .unwrap_or_else(|| no_sprint_lines(board_name, app));

    let height = (lines.len() as u16 + 2)
        .min(area.height.saturating_sub(2))
        .max(3);
    let inner = Dialog::new("Sprint details", width, height)
        .border_style(Style::default().fg(theme.border_fg()))
        .y_offset(area.height.saturating_sub(height) / 2)
        .render(frame, area);

    frame.render_widget(Paragraph::new(lines), inner);
}

fn no_sprint_lines(board_name: &str, app: &App) -> Vec<Line<'static>> {
    vec![
        Line::from(Span::styled(
            board_name.to_owned(),
            Style::default()
                .fg(app.theme().selected_alt_fg())
                .add_modifier(Modifier::BOLD),
        )),
        Line::default(),
        Line::from(Span::styled(
            "No active sprint",
            Style::default().fg(app.theme().muted_fg()),
        )),
    ]
}

fn sprint_details_lines(
    sprint: &SprintSummary,
    text_width: usize,
    app: &App,
) -> Vec<Line<'static>> {
    let theme = app.theme();
    let mut lines = vec![Line::from(Span::styled(
        sprint.name.clone(),
        Style::default()
            .fg(theme.selected_alt_fg())
            .add_modifier(Modifier::BOLD),
    ))];

    if let Some(goal) = &sprint.goal {
        for goal_line in wrap_text(goal, text_width) {
            lines.push(Line::from(Span::styled(
                goal_line,
                Style::default().fg(theme.status_text()),
            )));
        }
    }

    if let Some(days_left) = sprint.days_left_label() {
        lines.push(Line::default());
        lines.push(Line::from(Span::styled(
            days_left,
            Style::default().fg(theme.subtle_fg()),
        )));
    }

    if sprint.start_date.is_some() || sprint.end_date.is_some() {
        let column = (text_width / 2).max(12);
        lines.push(Line::default());
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:<column$}", "Start date"),
                Style::default()
                    .fg(theme.muted_fg())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "End date",
                Style::default()
                    .fg(theme.muted_fg())
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        let start = sprint
            .start_date
            .clone()
            .unwrap_or_else(|| String::from("—"));
        let end = sprint.end_date.clone().unwrap_or_else(|| String::from("—"));
        lines.push(Line::from(vec![
            Span::styled(
                format!("{start:<column$}"),
                Style::default().fg(theme.selected_alt_fg()),
            ),
            Span::styled(end, Style::default().fg(theme.selected_alt_fg())),
        ]));
    }

    lines
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_owned()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let next_len = if current.is_empty() {
            word.chars().count()
        } else {
            current.chars().count() + 1 + word.chars().count()
        };
        if next_len > width && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
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

/// One key-binding row: a fixed-width binding column followed by the summary,
/// background-highlighted when selected.
fn help_row_line<'a>(
    item: &'a HelpItem,
    is_selected: bool,
    binding_width: usize,
    theme: &Theme,
) -> Line<'a> {
    let row_style = if is_selected {
        Style::default().bg(theme.selected_bg())
    } else {
        Style::default()
    };
    let binding = format!("{:width$}", item.binding, width = binding_width);
    Line::from(vec![
        Span::styled(
            binding,
            row_style.fg(theme.accent_fg()).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", row_style),
        Span::styled(item.summary.as_str(), row_style.fg(theme.status_text())),
    ])
}

/// Rows for the items belonging to `scope`, in their original order, with the
/// globally-selected item highlighted.
fn section_lines<'a>(
    items: &'a [HelpItem],
    scope: HelpScope,
    selected: usize,
    binding_width: usize,
    theme: &Theme,
) -> Vec<Line<'a>> {
    items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.scope == scope)
        .map(|(index, item)| help_row_line(item, index == selected, binding_width, theme))
        .collect()
}

pub fn render_help_dialog(frame: &mut Frame<'_>, area: Rect, app: &App, keybindings: &KeyBindings) {
    let items = keybindings.help_items_for_context(
        app.screen(),
        app.active_tab().title(),
        app.help_context(),
    );
    if items.is_empty() {
        return;
    }
    let selected = app.help_selected().min(items.len().saturating_sub(1));
    let binding_width = items
        .iter()
        .map(|item| item.binding.chars().count())
        .max()
        .unwrap_or(0);
    let summary_width = items
        .iter()
        .map(|item| item.summary.chars().count())
        .max()
        .unwrap_or(0);
    let content_width = binding_width + 2 + summary_width;
    let width = area.width.min((content_width + 9) as u16).max(64);
    let height = area.height.min(26).max(16);
    let inner = Dialog::new("Shortcuts", width, height)
        .border_style(Style::default().fg(app.theme().border_fg()))
        .y_offset(area.height.saturating_sub(height) / 2)
        .render(frame, area);

    let [list_area, description_area] = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Min(1),
            ratatui::layout::Constraint::Length(3),
        ])
        .areas(inner);
    let scrollbar_area = Rect {
        x: list_area.x + list_area.width + 1,
        y: list_area.y,
        width: 1,
        height: list_area.height,
    };

    let local_count = items
        .iter()
        .filter(|item| item.scope == HelpScope::Local)
        .count();
    let global_count = items
        .iter()
        .filter(|item| item.scope == HelpScope::Global)
        .count();
    let total_lines = 1 + local_count + 1 + 1 + global_count;

    let selected_line = if items[selected].scope == HelpScope::Local {
        let index_in_local = items[..selected]
            .iter()
            .filter(|item| item.scope == HelpScope::Local)
            .count();
        1 + index_in_local
    } else {
        let index_in_global = items[..selected]
            .iter()
            .filter(|item| item.scope == HelpScope::Global)
            .count();
        1 + local_count + 2 + index_in_global
    };

    let viewport = list_area.height as usize;
    let mut scroll = 0;
    if total_lines > viewport {
        let max_scroll = total_lines.saturating_sub(viewport);
        let middle = viewport / 2;
        scroll = if selected_line <= middle {
            0
        } else {
            selected_line - middle
        };
        scroll = scroll.min(max_scroll);
    }

    let mut lines = Vec::new();
    lines.push(Line::from(vec![Span::styled(
        "── Local ──",
        Style::default()
            .fg(app.theme().success_fg())
            .add_modifier(Modifier::BOLD),
    )]));
    lines.extend(section_lines(
        &items,
        HelpScope::Local,
        selected,
        binding_width,
        app.theme(),
    ));

    lines.push(Line::default());
    lines.push(Line::from(vec![Span::styled(
        "── Global ──",
        Style::default()
            .fg(app.theme().success_fg())
            .add_modifier(Modifier::BOLD),
    )]));
    lines.extend(section_lines(
        &items,
        HelpScope::Global,
        selected,
        binding_width,
        app.theme(),
    ));

    let scroll_u16 = scroll as u16;
    frame.render_widget(Paragraph::new(lines).scroll((scroll_u16, 0)), list_area);

    if total_lines > viewport {
        scrollbar::render_range(
            frame,
            scrollbar_area,
            total_lines,
            scroll..scroll + viewport,
            app.theme(),
        );
    }
    if let Some(item) = items.get(selected) {
        let lines = vec![
            Line::from(vec![
                Span::styled(
                    item.binding.clone(),
                    Style::default()
                        .fg(app.theme().accent_fg())
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(
                    item.summary.as_str(),
                    Style::default()
                        .fg(app.theme().selected_alt_fg())
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(Span::styled(
                item.description.as_str(),
                Style::default().fg(app.theme().subtle_fg()),
            )),
        ];
        let separator_area = Rect {
            x: inner.x.saturating_sub(2),
            y: description_area.y,
            width: inner.width.saturating_add(4),
            height: 1,
        };
        let desc_inner = Rect {
            x: description_area.x,
            y: description_area.y.saturating_add(1),
            width: description_area.width,
            height: description_area.height.saturating_sub(1),
        };
        frame.render_widget(
            chrome::border_separator(separator_area.width, app.theme()),
            separator_area,
        );
        frame.render_widget(Paragraph::new(lines), desc_inner);
    }
}
pub fn render_notifications(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let notification_width = area.width.min(54);
    if notification_width < 20 {
        return;
    }

    for (index, notification) in app.notifications().iter().enumerate() {
        let y = area.y + 1 + (index as u16 * 5);
        if y + 4 > area.y + area.height {
            return;
        }

        let offset = notification.slide_offset(notification_width);
        let x = area
            .x
            .saturating_add(area.width.saturating_sub(notification_width + 1))
            .saturating_add(offset);
        if x >= area.x + area.width {
            continue;
        }

        let notification_area = Rect {
            x,
            y,
            width: notification_width,
            height: 4,
        };
        let icon = if crate::ui::theme::prefers_plain_icons() {
            match notification.kind() {
                NotificationKind::Success => "OK",
                NotificationKind::Error => "!!",
            }
        } else {
            match notification.kind() {
                NotificationKind::Success => "",
                NotificationKind::Error => "",
            }
        };
        let icon_style = match notification.kind() {
            NotificationKind::Success => Style::default().fg(app.theme().success_fg()),
            NotificationKind::Error => Style::default().fg(app.theme().error_fg()),
        };
        let content_width = notification_width.saturating_sub(4) as usize;
        let title =
            layout::truncate_with_ellipsis(notification.title(), content_width.saturating_sub(3));
        let message = layout::truncate_with_ellipsis(notification.message(), content_width);
        let lines = vec![
            Line::from(vec![
                Span::raw(" "),
                Span::styled(icon, icon_style),
                Span::raw("  "),
                Span::styled(
                    title,
                    Style::default()
                        .fg(app.theme().selected_alt_fg())
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw(" "),
                Span::styled(message, Style::default().fg(app.theme().subtle_fg())),
            ]),
        ];
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme().border_fg()));
        let inner = block.inner(notification_area);

        frame.render_widget(Clear, notification_area);
        frame.render_widget(block, notification_area);
        frame.render_widget(Paragraph::new(lines), inner);
    }
}

/// Renders one command-log entry as one or more lines. The leading
/// `timestamp method` prefix and the trailing `status duration` suffix stay on
/// the first and last lines; the path wraps within the remaining width with its
/// continuation lines indented to align under the path column (cell wrapping),
/// never back to column zero. Only the path itself is background-highlighted —
/// the query string (from `?` on) stays unhighlighted.
fn command_log_entry_lines(entry: &CommandLogEntry, app: &App, width: u16) -> Vec<Line<'static>> {
    let theme = app.theme();
    let status_style = if command_log_status_succeeded(entry.status.as_str()) {
        Style::default().fg(theme.success_fg())
    } else {
        Style::default().fg(theme.error_fg())
    };
    let path_style = Style::default()
        .fg(theme.selected_alt_fg())
        .bg(theme.selected_bg());
    let query_style = Style::default().fg(theme.muted_fg());

    let indent = entry.timestamp.chars().count() + 1 + entry.method.chars().count() + 1;
    let available = (width as usize).saturating_sub(indent).max(1);
    let suffix_len =
        entry.status.chars().count() + 2 + entry.duration_ms.to_string().chars().count() + 2;

    let path_chars: Vec<char> = entry.path.chars().collect();
    // The query string (from the first `?`) is not part of the highlighted path.
    let query_start = path_chars
        .iter()
        .position(|&c| c == '?')
        .unwrap_or(path_chars.len());
    let segments = path_segments(path_chars.len(), available);
    let last = segments.len().saturating_sub(1);

    let mut lines = Vec::new();
    for (index, &(start, end)) in segments.iter().enumerate() {
        let mut spans = if index == 0 {
            vec![
                Span::styled(
                    format!("{} ", entry.timestamp),
                    Style::default().fg(theme.muted_fg()),
                ),
                Span::styled(
                    format!("{} ", entry.method),
                    Style::default().fg(theme.accent_fg()),
                ),
            ]
        } else {
            vec![Span::raw(" ".repeat(indent))]
        };
        push_path_spans(
            &mut spans,
            &path_chars,
            start,
            end,
            query_start,
            path_style,
            query_style,
        );

        if index == last {
            let fits = (end - start) + 1 + suffix_len <= available;
            if fits {
                spans.extend(command_log_suffix_spans(entry, status_style, theme, true));
                lines.push(Line::from(spans));
            } else {
                lines.push(Line::from(spans));
                let mut suffix = vec![Span::raw(" ".repeat(indent))];
                suffix.extend(command_log_suffix_spans(entry, status_style, theme, false));
                lines.push(Line::from(suffix));
            }
        } else {
            lines.push(Line::from(spans));
        }
    }
    lines
}

fn command_log_status_succeeded(status: &str) -> bool {
    status
        .parse::<u16>()
        .is_ok_and(|code| (200..400).contains(&code))
}

/// Pushes the `[start, end)` slice of `chars` as styled spans, splitting at
/// `query_start` so the path is background-highlighted and the query string is
/// not.
fn push_path_spans(
    spans: &mut Vec<Span<'static>>,
    chars: &[char],
    start: usize,
    end: usize,
    query_start: usize,
    path_style: Style,
    query_style: Style,
) {
    let path_end = end.min(query_start);
    if start < path_end {
        spans.push(Span::styled(
            chars[start..path_end].iter().collect::<String>(),
            path_style,
        ));
    }
    let query_begin = start.max(query_start);
    if query_begin < end {
        spans.push(Span::styled(
            chars[query_begin..end].iter().collect::<String>(),
            query_style,
        ));
    }
}

fn command_log_suffix_spans(
    entry: &CommandLogEntry,
    status_style: Style,
    theme: &crate::ui::theme::Theme,
    leading_space: bool,
) -> Vec<Span<'static>> {
    let status = if leading_space {
        format!(" {}", entry.status)
    } else {
        entry.status.clone()
    };
    vec![
        Span::styled(status, status_style),
        Span::styled(
            format!("  {}ms", entry.duration_ms),
            Style::default().fg(theme.muted_fg()),
        ),
    ]
}

/// Fixed-width character ranges covering a path of `len` chars. Paths are
/// unbroken URLs, so they wrap by character rather than by word. Always returns
/// at least one (possibly empty) range so the prefix line is emitted.
fn path_segments(len: usize, width: usize) -> Vec<(usize, usize)> {
    if len == 0 {
        return vec![(0, 0)];
    }
    let width = width.max(1);
    (0..len)
        .step_by(width)
        .map(|start| (start, (start + width).min(len)))
        .collect()
}

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::{
    App, KeyBindings,
    components::generic::dialog::Dialog,
    components::generic::notification::NotificationKind,
    keymap::HelpScope,
    services::jira::CommandLogEntry,
};

use super::{chrome, layout, scrollbar};

pub fn render_command_log_dialog(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let entries = app.command_log_entries();
    let height = entries.len().min(8) as u16 + 2;
    let width = area.width.saturating_sub(area.width / 10);
    let height = height.min(area.height.saturating_sub(2)).max(3);
    let inner = Dialog::new("Command log", width, height)
        .border_style(Style::default().fg(app.theme().border_fg()))
        .y_offset(area.height.saturating_sub(height) / 2)
        .render(frame, area);

    let start = entries.len().saturating_sub(inner.height as usize);
    let lines = entries[start..]
        .iter()
        .map(|entry| render_command_log_line(entry, app))
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), inner);
}

pub fn render_help_dialog(frame: &mut Frame<'_>, area: Rect, app: &App, keybindings: &KeyBindings) {
    let items = keybindings.help_items(app.screen(), app.active_tab(), app.is_any_dropdown_open());
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
    let width = area.width.min((content_width + 5) as u16).max(48);
    let height = area.height.min(20).max(12);
    let inner = Dialog::new("Keyboard help", width, height)
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
    let list_content_area = list_area;
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

    let viewport = list_content_area.height as usize;
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

    // binding_width is already defined at the top

    let mut lines = Vec::new();
    lines.push(Line::from(vec![Span::styled(
        "── Local ──",
        Style::default()
            .fg(app.theme().success_fg())
            .add_modifier(Modifier::BOLD),
    )]));

    for (index, item) in items.iter().enumerate() {
        if item.scope == HelpScope::Local {
            let is_selected = index == selected;
            let row_style = if is_selected {
                Style::default().bg(app.theme().selected_bg())
            } else {
                Style::default()
            };
            let binding = format!("{:width$}", item.binding, width = binding_width);
            lines.push(Line::from(vec![
                Span::styled(
                    binding,
                    row_style
                        .fg(app.theme().accent_fg())
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("  ", row_style),
                Span::styled(
                    item.summary.as_str(),
                    row_style.fg(app.theme().status_text()),
                ),
            ]));
        }
    }

    lines.push(Line::default());
    lines.push(Line::from(vec![Span::styled(
        "── Global ──",
        Style::default()
            .fg(app.theme().success_fg())
            .add_modifier(Modifier::BOLD),
    )]));

    for (index, item) in items.iter().enumerate() {
        if item.scope == HelpScope::Global {
            let is_selected = index == selected;
            let row_style = if is_selected {
                Style::default().bg(app.theme().selected_bg())
            } else {
                Style::default()
            };
            let binding = format!("{:width$}", item.binding, width = binding_width);
            lines.push(Line::from(vec![
                Span::styled(
                    binding,
                    row_style
                        .fg(app.theme().accent_fg())
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("  ", row_style),
                Span::styled(
                    item.summary.as_str(),
                    row_style.fg(app.theme().status_text()),
                ),
            ]));
        }
    }

    let scroll_u16 = scroll as u16;
    frame.render_widget(
        Paragraph::new(lines).scroll((scroll_u16, 0)),
        list_content_area,
    );

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

fn render_command_log_line(entry: &CommandLogEntry, app: &App) -> Line<'static> {
    let status_style = if entry.status == "ERR" {
        Style::default().fg(app.theme().error_fg())
    } else {
        Style::default().fg(app.theme().success_fg())
    };

    Line::from(vec![
        Span::styled(
            format!("{} ", entry.timestamp),
            Style::default().fg(app.theme().muted_fg()),
        ),
        Span::styled(
            format!("{} ", entry.method),
            Style::default().fg(app.theme().accent_fg()),
        ),
        Span::raw(format!("{} ", entry.path)),
        Span::styled(format!("{} ", entry.status), status_style),
        Span::styled(
            format!(" {}ms", entry.duration_ms),
            Style::default().fg(app.theme().muted_fg()),
        ),
    ])
}

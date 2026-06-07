use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::{
    App, KeyBindings, components::generic::dialog::Dialog,
    components::generic::notification::NotificationKind, services::jira::CommandLogEntry,
};

use super::layout;

pub fn render_command_log_dialog(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let entries = app.command_log_entries();
    let height = entries.len().min(8) as u16 + 2;
    let width = area.width.saturating_sub(area.width / 10);
    let height = height.min(area.height.saturating_sub(2)).max(3);
    let inner = Dialog::new("Command log", width, height)
        .border_style(Style::default().fg(app.theme().border_fg()))
        .y_offset(1)
        .render(frame, area);

    let start = entries.len().saturating_sub(inner.height as usize);
    let lines = entries[start..]
        .iter()
        .map(|entry| render_command_log_line(entry, app))
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), inner);
}

pub fn render_help_dialog(frame: &mut Frame<'_>, area: Rect, app: &App, keybindings: &KeyBindings) {
    let width = area.width.min(72).max(32);
    let height = area.height.min(10).max(6);
    let inner = Dialog::new("Keyboard help", width, height)
        .border_style(Style::default().fg(app.theme().border_fg()))
        .y_offset(1)
        .render(frame, area);
    let lines = vec![
        Line::from(Span::styled(
            "Context",
            Style::default()
                .fg(app.theme().selected_alt_fg())
                .add_modifier(Modifier::BOLD),
        )),
        Line::raw(keybindings.list_hint_text()),
        Line::raw(""),
        Line::from(Span::styled(
            "Global",
            Style::default()
                .fg(app.theme().selected_alt_fg())
                .add_modifier(Modifier::BOLD),
        )),
        Line::raw(format!("{} close help", keybindings.open_help_label())),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
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
        let icon = match notification.kind() {
            NotificationKind::Success => "",
            NotificationKind::Error => "",
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

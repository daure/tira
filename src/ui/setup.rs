use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{App, CredentialField};

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let form = app.setup_form();
    let active_field = form.active_field();
    let mut lines = Vec::with_capacity(8);

    lines.push(Line::from(Span::styled(
        "Jira connection",
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::raw(""));

    let mut active_field_idx = 0;

    for (idx, (field, value)) in form.fields().into_iter().enumerate() {
        if field == active_field {
            active_field_idx = idx;
        }
        let marker = if field == active_field { "> " } else { "  " };
        let display_value = if field == CredentialField::ApiKey && !value.is_empty() {
            "*".repeat(value.chars().count())
        } else {
            value.to_owned()
        };

        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(Color::Green)),
            Span::styled(
                format!("{:12}", field.label()),
                Style::default().fg(Color::Gray),
            ),
            Span::raw(display_value),
        ]));
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "Tab next field | Shift+Tab previous field | Enter load issues | Ctrl+C quit",
        Style::default().fg(Color::DarkGray),
    )));

    frame.render_widget(Paragraph::new(lines), area);

    let cursor_offset = form.cursors()[form.active_field_idx()];
    let cursor_x = area.x + 14 + cursor_offset as u16;
    let cursor_y = area.y + 2 + active_field_idx as u16;
    frame.set_cursor_position(ratatui::layout::Position::new(cursor_x, cursor_y));
}

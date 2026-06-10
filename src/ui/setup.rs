use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{App, CredentialField, KeyBindings};

/// Width of the field-label column (e.g. `"API key"` padded). The value/cursor
/// begins immediately after it.
const LABEL_WIDTH: u16 = 12;
/// Width of the active-field marker (`"> "` / `"  "`) preceding each label.
const MARKER_WIDTH: u16 = 2;

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &App, keybindings: &KeyBindings) {
    let form = app.setup_form();
    let active_field = form.active_field();
    let mut lines = Vec::with_capacity(8);

    lines.push(Line::from(Span::styled(
        "Jira connection",
        Style::default()
            .fg(app.theme().accent_fg())
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
            Span::styled(marker, Style::default().fg(app.theme().accent_fg())),
            Span::styled(
                format!("{:width$}", field.label(), width = LABEL_WIDTH as usize),
                Style::default().fg(app.theme().subtle_fg()),
            ),
            Span::raw(display_value),
        ]));
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        keybindings.setup_hint_text(),
        Style::default().fg(app.theme().muted_fg()),
    )));

    frame.render_widget(Paragraph::new(lines), area);

    let cursor_offset = form.cursors()[form.active_field_idx()];
    let cursor_x = area.x + MARKER_WIDTH + LABEL_WIDTH + cursor_offset as u16;
    let cursor_y = area.y + 2 + active_field_idx as u16;
    frame.set_cursor_position(ratatui::layout::Position::new(cursor_x, cursor_y));
}

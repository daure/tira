use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    App, KeyBindings,
    components::generic::filter,
};

pub(super) fn render_filter(frame: &mut Frame<'_>, area: Rect, app: &App, _keybindings: &KeyBindings) {
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

pub(super) fn details_trigger_text(app: &App) -> String {
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

pub(super) fn render_details_trigger(frame: &mut Frame<'_>, area: Rect, app: &App, text: &str) {
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

pub(super) fn render_group_trigger(frame: &mut Frame<'_>, area: Rect, app: &App) {
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

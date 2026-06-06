pub mod chrome;
pub mod layout;
mod overlays;
mod project_switcher;
pub(crate) mod scrollbar;
mod setup;
pub(crate) mod style;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::Paragraph,
};

use crate::{App, Screen, components::jira::issue_list};

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    let [frame_area, status_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .areas(frame.area());

    let outer = chrome::tabbed_frame(app.active_tab_index(), app.tabs_view_mode());
    let inner = outer.inner(frame_area);
    frame.render_widget(outer, frame_area);

    match app.screen() {
        Screen::Setup => setup::render(frame, inner, app),
        Screen::Main => render_main(frame, inner, app),
    }

    if app.is_command_log_open() {
        overlays::render_command_log_dialog(frame, inner, app.command_log_entries());
    }

    overlays::render_notifications(frame, inner, app);

    frame.render_widget(chrome::status_bar(app, status_area.width), status_area);
    project_switcher::render(frame, status_area, app);
}

fn render_main(frame: &mut Frame<'_>, area: Rect, app: &App) {
    match app.active_tab() {
        "List" => issue_list::render(frame, area, app),
        "Board" => render_empty_tab(frame, area, "Board"),
        tab => render_empty_tab(frame, area, tab),
    }
}

fn render_empty_tab(frame: &mut Frame<'_>, area: Rect, tab: &str) {
    let body = Paragraph::new(Line::from(tab))
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(body, area);
}

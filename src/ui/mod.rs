mod assignee_selector;
mod board;
mod board_group_picker;
pub mod chrome;
pub mod layout;
mod logo;
mod overlays;
pub(crate) mod project_switcher;
mod quick_switcher;
pub(crate) mod scrollbar;
pub(crate) mod selector;
mod setup;
pub(crate) mod style;
pub mod theme;
mod theme_picker;
mod timeline;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
};

use crate::{
    App, ApplicationTab, KeyBindings, Screen,
    components::jira::{issue_list, ticket_dialog},
};

pub fn draw(frame: &mut Frame<'_>, app: &App, keybindings: &KeyBindings) {
    let [frame_area, status_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .areas(frame.area());

    if app.is_loading_splash() {
        logo::render(frame, frame_area, app.anim_elapsed(), app.theme());
        frame.render_widget(
            chrome::status_bar(app, keybindings, status_area.width),
            status_area,
        );
        return;
    }

    let outer = chrome::tabbed_frame(app.active_tab_index(), app.tabs_view_mode(), app.theme());
    let inner = outer.inner(frame_area);
    frame.render_widget(outer, frame_area);

    match app.screen() {
        Screen::Setup => setup::render(frame, inner, app, keybindings),
        Screen::Main => render_main(frame, inner, app, keybindings),
    }

    frame.render_widget(
        chrome::status_bar(app, keybindings, status_area.width),
        status_area,
    );
    quick_switcher::render(frame, inner, app, keybindings);
    theme_picker::render(frame, inner, app, keybindings);
    project_switcher::render(frame, inner, app, keybindings);
    assignee_selector::render(frame, inner, app, keybindings);
    board_group_picker::render(frame, inner, app, keybindings);

    if app.is_command_log_open() {
        overlays::render_command_log_dialog(frame, inner, app);
    }
    if app.is_sprint_details_open() {
        overlays::render_sprint_details_dialog(frame, inner, app);
    }
    if let Some(ticket) = app.ticket_dialog() {
        ticket_dialog::render(frame, inner, ticket, app.theme());
    }
    if app.is_help_open() {
        overlays::render_help_dialog(frame, inner, app, keybindings);
    }

    overlays::render_notifications(frame, inner, app);
}
fn render_main(frame: &mut Frame<'_>, area: Rect, app: &App, keybindings: &KeyBindings) {
    match app.active_tab() {
        ApplicationTab::List => issue_list::render(frame, area, app, keybindings),
        ApplicationTab::Board => board::render(frame, area, app, keybindings),
        ApplicationTab::Timeline => timeline::render(frame, area, app, keybindings),
    }
}

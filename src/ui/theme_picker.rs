use ratatui::{Frame, layout::Rect};

use crate::{App, KeyBindings};

use super::selector;

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &App, keybindings: &KeyBindings) {
    let Some(dropdown) = app.theme_dropdown() else {
        return;
    };
    selector::render_single_select(
        frame,
        area,
        "Theme",
        dropdown,
        app.theme(),
        keybindings,
        34,
        12,
        !app.is_help_open() && !app.is_command_log_open(),
    );
}

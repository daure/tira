use ratatui::{Frame, layout::Rect};

use crate::{App, KeyBindings};

use super::selector;

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &App, keybindings: &KeyBindings) {
    let Some(dropdown) = app.assignee_dropdown() else {
        return;
    };
    selector::render_single_select(
        frame,
        area,
        "Assignee",
        dropdown,
        app.theme(),
        keybindings,
        32,
        10,
        app.dropdown_cursor_visible(),
    );
}

use ratatui::{Frame, layout::Rect};

use crate::{App, KeyBindings};

use super::selector;

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &App, keybindings: &KeyBindings) {
    let Some(dropdown) = app.quick_switcher() else {
        return;
    };
    selector::render_single_select(
        frame,
        area,
        "Quick actions",
        dropdown,
        app.theme(),
        keybindings,
        34,
        10,
        app.dropdown_cursor_visible(),
    );
}

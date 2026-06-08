use ratatui::{Frame, layout::Rect};

use crate::{App, BoardGrouping, KeyBindings};

use super::selector;

impl selector::HasShortcut for BoardGrouping {
    fn shortcut(&self, _keybindings: &KeyBindings) -> Option<String> {
        None
    }
}

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &App, keybindings: &KeyBindings) {
    let Some(dropdown) = app.board_group_dropdown() else {
        return;
    };
    selector::render_single_select(
        frame,
        area,
        "Group",
        dropdown,
        app.theme(),
        keybindings,
        28,
        6,
        !app.is_help_open() && !app.is_command_log_open(),
    );
}

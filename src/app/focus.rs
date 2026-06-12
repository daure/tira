//! Derived input-mode classification for the app.
//!
//! Visual mode is DEFERRED: no item multi-selection exists yet, so the
//! constitution §5 three-mode clause is aspirational until item multi-select
//! ships. Only `Normal` and `Input` are modeled here.
//!
//! A stored `Focus` enum is also DEFERRED: focus is fully derivable from the
//! existing state today, so introducing a stored field would be redundant.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputMode {
    Normal,
    Input,
}

impl App {
    pub(crate) fn input_mode(&self) -> InputMode {
        if self.screen == Screen::Setup
            || self.any_overlay_filter_focused()
            || self.is_board_filter_focused()
            || self.is_timeline_filter_focused()
        {
            InputMode::Input
        } else {
            InputMode::Normal
        }
    }

    fn any_overlay_filter_focused(&self) -> bool {
        self.is_filter_focused()
            || self.is_column_dropdown_filter_focused()
            || self
                .overlay
                .as_ref()
                .is_some_and(Overlay::is_filter_focused)
    }

    /// Whether a printable key should be treated as text input.
    pub(crate) fn text_input_focused(&self) -> bool {
        self.is_input_focused()
    }

    /// Whether an open overlay (or focused filter) should capture keys, blocking
    /// the bare quit binding. Keys on dropdown OPEN (not filter focus) and
    /// deliberately OMITS the board-group dropdown.
    pub(crate) fn overlay_captures_keys(&self) -> bool {
        match self.screen {
            Screen::Setup => true,
            Screen::Main
                if self.overlay.is_some()
                    && !matches!(self.overlay, Some(Overlay::BoardGroup(_))) =>
            {
                true
            }
            Screen::Main if self.filtered_tree.is_column_dropdown_open() => true,
            Screen::Main if self.filtered_tree.is_filter_focused() => true,
            Screen::Main if self.board_filter.is_focused() => true,
            Screen::Main if self.timeline.is_filter_focused() => true,
            _ => false,
        }
    }
}

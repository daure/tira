use ratatui::{
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::ui::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterAction {
    Text(char),
    Backspace,
    Exit,
    Submit,
    Quit,
    None,
    MoveCursorStart,
    MoveCursorEnd,
    Clear,
    MoveCursorWordLeft,
    MoveCursorWordRight,
    DeleteWordLeft,
    DeleteWordRight,
    MoveCursorLeft,
    MoveCursorRight,
    DeleteToEnd,
    DeleteToStart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterEvent {
    Changed,
    Blurred,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FilterState {
    value: String,
    focused: bool,
    cursor: usize,
}

impl FilterState {
    pub fn value(&self) -> &str {
        &self.value
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn focus(&mut self) {
        self.focused = true;
        self.cursor = self.value.chars().count();
    }

    pub fn clear_focus(&mut self) {
        self.focused = false;
    }

    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
        self.focused = false;
    }

    pub fn dispatch(&mut self, action: FilterAction) -> Option<FilterEvent> {
        use super::input;
        match action {
            FilterAction::Text(c) => {
                input::insert_char(&mut self.value, &mut self.cursor, c);
                Some(FilterEvent::Changed)
            }
            FilterAction::Backspace => {
                input::delete_backwards(&mut self.value, &mut self.cursor);
                Some(FilterEvent::Changed)
            }
            FilterAction::Exit | FilterAction::Submit => {
                self.focused = false;
                Some(FilterEvent::Blurred)
            }
            FilterAction::Quit | FilterAction::None => None,
            FilterAction::MoveCursorStart => {
                self.cursor = 0;
                None
            }
            FilterAction::MoveCursorEnd => {
                self.cursor = self.value.chars().count();
                None
            }
            FilterAction::Clear => {
                self.value.clear();
                self.cursor = 0;
                Some(FilterEvent::Changed)
            }
            FilterAction::MoveCursorWordLeft => {
                input::move_word_left(&self.value, &mut self.cursor);
                None
            }
            FilterAction::MoveCursorWordRight => {
                input::move_word_right(&self.value, &mut self.cursor);
                None
            }
            FilterAction::DeleteWordLeft => {
                input::delete_word_left(&mut self.value, &mut self.cursor);
                Some(FilterEvent::Changed)
            }
            FilterAction::DeleteWordRight => {
                input::delete_word_right(&mut self.value, self.cursor);
                Some(FilterEvent::Changed)
            }
            FilterAction::MoveCursorLeft => {
                input::move_left(&mut self.cursor);
                None
            }
            FilterAction::MoveCursorRight => {
                input::move_right(&self.value, &mut self.cursor);
                None
            }
            FilterAction::DeleteToEnd => {
                input::delete_to_end(&mut self.value, self.cursor);
                Some(FilterEvent::Changed)
            }
            FilterAction::DeleteToStart => {
                input::delete_to_start(&mut self.value, &mut self.cursor);
                Some(FilterEvent::Changed)
            }
        }
    }
}

pub fn render_icon<'a>(state: &FilterState, theme: &Theme) -> Paragraph<'a> {
    Paragraph::new(Line::from(Span::styled("", prefix_style(state, theme))))
}

pub fn render_text<'a>(state: &'a FilterState, theme: &Theme) -> Paragraph<'a> {
    let value = if state.value.is_empty() {
        Span::styled("Search", Style::default().fg(theme.muted_fg()))
    } else {
        Span::raw(state.value.as_str())
    };

    Paragraph::new(Line::from(value))
}

fn prefix_style(state: &FilterState, theme: &Theme) -> Style {
    if state.focused {
        Style::default().fg(theme.accent_fg())
    } else {
        Style::default().fg(theme.muted_fg())
    }
}

#[cfg(test)]
mod tests {
    use super::{FilterAction, FilterEvent, FilterState};

    #[test]
    fn text_and_backspace_update_value() {
        let mut filter = FilterState::default();

        filter.dispatch(FilterAction::Text('c'));
        filter.dispatch(FilterAction::Text('a'));
        filter.dispatch(FilterAction::Backspace);

        assert_eq!(filter.value(), "c");
    }

    #[test]
    fn submit_emits_blurred_event_and_clears_focus() {
        let mut filter = FilterState::default();
        filter.focus();

        let event = filter.dispatch(FilterAction::Submit);

        assert_eq!(event, Some(FilterEvent::Blurred));
        assert!(!filter.is_focused());
    }

    #[test]
    fn exit_emits_blurred_event_and_keeps_value() {
        let mut filter = FilterState::default();
        filter.focus();
        filter.dispatch(FilterAction::Text('c'));

        let event = filter.dispatch(FilterAction::Exit);

        assert_eq!(event, Some(FilterEvent::Blurred));
        assert_eq!(filter.value(), "c");
        assert!(!filter.is_focused());
    }
}

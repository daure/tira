use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, ListItem},
};

use crate::{App, KeyBindings};

use super::selector;

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &App, keybindings: &KeyBindings) {
    let Some(dropdown) = app.project_dropdown() else {
        return;
    };
    selector::render_single_select(
        frame,
        area,
        "Project",
        dropdown,
        app.theme(),
        keybindings,
        32,
        10,
        app.dropdown_cursor_visible(),
    );
}

pub(crate) fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

pub(crate) fn dropdown_block(
    title: &'static str,
    theme: &crate::ui::theme::Theme,
) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_fg()))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                title,
                Style::default()
                    .fg(theme.accent_fg())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]))
}

pub(crate) fn no_results_item(theme: &crate::ui::theme::Theme) -> ListItem<'static> {
    ListItem::new(Line::from(Span::styled(
        "No results",
        Style::default().fg(theme.muted_fg()),
    )))
}

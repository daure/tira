use ratatui::{
    layout::Alignment,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{App, ApplicationTab, KeyBindings, app::app_tabs, components::generic::tabs};

pub fn tabbed_frame(
    active_tab: usize,
    view_mode: tabs::TabsViewMode,
    theme: &crate::ui::theme::Theme,
) -> ratatui::widgets::Block<'static> {
    tabs::tabbed_frame(&app_tabs(), active_tab, view_mode, theme)
}

/// The compact toolbar hint shown when a board/list toolbar is too narrow to
/// carry its inline triggers: the configured help binding (accent colour)
/// followed by " shortcuts" (muted), right-aligned to sit where the triggers
/// were.
pub fn shortcuts_hint(
    keybindings: &KeyBindings,
    theme: &crate::ui::theme::Theme,
) -> Paragraph<'static> {
    let line = Line::from(vec![
        Span::styled(
            keybindings.open_help_label(),
            Style::default()
                .fg(theme.accent_fg())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" shortcuts ", Style::default().fg(theme.muted_fg())),
    ]);
    Paragraph::new(line).alignment(Alignment::Right)
}

pub fn border_separator(width: u16, theme: &crate::ui::theme::Theme) -> Paragraph<'static> {
    let line = if width < 2 {
        "─".repeat(width as usize)
    } else {
        format!("├{}┤", "─".repeat(width.saturating_sub(2) as usize))
    };
    Paragraph::new(Line::from(Span::styled(
        line,
        Style::reset().fg(theme.border_fg()),
    )))
}

pub fn status_bar(app: &App, keybindings: &KeyBindings, width: u16) -> Paragraph<'static> {
    let theme = app.theme();
    let (mode_str, mode_bg) = if app.is_board_move_mode() {
        ("  MOVE  ", theme.status_project_bg())
    } else if app.is_input_focused() {
        (" INSERT ", theme.status_input_bg())
    } else {
        (" NORMAL ", theme.status_normal_bg())
    };

    let sep_right = "";
    let sep_left = "";
    let left_spans = vec![
        Span::styled(
            mode_str,
            Style::default()
                .fg(theme.status_mode_fg())
                .bg(mode_bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            sep_right,
            Style::default().fg(mode_bg).bg(theme.status_bar_bg()),
        ),
    ];

    let active_context = if app.is_loading() {
        format!(
            " {} {} · {} ",
            app.spinner_glyph(),
            app.status(),
            status_hint(app, keybindings)
        )
    } else {
        format!(" {} · {} ", app.status(), status_hint(app, keybindings))
    };
    let time_str = format!("  {} ", chrono::Local::now().format("%H:%M"));
    let project = app.current_project();
    let project_str = if project.is_empty() {
        String::from(" - ")
    } else {
        format!(" {} ", project)
    };

    let right_spans = vec![
        Span::styled(
            sep_left,
            Style::default()
                .fg(theme.status_project_bg())
                .bg(theme.status_bar_bg()),
        ),
        Span::styled(
            project_str,
            Style::default()
                .fg(theme.status_project_fg())
                .bg(theme.status_project_bg())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            sep_left,
            Style::default()
                .fg(theme.status_time_bg())
                .bg(theme.status_project_bg()),
        ),
        Span::styled(
            time_str,
            Style::default()
                .fg(theme.status_time_fg())
                .bg(theme.status_time_bg())
                .add_modifier(Modifier::BOLD),
        ),
    ];

    let left_len: usize = left_spans.iter().map(|s| s.content.chars().count()).sum();
    let right_len: usize = right_spans.iter().map(|s| s.content.chars().count()).sum();
    let context_width = (width as usize).saturating_sub(left_len + right_len);
    let context_text = super::layout::truncate_with_ellipsis(&active_context, context_width);
    let middle_spaces = context_width.saturating_sub(context_text.chars().count());

    let mut spans = left_spans;
    spans.push(Span::styled(
        context_text,
        Style::default()
            .fg(theme.status_text())
            .bg(theme.status_bar_bg()),
    ));
    spans.push(Span::styled(
        " ".repeat(middle_spaces),
        Style::default().bg(theme.status_bar_bg()),
    ));
    spans.extend(right_spans);

    Paragraph::new(Line::from(spans)).style(Style::default().bg(theme.status_bar_bg()))
}

fn status_hint(app: &App, keybindings: &KeyBindings) -> String {
    if app.is_command_log_open() {
        return keybindings.command_log_hint_text();
    }
    if app.is_ticket_dialog_open() {
        return keybindings.ticket_dialog_hint_text();
    }
    match app.screen() {
        crate::Screen::Setup => keybindings.setup_hint_text(),
        crate::Screen::Main if app.active_tab() == ApplicationTab::Board => {
            keybindings.board_hint_text()
        }
        crate::Screen::Main if app.active_tab() == ApplicationTab::Timeline => {
            keybindings.timeline_hint_text()
        }
        crate::Screen::Main => keybindings.list_hint_text(),
    }
}

extern crate chrono;

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{App, app::APP_TABS, components::generic::tabs};

pub fn tabbed_frame(
    active_tab: usize,
    view_mode: tabs::TabsViewMode,
) -> ratatui::widgets::Block<'static> {
    tabs::tabbed_frame(APP_TABS, active_tab, view_mode)
}

pub fn status_bar(app: &App, width: u16) -> Paragraph<'_> {
    let (mode_str, mode_bg) = if app.is_input_focused() {
        (" INSERT ", Color::Green)
    } else {
        (" NORMAL ", Color::Rgb(240, 220, 140)) // Yellow
    };
    let mode_fg = Color::Black;

    let bar_bg = Color::Rgb(20, 20, 20); // Dark background for status bar
    let sep_right = "";
    let sep_left = "";

    let left_spans = vec![
        Span::styled(
            mode_str,
            Style::default()
                .fg(mode_fg)
                .bg(mode_bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(sep_right, Style::default().fg(mode_bg).bg(bar_bg)),
    ];

    let time_str = format!("  {} ", chrono::Local::now().format("%H:%M"));
    let time_bg = Color::Rgb(240, 220, 140); // Yellow
    let time_fg = Color::Black;

    let project = app.current_project();
    let project_str = if project.is_empty() {
        String::from(" - ")
    } else {
        format!(" {} ", project)
    };
    let project_bg = Color::Rgb(150, 100, 200); // Purple/magenta
    let project_fg = Color::White;

    let right_spans = vec![
        Span::styled(sep_left, Style::default().fg(project_bg).bg(bar_bg)),
        Span::styled(
            project_str,
            Style::default()
                .fg(project_fg)
                .bg(project_bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(sep_left, Style::default().fg(time_bg).bg(project_bg)),
        Span::styled(
            time_str,
            Style::default()
                .fg(time_fg)
                .bg(time_bg)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    let left_len: usize = left_spans.iter().map(|s| s.content.chars().count()).sum();
    let right_len: usize = right_spans.iter().map(|s| s.content.chars().count()).sum();

    let total_width = width as usize;
    let middle_spaces = total_width.saturating_sub(left_len + right_len);
    let middle_span = Span::styled(" ".repeat(middle_spaces), Style::default().bg(bar_bg));

    let mut final_spans = left_spans;
    final_spans.push(middle_span);
    final_spans.extend(right_spans);

    Paragraph::new(Line::from(final_spans)).style(Style::default().bg(bar_bg))
}

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::ui::theme::{Theme, prefers_plain_icons};

use super::lanes::BoardHeading;

const NERD_COLLAPSED_ICON: &str = "";
const NERD_EXPANDED_ICON: &str = "";

pub(super) fn board_heading_line(
    name: &str,
    count: usize,
    collapsed: bool,
    selected: bool,
    theme: &Theme,
    header_width: u16,
) -> Line<'static> {
    let marker = if collapsed {
        collapsed_icon()
    } else {
        expanded_icon()
    };
    let suffix = if count == 1 {
        "work item"
    } else {
        "work items"
    };
    let header_text = format!(" {marker} {name} ({count} {suffix}) ");
    let text_len = header_text.chars().count();
    let fill_char = if selected { "═" } else { "─" };
    let border_style = if selected {
        Style::default()
            .fg(theme.accent_fg())
            .bg(theme.selected_bg())
    } else {
        Style::default().fg(theme.border_fg())
    };
    let text_style = if selected {
        Style::default()
            .fg(theme.selected_fg())
            .bg(theme.selected_bg())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.subtle_fg())
    };
    let filler_len = (header_width as usize).saturating_sub(text_len);
    Line::from(vec![
        Span::styled(header_text, text_style),
        Span::styled(fill_char.repeat(filler_len), border_style),
    ])
}

/// Returns the stacked sticky headings (with their level) for the row at the
/// top of the viewport: the current swimlane, group and column-title context.
pub(super) fn sticky_headings(
    headings: &[BoardHeading],
    scroll_offset: usize,
) -> Vec<(usize, Line<'static>)> {
    let mut sticky = Vec::new();
    let mut min_y = 0;
    for level in 0..=2 {
        if let Some(heading) = headings
            .iter()
            .take_while(|heading| heading.y <= scroll_offset)
            .filter(|heading| heading.level == level && heading.y >= min_y)
            .last()
        {
            min_y = heading.y;
            sticky.push((level, heading.line.clone()));
        }
    }
    sticky
}

fn collapsed_icon() -> &'static str {
    if prefers_plain_icons() {
        ">"
    } else {
        NERD_COLLAPSED_ICON
    }
}

fn expanded_icon() -> &'static str {
    if prefers_plain_icons() {
        "v"
    } else {
        NERD_EXPANDED_ICON
    }
}

use ratatui::{
    style::{Modifier, Style},
    text::Span,
};

use crate::ui::{
    style,
    theme::{Theme, prefers_plain_icons},
};

pub fn spans(
    theme: &Theme,
    priority: &str,
    filter: &str,
    base_style: Style,
    minimal: bool,
) -> Vec<Span<'static>> {
    let mut spans = vec![Span::styled(icon(priority), icon_style(theme, priority))];
    if !minimal && !priority.is_empty() {
        spans.push(Span::raw(" "));
        spans.extend(style::highlighted_spans_owned(
            theme, priority, filter, base_style,
        ));
    }
    spans
}

pub fn display_width(priority: &str, minimal: bool) -> usize {
    let icon = icon(priority);
    if minimal {
        icon.chars().count()
    } else {
        let gap = usize::from(!priority.is_empty());
        icon.chars().count() + gap + priority.chars().count()
    }
}

pub fn icon(priority: &str) -> &'static str {
    if prefers_plain_icons() {
        return match priority {
            "Highest" => "⌃",
            "High" => "^",
            "Medium" => "=",
            "Low" => "v",
            "Lowest" => "⌄",
            _ => "•",
        };
    }

    match priority {
        "Highest" => "󰄿",
        "High" => "󰅃",
        "Medium" => "󰇼",
        "Low" => "󰅀",
        "Lowest" => "󰄼",
        _ => "•",
    }
}

fn icon_style(theme: &Theme, priority: &str) -> Style {
    let color = match priority {
        "Highest" => theme.error_fg(),
        "High" => theme.error_fg(),
        "Medium" => theme.warning_fg(),
        "Low" => theme.accent_fg(),
        "Lowest" => theme.accent_fg(),
        _ => theme.muted_fg(),
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

#[cfg(test)]
mod tests {
    use super::icon;
    #[test]
    fn highest_and_lowest_use_double_icons() {
        assert_eq!(icon("Highest"), "󰄿");
        assert_eq!(icon("High"), "󰅃");
        assert_eq!(icon("Medium"), "󰇼");
        assert_eq!(icon("Low"), "󰅀");
        assert_eq!(icon("Lowest"), "󰄼");
    }
}

use ratatui::{style::Style, text::Span};

use crate::{
    components::generic::tree,
    ui::theme::{Theme, prefers_plain_icons},
};

pub fn spans<'a>(
    theme: &Theme,
    key: &'a str,
    kind: &str,
    filter: &str,
    key_style: Style,
) -> Vec<Span<'a>> {
    let mut spans = vec![
        Span::styled(icon(kind), icon_style(theme, kind)),
        Span::raw(" "),
    ];
    spans.extend(highlighted_key_spans(theme, key, filter, key_style));
    spans
}

pub fn icon(kind: &str) -> &'static str {
    if prefers_plain_icons() {
        return match kind {
            "Epic" => "⚡",
            "Story" => "▣",
            "Task" => "✓",
            "Subtask" | "Sub-task" => "⧉",
            "Bug" => "!",
            _ => "•",
        };
    }

    match kind {
        "Epic" => "",
        "Story" => "",
        "Task" => "",
        "Subtask" | "Sub-task" => "",
        "Bug" => "",
        _ => "",
    }
}

fn icon_style(theme: &Theme, kind: &str) -> Style {
    Style::default().fg(theme.issue_type_fg(kind))
}

fn highlighted_key_spans<'a>(
    theme: &Theme,
    key: &'a str,
    filter: &str,
    base_style: Style,
) -> Vec<Span<'a>> {
    let indices = tree::fuzzy_indices(key, filter);
    if indices.is_empty() {
        return vec![Span::styled(key, base_style)];
    }

    let mut matched = indices.into_iter().peekable();
    let mut spans = Vec::new();
    let mut segment_start = 0;
    let mut current_style = base_style;

    for (char_index, (byte_start, ch)) in key.char_indices().enumerate() {
        let is_match = matched
            .peek()
            .is_some_and(|match_index| *match_index == char_index);
        if is_match {
            matched.next();
        }
        let next_style = if is_match {
            Style::default()
                .fg(theme.highlight_fg())
                .bg(theme.highlight_bg())
        } else {
            base_style
        };

        if byte_start > segment_start && next_style != current_style {
            spans.push(Span::styled(&key[segment_start..byte_start], current_style));
            segment_start = byte_start;
        }
        current_style = next_style;

        if byte_start + ch.len_utf8() == key.len() {
            spans.push(Span::styled(&key[segment_start..], current_style));
        }
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::Theme;

    #[test]
    fn renders_icon_space_and_work_item_key() {
        let spans = spans(&Theme::default(), "KAN-1", "Task", "", Style::default());

        assert_eq!(spans[0].content.as_ref(), "");
        assert_eq!(spans[1].content.as_ref(), " ");
        assert_eq!(spans[2].content.as_ref(), "KAN-1");
    }

    #[test]
    fn subtask_uses_copy_icon() {
        let spans = spans(&Theme::default(), "KAN-2", "Subtask", "", Style::default());

        assert_eq!(spans[0].content.as_ref(), "");
        assert_eq!(icon("Sub-task"), "");
    }

    #[test]
    fn fallback_icon_is_book() {
        assert_eq!(icon("Custom"), "");
    }
}

use ratatui::{
    style::{Color, Style},
    text::Span,
};

use crate::components::generic::tree;

pub fn spans<'a>(key: &'a str, kind: &str, filter: &str, key_style: Style) -> Vec<Span<'a>> {
    let mut spans = vec![Span::styled(icon(kind), icon_style(kind)), Span::raw(" ")];
    spans.extend(highlighted_key_spans(key, filter, key_style));
    spans
}

pub fn icon(kind: &str) -> &'static str {
    match kind {
        "Epic" => "",
        "Story" => "",
        "Task" => "",
        "Subtask" | "Sub-task" => "",
        "Bug" => "",
        _ => "",
    }
}

fn icon_style(kind: &str) -> Style {
    match kind {
        "Epic" => Style::default().fg(Color::Rgb(150, 100, 200)),
        "Story" => Style::default().fg(Color::Rgb(100, 200, 100)),
        "Task" | "Subtask" | "Sub-task" => Style::default().fg(Color::Rgb(100, 150, 240)),
        "Bug" => Style::default().fg(Color::Rgb(240, 100, 100)),
        _ => Style::default().fg(Color::Rgb(220, 150, 80)),
    }
}

fn highlighted_key_spans<'a>(key: &'a str, filter: &str, base_style: Style) -> Vec<Span<'a>> {
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
            Style::default().fg(Color::Black).bg(Color::Yellow)
        } else {
            base_style
        };
        if next_style != current_style {
            if segment_start < byte_start {
                spans.push(Span::styled(&key[segment_start..byte_start], current_style));
            }
            segment_start = byte_start;
            current_style = next_style;
        }
        if byte_start + ch.len_utf8() == key.len() {
            spans.push(Span::styled(&key[segment_start..], current_style));
        }
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_icon_space_and_work_item_key() {
        let spans = spans("KAN-20", "Task", "", Style::default());

        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content.as_ref(), "");
        assert_eq!(spans[1].content.as_ref(), " ");
        assert_eq!(spans[2].content.as_ref(), "KAN-20");
    }

    #[test]
    fn subtask_uses_blue_copy_icon() {
        let spans = spans("KAN-21", "Subtask", "", Style::default());

        assert_eq!(spans[0].content.as_ref(), "");
        assert_eq!(spans[0].style, icon_style("Task"));
        assert_eq!(icon("Sub-task"), "");
    }

    #[test]
    fn fallback_icon_is_book() {
        assert_eq!(icon("Custom"), "");
    }
}

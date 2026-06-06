use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};

use crate::{
    TreeItem,
    components::{generic::tree, jira::work_item_key},
};

pub fn selected_row_style(is_selected: bool) -> Style {
    if is_selected {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    }
}

pub fn dropdown_option_label_style(is_selected: bool, is_focused: bool) -> Style {
    if is_focused {
        selected_row_style(true)
    } else if is_selected {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    }
}

pub fn single_select_dropdown_spans(
    label: &str,
    filter: &str,
    is_selected: bool,
    is_focused: bool,
    row_width: usize,
    label_style: Style,
) -> Vec<Span<'static>> {
    let chevron = if is_focused { "› " } else { "  " };
    let checkmark = if is_selected { "✓" } else { "" };
    let gap_before_checkmark = usize::from(is_selected);
    let reserved_width = chevron.chars().count() + gap_before_checkmark + checkmark.chars().count();
    let label_width = row_width.saturating_sub(reserved_width);
    let label = super::layout::truncate_with_ellipsis(label, label_width);
    let used_width = chevron.chars().count()
        + label.chars().count()
        + gap_before_checkmark
        + checkmark.chars().count();
    let gap = row_width.saturating_sub(used_width);

    let mut spans = vec![Span::styled(chevron, selected_row_style(is_focused))];
    spans.extend(highlighted_spans_owned(&label, filter, label_style));
    spans.push(Span::raw(" ".repeat(gap + gap_before_checkmark)));
    let checkmark_style = if is_focused {
        selected_row_style(true)
    } else {
        Style::default().fg(Color::White)
    };
    spans.push(Span::styled(checkmark, checkmark_style));
    spans
}

pub fn code_cell_spans<'a>(item: &'a TreeItem, filter: &str, base_style: Style) -> Vec<Span<'a>> {
    work_item_key::spans(&item.id, &item.kind, filter, base_style.fg(Color::Blue))
}

pub fn highlighted_spans_owned(text: &str, filter: &str, base_style: Style) -> Vec<Span<'static>> {
    highlighted_spans(text, filter, base_style)
        .into_iter()
        .map(|span| Span::styled(span.content.into_owned(), span.style))
        .collect()
}

pub fn highlighted_spans<'a>(text: &'a str, filter: &str, base_style: Style) -> Vec<Span<'a>> {
    let indices = tree::fuzzy_indices(text, filter);
    if indices.is_empty() {
        return vec![Span::styled(text, base_style)];
    }

    let mut matched = indices.into_iter().peekable();
    let mut spans = Vec::new();
    let mut segment_start = 0;
    let mut current_style = base_style;

    for (char_index, (byte_start, ch)) in text.char_indices().enumerate() {
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

        if byte_start > segment_start && next_style != current_style {
            spans.push(Span::styled(
                &text[segment_start..byte_start],
                current_style,
            ));
            segment_start = byte_start;
        }
        current_style = next_style;

        if byte_start + ch.len_utf8() == text.len() {
            spans.push(Span::styled(&text[segment_start..], current_style));
        }
    }

    spans
}

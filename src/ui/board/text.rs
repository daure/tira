use ratatui::{style::Style, text::Span};
use unicode_width::UnicodeWidthStr;

pub(super) fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

pub(super) fn bordered_line(left: char, fill: char, right: char, width: usize) -> String {
    if width <= 1 {
        return left.to_string();
    }
    format!("{left}{}{right}", fill.to_string().repeat(width - 2))
}

pub(super) fn wrapped_lines(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let next_len = if current.is_empty() {
            display_width(word)
        } else {
            display_width(&current) + 1 + display_width(word)
        };
        if next_len > width && !current.is_empty() {
            lines.push(current);
            current = word.to_owned();
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

pub(super) fn apply_background(spans: &mut [Span<'static>], base_style: Style) {
    let Some(bg) = base_style.bg else {
        return;
    };
    for span in spans {
        if span.style.bg.is_none() {
            span.style = span.style.bg(bg);
        }
    }
}

/// Calculates the maximum width for a column based on a slice of items and a header label.
pub fn max_column_width<T, F>(items: &[T], header: &str, f: F) -> u16
where
    F: Fn(&T) -> usize,
{
    let mut max_len = header.chars().count();
    for item in items {
        max_len = max_len.max(f(item));
    }
    max_len as u16
}

/// Truncates a string to fit within a given maximum width, adding "..." if it is truncated.
pub fn truncate_with_ellipsis(text: &str, max_width: usize) -> String {
    if text.chars().count() <= max_width {
        text.to_owned()
    } else {
        let mut truncated = text
            .chars()
            .take(max_width.saturating_sub(3))
            .collect::<String>();
        truncated.push_str("...");
        truncated
    }
}

/// Truncates a sequence of styled spans to fit within `max_width` display
/// columns, appending a styled "..." when content is dropped. Per-span styles
/// are preserved so callers keep their coloring (e.g. label brackets).
pub fn truncate_spans_with_ellipsis(
    spans: Vec<ratatui::text::Span<'static>>,
    max_width: usize,
) -> Vec<ratatui::text::Span<'static>> {
    use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

    let total: usize = spans.iter().map(|span| span.content.width()).sum();
    if total <= max_width {
        return spans;
    }

    let budget = max_width.saturating_sub(3);
    let mut out = Vec::with_capacity(spans.len() + 1);
    let mut used = 0usize;
    for span in spans {
        let width = span.content.width();
        if used + width <= budget {
            used += width;
            out.push(span);
            continue;
        }
        let remaining = budget - used;
        if remaining > 0 {
            let mut partial = String::new();
            let mut acc = 0usize;
            for ch in span.content.chars() {
                let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
                if acc + ch_width > remaining {
                    break;
                }
                acc += ch_width;
                partial.push(ch);
            }
            if !partial.is_empty() {
                out.push(ratatui::text::Span::styled(partial, span.style));
            }
        }
        out.push(ratatui::text::Span::styled("...", span.style));
        return out;
    }
    out
}

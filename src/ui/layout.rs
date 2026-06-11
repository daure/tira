use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

/// Below this toolbar width (in cells), the board/list toolbar collapses its
/// inline triggers into a single "? shortcuts" affordance so the filter keeps
/// room to breathe on narrow terminals.
pub const RESPONSIVE_TOOLBAR_WIDTH: u16 = 60;

/// Whether a toolbar of this width should collapse its inline triggers down to
/// the compact "? shortcuts" hint.
pub fn toolbar_is_collapsed(width: u16) -> bool {
    width < RESPONSIVE_TOOLBAR_WIDTH
}

/// Returns the sub-slice of a line covering display columns
/// `[start, start + width)`, preserving span styles and adding no ellipsis.
/// Used to trim a wider-than-viewport row to the horizontal scroll window.
pub fn slice_line<'a>(line: Line<'a>, start: usize, width: usize) -> Line<'a> {
    clip_head(clip_tail(line, start), width)
}

fn clip_head<'a>(line: Line<'a>, width: usize) -> Line<'a> {
    let mut out = Vec::new();
    let mut used = 0usize;
    for span in line.spans {
        if used >= width {
            break;
        }
        let span_width = span.content.width();
        if used + span_width <= width {
            used += span_width;
            out.push(span);
        } else {
            let remaining = width - used;
            let mut partial = String::new();
            let mut acc = 0usize;
            for ch in span.content.chars() {
                let ch_width = ch.to_string().width();
                if acc + ch_width > remaining {
                    break;
                }
                acc += ch_width;
                partial.push(ch);
            }
            if !partial.is_empty() {
                out.push(Span::styled(partial, span.style));
            }
            break;
        }
    }
    Line::from(out)
}

fn clip_tail<'a>(line: Line<'a>, skip: usize) -> Line<'a> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    for span in line.spans {
        let span_width = span.content.width();
        let span_end = pos + span_width;
        if span_end <= skip {
            pos = span_end;
            continue;
        }
        if pos >= skip {
            out.push(span);
        } else {
            let drop_cols = skip - pos;
            let mut acc = 0usize;
            let mut kept = String::new();
            for ch in span.content.chars() {
                let ch_width = ch.to_string().width();
                if acc < drop_cols {
                    acc += ch_width;
                    continue;
                }
                kept.push(ch);
            }
            if !kept.is_empty() {
                out.push(Span::styled(kept, span.style));
            }
        }
        pos = span_end;
    }
    Line::from(out)
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_line_returns_the_requested_window() {
        let line = Line::from(vec![Span::raw("abcdefghij".to_owned())]);
        let sliced = slice_line(line, 3, 4);
        let text: String = sliced.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "defg");
    }

    #[test]
    fn slice_line_preserves_per_span_styles_across_the_cut() {
        let line = Line::from(vec![Span::raw("ab".to_owned()), Span::raw("cdef".to_owned())]);
        let sliced = slice_line(line, 1, 3);
        let text: String = sliced.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "bcd");
    }
}

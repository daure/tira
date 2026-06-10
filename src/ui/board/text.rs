use ratatui::{
    style::Style,
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr;

/// Returns the sub-slice of a line covering display columns
/// `[start, start + width)`, preserving span styles and adding no ellipsis.
/// This is how the full board strip is trimmed to the horizontal viewport;
/// fractional column boundaries (mid-glide) simply land inside a span.
pub(super) fn slice_line(line: Line<'static>, start: usize, width: usize) -> Line<'static> {
    clip_head(clip_tail(line, start), width)
}

/// Keeps the leading `width` display columns of a line.
fn clip_head(line: Line<'static>, width: usize) -> Line<'static> {
    let mut out = Vec::new();
    let mut used = 0usize;
    for span in line.spans {
        if used >= width {
            break;
        }
        let span_width = display_width(span.content.as_ref());
        if used + span_width <= width {
            used += span_width;
            out.push(span);
        } else {
            let remaining = width - used;
            let mut partial = String::new();
            let mut acc = 0usize;
            for ch in span.content.chars() {
                let ch_width = display_width(ch.encode_utf8(&mut [0; 4]));
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

/// Drops the leading `skip` display columns from a line, keeping the rest.
fn clip_tail(line: Line<'static>, skip: usize) -> Line<'static> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    for span in line.spans {
        let span_width = display_width(span.content.as_ref());
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
                let ch_width = display_width(ch.encode_utf8(&mut [0; 4]));
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
        let line = Line::from(vec![
            Span::raw("ab".to_owned()),
            Span::raw("cdef".to_owned()),
        ]);
        let sliced = slice_line(line, 1, 3);
        let text: String = sliced.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "bcd");
    }
}

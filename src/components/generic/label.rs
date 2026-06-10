use ratatui::{style::Style, text::Span};

use crate::ui::theme::Theme;

pub fn spans(theme: &Theme, labels: &str, filter: &str, base_style: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for label in parse(labels) {
        spans.extend(single_label_spans(theme, label, filter, base_style));
    }
    spans
}

pub fn display_width(labels: &str) -> usize {
    parse(labels)
        .into_iter()
        .map(|label| label.chars().count() + 2)
        .sum()
}

/// Returns true when `labels` contains at least one renderable label.
pub fn has_labels(labels: &str) -> bool {
    !parse(labels).is_empty()
}

const BRACKET_LEFT: &str = "⟦";
const BRACKET_RIGHT: &str = "⟧";

fn single_label_spans(
    theme: &Theme,
    label: &str,
    filter: &str,
    base_style: Style,
) -> Vec<Span<'static>> {
    let bracket_style = base_style.fg(theme.muted_fg());
    let mut spans = vec![Span::styled(BRACKET_LEFT, bracket_style)];
    spans.extend(crate::ui::style::highlighted_spans_owned(
        theme,
        label,
        filter,
        base_style.fg(theme.status_text()),
    ));
    spans.push(Span::styled(BRACKET_RIGHT, bracket_style));
    spans
}

fn parse(labels: &str) -> Vec<&str> {
    labels
        .split(',')
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{display_width, parse};

    #[test]
    fn parses_labels_and_counts_width() {
        assert_eq!(
            parse("frontend, urgent, api"),
            vec!["frontend", "urgent", "api"]
        );
        assert_eq!(display_width("frontend, urgent"), 18);
    }
}

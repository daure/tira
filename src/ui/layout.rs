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

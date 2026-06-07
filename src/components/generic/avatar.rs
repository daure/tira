use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use ratatui::{
    style::{Modifier, Style},
    text::Span,
};

use crate::ui::{style, theme::Theme};

const PARTICLES: &[&str] = &[
    "da", "de", "del", "der", "di", "dos", "la", "le", "van", "von",
];

pub fn spans(theme: &Theme, name: &str, filter: &str, base_style: Style) -> Vec<Span<'static>> {
    let mut spans = bubble_spans(theme, name);
    if !name.is_empty() {
        spans.push(Span::raw(" "));
        spans.extend(style::highlighted_spans_owned(
            theme, name, filter, base_style,
        ));
    }
    spans
}

pub fn bubble_only_spans(theme: &Theme, name: &str) -> Vec<Span<'static>> {
    bubble_spans(theme, name)
}

pub fn bubble_width(name: &str) -> usize {
    format!("@{}", initials(name)).chars().count()
}

pub fn display_width(name: &str) -> usize {
    let bubble = bubble(name);
    let gap = usize::from(!name.is_empty());
    bubble.chars().count() + gap + name.chars().count()
}

pub fn initials(name: &str) -> String {
    let tokens = name
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        return String::from("??");
    }
    if tokens.len() == 1 {
        return take_initials(tokens[0], 2);
    }

    let first = first_grapheme(tokens[0]);
    let last = tokens
        .iter()
        .rev()
        .find(|token| !is_particle(token))
        .copied()
        .unwrap_or(tokens[tokens.len() - 1]);
    format!(
        "{}{}",
        first.to_uppercase(),
        first_grapheme(last).to_uppercase()
    )
}

fn bubble_spans(theme: &Theme, name: &str) -> Vec<Span<'static>> {
    let palette = avatar_palette(theme);
    let (_, avatar) = palette[hash_index(name) % palette.len()];
    let text_style = Style::default().fg(avatar).add_modifier(Modifier::BOLD);
    vec![Span::styled(format!("@{}", initials(name)), text_style)]
}

fn bubble(name: &str) -> String {
    format!("@{}", initials(name))
}

fn avatar_palette(theme: &Theme) -> [(ratatui::style::Color, ratatui::style::Color); 6] {
    [
        (theme.status_project_fg(), theme.status_project_bg()),
        (theme.status_time_fg(), theme.status_time_bg()),
        (theme.status_text(), theme.status_input_bg()),
        (theme.highlight_fg(), theme.accent_fg()),
        (theme.highlight_fg(), theme.success_fg()),
        (theme.highlight_fg(), theme.issue_type_fg("Task")),
    ]
}

fn hash_index(name: &str) -> usize {
    let normalized = name
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    let mut hasher = DefaultHasher::new();
    normalized.hash(&mut hasher);
    hasher.finish() as usize
}

fn is_particle(token: &str) -> bool {
    let lowercase = token.to_ascii_lowercase();
    PARTICLES.contains(&lowercase.as_str())
}

fn take_initials(token: &str, count: usize) -> String {
    token
        .chars()
        .take(count)
        .flat_map(char::to_uppercase)
        .collect()
}

fn first_grapheme(token: &str) -> String {
    token
        .chars()
        .next()
        .map(|ch| ch.to_string())
        .unwrap_or_else(|| String::from("?"))
}

#[cfg(test)]
mod tests {
    use super::initials;

    #[test]
    fn initials_use_first_and_last_significant_name_parts() {
        assert_eq!(initials("Johan van der Brink"), "JB");
        assert_eq!(initials("Marlo Vlietstra"), "MV");
        assert_eq!(initials("Madonna"), "MA");
    }
}

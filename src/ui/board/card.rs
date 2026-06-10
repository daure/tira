use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::{
    components::{
        generic::{avatar, label, priority},
        jira::work_item_key,
    },
    services::jira::IssueSummary,
    ui::{
        layout::truncate_spans_with_ellipsis,
        layout::truncate_with_ellipsis,
        theme::Theme,
    },
};

use super::text::{apply_background, bordered_line, display_width, wrapped_lines};

pub(super) fn issue_card_lines(
    issue: &IssueSummary,
    selected: bool,
    theme: &Theme,
    width: u16,
    search: &str,
) -> Vec<Line<'static>> {
    let width = width as usize;
    if width < 8 {
        return vec![Line::from(Span::styled(
            truncate_with_ellipsis(&issue.key, width),
            Style::default().fg(theme.accent_fg()),
        ))];
    }

    let border_style = card_border_style(selected, theme);
    let content_style = card_content_style(selected, theme);
    let inner_width = width.saturating_sub(2);
    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled(
        card_top_border(width, selected),
        border_style,
    )));

    for summary_line in wrapped_lines(&issue.summary, inner_width)
        .into_iter()
        .take(3)
    {
        lines.push(card_highlighted_content_line(
            &summary_line,
            search,
            inner_width,
            selected,
            border_style,
            content_style,
            theme,
        ));
    }

    if let Some(epic) = issue.field_values.get("epic_summary") {
        let epic_icon = work_item_key::icon("Epic");
        let epic = format!(
            "{epic_icon} {}",
            truncate_with_ellipsis(
                epic,
                inner_width.saturating_sub(epic_icon.chars().count() + 1)
            )
        );
        lines.push(card_highlighted_content_line(
            &epic,
            search,
            inner_width,
            selected,
            border_style,
            content_style
                .fg(theme.accent_fg())
                .add_modifier(Modifier::BOLD),
            theme,
        ));
    }

    if let Some(labels) = issue.field_values.get("labels")
        && label::has_labels(labels)
    {
        let mut spans = truncate_spans_with_ellipsis(
            label::spans(theme, labels, search, content_style),
            inner_width,
        );
        apply_background(&mut spans, content_style);
        lines.push(card_content_spans(
            spans,
            inner_width,
            selected,
            border_style,
            content_style,
        ));
    }

    if let Some(due_date) = issue.field_values.get("dueDate") {
        let due = format!(" {due_date}");
        let due = truncate_with_ellipsis(&due, inner_width);
        lines.push(card_highlighted_content_line(
            &due,
            search,
            inner_width,
            selected,
            border_style,
            content_style.fg(theme.muted_fg()),
            theme,
        ));
    }

    lines.push(card_bottom_border(issue, width, selected, theme, search));
    lines
}

fn card_highlighted_content_line(
    text: &str,
    search: &str,
    inner_width: usize,
    selected: bool,
    border_style: Style,
    content_style: Style,
    theme: &Theme,
) -> Line<'static> {
    let mut spans = crate::ui::style::highlighted_spans_owned(theme, text, search, content_style);
    apply_background(&mut spans, content_style);
    card_content_spans(spans, inner_width, selected, border_style, content_style)
}

fn card_content_spans(
    spans: Vec<Span<'static>>,
    inner_width: usize,
    selected: bool,
    border_style: Style,
    content_style: Style,
) -> Line<'static> {
    let side = if selected { "║" } else { "│" };
    let used = spans
        .iter()
        .map(|span| display_width(span.content.as_ref()))
        .sum::<usize>();
    let pad = inner_width.saturating_sub(used);
    let mut line_spans = Vec::with_capacity(spans.len() + 3);
    line_spans.push(Span::styled(side, border_style));
    line_spans.extend(spans);
    line_spans.push(Span::styled(" ".repeat(pad), content_style));
    line_spans.push(Span::styled(side, border_style));
    Line::from(line_spans)
}

fn card_top_border(width: usize, selected: bool) -> String {
    let (left, fill, right) = if selected {
        ('╔', '═', '╗')
    } else {
        ('┌', '─', '┐')
    };
    bordered_line(left, fill, right, width)
}

fn card_bottom_border(
    issue: &IssueSummary,
    width: usize,
    selected: bool,
    theme: &Theme,
    search: &str,
) -> Line<'static> {
    let (left, fill, right) = if selected {
        ("╚", "═", "╝")
    } else {
        ("└", "─", "┘")
    };
    let border_style = card_border_style(selected, theme);
    let content_style = card_content_style(selected, theme);
    let priority_name = issue
        .field_values
        .get("priorityName")
        .map(String::as_str)
        .unwrap_or("");
    let assignee = issue.field_values.get("assignee").map(String::as_str);
    let work_icon = work_item_key::icon(&issue.issue_type);
    let work_key_left_pad = " ";
    let key_segment = format!(" {}", issue.key);
    let work_key_right_pad = " ";
    let priority_left_pad = " ";
    let priority_right_pad = " ";
    let assignee_right_pad = " ";
    let avatar_width = assignee.map_or(0, avatar::bubble_width);
    let assignee_segment_width = if assignee.is_some() {
        avatar_width + display_width(assignee_right_pad)
    } else {
        0
    };
    let priority_width = display_width(priority::icon(priority_name));
    let fixed_width = display_width(left)
        + display_width(work_key_left_pad)
        + display_width(work_icon)
        + display_width(&key_segment)
        + display_width(work_key_right_pad)
        + display_width(priority_left_pad)
        + priority_width
        + display_width(priority_right_pad)
        + assignee_segment_width
        + display_width(right);
    let filler = width.saturating_sub(fixed_width);
    let mut priority_spans = priority::spans(
        theme,
        priority_name,
        "",
        content_style.fg(theme.muted_fg()),
        true,
    );
    apply_background(&mut priority_spans, content_style);

    let mut spans = vec![
        Span::styled(left.to_owned(), border_style),
        Span::styled(work_key_left_pad, content_style),
        Span::styled(
            work_icon.to_owned(),
            content_style.fg(theme.issue_type_fg(&issue.issue_type)),
        ),
    ];
    spans.push(Span::styled(" ", content_style));
    let mut key_spans = crate::ui::style::highlighted_spans_owned(
        theme,
        &issue.key,
        search,
        content_style.fg(theme.accent_fg()),
    );
    apply_background(&mut key_spans, content_style);
    spans.extend(key_spans);
    spans.push(Span::styled(work_key_right_pad, content_style));
    spans.push(Span::styled(fill.repeat(filler), border_style));
    spans.push(Span::styled(priority_left_pad, content_style));
    spans.extend(priority_spans);
    spans.push(Span::styled(priority_right_pad, content_style));
    if let Some(assignee) = assignee {
        let mut avatar_spans = highlighted_avatar_spans(theme, assignee, search, content_style);
        apply_background(&mut avatar_spans, content_style);
        spans.extend(avatar_spans);
        spans.push(Span::styled(assignee_right_pad, content_style));
    }
    spans.push(Span::styled(right.to_owned(), border_style));
    Line::from(spans)
}

fn highlighted_avatar_spans(
    theme: &Theme,
    assignee: &str,
    search: &str,
    content_style: Style,
) -> Vec<Span<'static>> {
    let mut spans = avatar::bubble_only_spans(theme, assignee);
    let search = search.trim().to_ascii_lowercase();
    if search.is_empty() {
        return spans;
    }
    let initials = avatar::initials(assignee).to_ascii_lowercase();
    let bubble = format!("@{initials}");
    let assignee = assignee.to_ascii_lowercase();
    if assignee.contains(&search) || initials.contains(&search) || bubble.contains(&search) {
        for span in &mut spans {
            span.style = span.style.fg(theme.highlight_fg()).bg(theme.highlight_bg());
        }
    } else {
        apply_background(&mut spans, content_style);
    }
    spans
}

fn card_border_style(selected: bool, theme: &Theme) -> Style {
    let style = Style::default().fg(if selected {
        theme.accent_fg()
    } else {
        theme.border_fg()
    });
    if selected {
        style.bg(theme.selected_bg())
    } else {
        style
    }
}

fn card_content_style(selected: bool, theme: &Theme) -> Style {
    let style = Style::default().fg(if selected {
        theme.selected_fg()
    } else {
        theme.selected_alt_fg()
    });
    if selected {
        style.bg(theme.selected_bg())
    } else {
        style
    }
}

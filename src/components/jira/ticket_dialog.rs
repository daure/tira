use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::{
    app::ticket_dialog::{TicketDialogIssue, TicketDialogState, TicketDialogTab},
    components::jira::work_item_key,
    ui::{layout, theme::Theme},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, state: &TicketDialogState, theme: &Theme) {
    frame.render_widget(Clear, area);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border_fg())),
        area,
    );

    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };
    let [header_area, tab_area, content_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .areas(inner);

    render_header(frame, header_area, &state.ticket, theme);
    render_tabs(frame, area, tab_area, state.selected_tab, theme);
    render_tab_content(frame, content_area, state, theme);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, ticket: &TicketDialogIssue, theme: &Theme) {
    let left_width = area
        .width
        .saturating_sub(ticket.status.chars().count() as u16 + 2) as usize;
    let mut key_spans = header_key_spans(ticket, theme);
    key_spans.push(Span::raw(
        " ".repeat(left_width.saturating_sub(line_width(&key_spans))),
    ));
    key_spans.push(Span::styled(
        ticket.status.as_str(),
        Style::default()
            .fg(theme.selected_alt_fg())
            .add_modifier(Modifier::BOLD),
    ));

    let summary = layout::truncate_with_ellipsis(&ticket.summary, area.width as usize);
    let lines = vec![
        Line::from(key_spans),
        Line::from(Span::styled(
            summary,
            Style::default()
                .fg(theme.status_text())
                .add_modifier(Modifier::BOLD),
        )),
        Line::default(),
    ];
    frame.render_widget(Paragraph::new(lines), area);
}

fn header_key_spans<'a>(ticket: &'a TicketDialogIssue, theme: &Theme) -> Vec<Span<'a>> {
    let mut spans = Vec::new();
    if let Some(parent_key) = ticket.parent_key.as_deref() {
        spans.extend(work_item_key::spans(
            theme,
            parent_key,
            ticket.parent_issue_type.as_deref().unwrap_or("Epic"),
            "",
            Style::default().fg(theme.muted_fg()),
        ));
        spans.push(Span::styled(
            "  /  ",
            Style::default().fg(theme.subtle_fg()),
        ));
    }
    spans.extend(work_item_key::spans(
        theme,
        &ticket.key,
        &ticket.issue_type,
        "",
        Style::default().fg(theme.key_fg()),
    ));
    spans
}

fn render_tabs(
    frame: &mut Frame<'_>,
    outer: Rect,
    area: Rect,
    selected: TicketDialogTab,
    theme: &Theme,
) {
    let mut spans = vec![Span::styled("├─ ", Style::default().fg(theme.border_fg()))];
    for (index, tab) in TicketDialogTab::ALL.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled("  - ", Style::default().fg(theme.muted_fg())));
        }
        let style = if *tab == selected {
            Style::default()
                .fg(theme.accent_fg())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.muted_fg())
        };
        spans.push(Span::styled(tab.label(), style));
    }
    let used = line_width(&spans);
    let width = outer.width as usize;
    spans.push(Span::styled(
        "─".repeat(width.saturating_sub(used + 1)),
        Style::default().fg(theme.border_fg()),
    ));
    spans.push(Span::styled("┤", Style::default().fg(theme.border_fg())));

    let line_area = Rect {
        x: outer.x,
        y: area.y,
        width: outer.width,
        height: 1,
    };
    frame.render_widget(Paragraph::new(Line::from(spans)), line_area);
}

fn render_tab_content(frame: &mut Frame<'_>, area: Rect, state: &TicketDialogState, theme: &Theme) {
    let lines = match state.selected_tab {
        TicketDialogTab::Overview => overview_lines(&state.ticket, theme),
        TicketDialogTab::Properties => property_lines(&state.ticket, theme),
        TicketDialogTab::Subtasks => empty_tab_lines("No subtasks loaded yet.", theme),
        TicketDialogTab::LinkedWorkItems => {
            empty_tab_lines("No linked work items loaded yet.", theme)
        }
    };
    frame.render_widget(Paragraph::new(lines), area);
}

fn overview_lines(ticket: &TicketDialogIssue, theme: &Theme) -> Vec<Line<'static>> {
    let description = ticket
        .fields
        .get("description")
        .or_else(|| ticket.fields.get("Description"));
    let mut lines = vec![section_title("Overview", theme), Line::default()];
    match description {
        Some(description) if !description.trim().is_empty() => {
            lines.extend(description.lines().map(|line| {
                Line::from(Span::styled(
                    line.to_owned(),
                    Style::default().fg(theme.status_text()),
                ))
            }));
        }
        _ => lines.push(Line::from(Span::styled(
            "No description loaded yet.",
            Style::default().fg(theme.muted_fg()),
        ))),
    }
    lines
}

fn property_lines(ticket: &TicketDialogIssue, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = vec![section_title("Properties", theme), Line::default()];
    lines.push(property_line("Type", &ticket.issue_type, theme));
    lines.push(property_line("Status", &ticket.status, theme));
    if let Some(parent) = &ticket.parent_key {
        lines.push(property_line("Parent", parent, theme));
    }
    for (key, value) in &ticket.fields {
        if key == "description" || key == "Description" {
            continue;
        }
        lines.push(property_line(key, value, theme));
    }
    lines
}

fn empty_tab_lines(message: &'static str, theme: &Theme) -> Vec<Line<'static>> {
    vec![
        section_title(message, theme),
        Line::default(),
        Line::from(Span::styled(
            "POC placeholder until ticket detail endpoints are wired.",
            Style::default().fg(theme.muted_fg()),
        )),
    ]
}

fn section_title(title: &str, theme: &Theme) -> Line<'static> {
    Line::from(Span::styled(
        title.to_owned(),
        Style::default()
            .fg(theme.selected_alt_fg())
            .add_modifier(Modifier::BOLD),
    ))
}

fn property_line(label: &str, value: &str, theme: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{label:<18}"),
            Style::default().fg(theme.muted_fg()),
        ),
        Span::styled(value.to_owned(), Style::default().fg(theme.status_text())),
    ])
}

fn line_width(spans: &[Span<'_>]) -> usize {
    spans.iter().map(|span| span.content.chars().count()).sum()
}

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::App;

pub fn render(frame: &mut Frame<'_>, area: Rect, viewport_height: u16, app: &App) {
    let rows = app.visible_issue_rows();
    if area.width == 0 || area.height == 0 || rows.is_empty() {
        return;
    }

    let visible_range = app.visible_issue_range(viewport_height as usize);
    render_range(frame, area, rows.len(), visible_range, app.theme());
}

pub fn render_range(
    frame: &mut Frame<'_>,
    area: Rect,
    row_count: usize,
    visible_range: std::ops::Range<usize>,
    theme: &crate::ui::theme::Theme,
) {
    if area.width == 0 || area.height == 0 || row_count == 0 {
        return;
    }

    let viewport = visible_range.len();
    if row_count <= viewport || viewport == 0 {
        return;
    }

    let track_height = area.height as usize;
    let thumb_height = viewport.saturating_mul(track_height).div_ceil(row_count);
    let thumb_height = thumb_height.clamp(1, track_height);
    let max_scroll = row_count.saturating_sub(viewport);
    let thumb_start = if max_scroll == 0 {
        0
    } else {
        let max_thumb_start = track_height.saturating_sub(thumb_height);
        (visible_range.start * max_thumb_start + max_scroll / 2) / max_scroll
    };

    let lines = (0..track_height).map(|index| {
        let symbol = if (thumb_start..thumb_start + thumb_height).contains(&index) {
            Span::styled("█", Style::default().fg(theme.accent_fg()))
        } else {
            Span::styled("│", Style::default().fg(theme.border_fg()))
        };
        Line::from(symbol)
    });

    frame.render_widget(Paragraph::new(lines.collect::<Vec<_>>()), area);
}

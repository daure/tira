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
    let thumb_range = thumb_range(row_count, visible_range, area.height as usize);
    if thumb_range.is_empty() {
        return;
    }

    let lines = (0..area.height as usize).map(|index| {
        let symbol = if thumb_range.contains(&index) {
            Span::styled("█", Style::default().fg(theme.accent_fg()))
        } else {
            Span::styled("│", Style::default().fg(theme.border_fg()))
        };
        Line::from(symbol)
    });

    frame.render_widget(Paragraph::new(lines.collect::<Vec<_>>()), area);
}

pub fn render_range_horizontal(
    frame: &mut Frame<'_>,
    area: Rect,
    item_count: usize,
    visible_range: std::ops::Range<usize>,
    theme: &crate::ui::theme::Theme,
) {
    let thumb_range = thumb_range(item_count, visible_range, area.width as usize);
    if thumb_range.is_empty() {
        return;
    }

    let spans = (0..area.width as usize)
        .map(|index| {
            if thumb_range.contains(&index) {
                Span::styled("━", Style::default().fg(theme.accent_fg()))
            } else {
                Span::styled("─", Style::default().fg(theme.border_fg()))
            }
        })
        .collect::<Vec<_>>();

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

pub fn thumb_range(
    row_count: usize,
    visible_range: std::ops::Range<usize>,
    track_height: usize,
) -> std::ops::Range<usize> {
    if row_count == 0 || track_height == 0 {
        return 0..0;
    }

    let viewport = visible_range.len();
    if row_count <= viewport || viewport == 0 {
        return 0..0;
    }

    let thumb_height = viewport.saturating_mul(track_height).div_ceil(row_count);
    let thumb_height = thumb_height.clamp(1, track_height);
    let max_scroll = row_count.saturating_sub(viewport);
    let thumb_start = if max_scroll == 0 {
        0
    } else {
        let max_thumb_start = track_height.saturating_sub(thumb_height);
        (visible_range.start * max_thumb_start + max_scroll / 2) / max_scroll
    };

    thumb_start..thumb_start + thumb_height
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumb_range_tracks_visible_window() {
        assert_eq!(thumb_range(20, 0..10, 10), 0..5);
        assert_eq!(thumb_range(20, 10..20, 10), 5..10);
        assert_eq!(thumb_range(10, 0..10, 10), 0..0);
    }
}

use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Minimum readable width for a board column. Below this we start scrolling
/// horizontally instead of squeezing more columns in.
const MIN_COL_WIDTH: u16 = 34;
/// Upper bound so columns don't grow absurdly wide when only a few are shown.
const MAX_COL_WIDTH: u16 = 52;
/// Sliver of the neighbouring column left visible at rest on each side that has
/// more columns, so the board reads as "more this way". Reserved on both sides
/// so the column width stays constant as the strip glides (no resizing mid
/// scroll); the slivers simply widen/narrow as the horizontal offset animates.
const PEEK_WIDTH: u16 = 8;

/// The run of board columns that sit fully on screen at rest. Drives the scroll
/// target (so the selected card stays visible) and the persisted column offset.
#[derive(Clone, Copy)]
pub(super) struct ColumnWindow {
    /// Index of the leftmost fully-visible column.
    pub(super) start: usize,
}

impl ColumnWindow {
    fn more_left(&self) -> bool {
        self.start > 0
    }
}

/// All horizontal layout decisions for one frame of the board: the width of
/// every column and which columns sit fully on screen at rest (the window).
///
/// The board is drawn as a single strip [`strip_width`](Self::strip_width) wide
/// (every column, full width) and then sliced to the viewport at an animated
/// cell offset. Rendering the whole strip — rather than only the visible
/// columns — is what lets the horizontal scroll glide smoothly: partial columns
/// at the edges (the "peek") fall out of the slice for free, at any sub-column
/// offset, instead of being special-cased.
///
/// This is a *pure* value computed by [`ColumnLayout::compute`] so the
/// non-trivial geometry can be unit-tested without a terminal.
pub(super) struct ColumnLayout {
    /// Width/position of every column, indexed by board column index. Uniform
    /// width while scrolling; equal-ratio fill when everything fits.
    pub(super) rects: Vec<Rect>,
    pub(super) window: ColumnWindow,
    /// Whether there are more columns than fit (drives the horizontal scrollbar
    /// and whether a non-zero scroll offset is possible).
    pub(super) scrolling: bool,
}

impl ColumnLayout {
    /// Columns that fit at the minimum readable width. At least one, so a very
    /// narrow terminal still shows a single (clipped) column rather than panic.
    fn max_visible(board_width: u16) -> usize {
        (board_width.max(1) / MIN_COL_WIDTH).max(1) as usize
    }

    /// Whether the board must scroll horizontally at this width. Cheap enough to
    /// call before [`compute`](Self::compute) so the caller can reserve a row
    /// for the scrollbar before the (height-reduced) area is handed back in.
    pub(super) fn will_scroll(board_width: u16, column_count: usize) -> bool {
        column_count.max(1) > Self::max_visible(board_width)
    }

    /// Derive the full layout. `selected_col` keeps the window scrolled so the
    /// selected card stays visible (mirroring the vertical auto-scroll), and
    /// `stored_scroll` is the previous column offset so the view is stable
    /// across frames when the selection doesn't move.
    pub(super) fn compute(
        board_area: Rect,
        column_count: usize,
        selected_col: Option<usize>,
        stored_scroll: usize,
    ) -> Self {
        let column_count = column_count.max(1);
        let board_width = board_area.width.max(1);
        let max_visible = Self::max_visible(board_width);

        if column_count <= max_visible {
            // Everything fits: equal-ratio fill, one window over all columns.
            let rects = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(
                    (0..column_count)
                        .map(|_| Constraint::Ratio(1, column_count as u32))
                        .collect::<Vec<_>>(),
                )
                .split(board_area)
                .to_vec();
            return Self {
                rects,
                window: ColumnWindow { start: 0 },
                scrolling: false,
            };
        }

        let visible_count = max_visible.min(column_count).max(1);

        // Slide the window to keep the selected column inside it.
        let mut start = stored_scroll.min(column_count - visible_count);
        if let Some(sel) = selected_col {
            let sel = sel.min(column_count - 1);
            if sel < start {
                start = sel;
            } else if sel >= start + visible_count {
                start = sel + 1 - visible_count;
            }
        }
        let window = ColumnWindow { start };

        // Reserve a peek on *both* sides up front so the column width is the
        // same at every scroll position (columns must not resize as you glide).
        // The scroll offset later decides which slivers are actually revealed.
        let reserve = 2 * PEEK_WIDTH;
        let full_area = board_width.saturating_sub(reserve);
        let col_width = (full_area / visible_count as u16).clamp(MIN_COL_WIDTH, MAX_COL_WIDTH);
        let rects = (0..column_count)
            .map(|_| Rect {
                x: board_area.x,
                y: board_area.y,
                width: col_width,
                height: board_area.height,
            })
            .collect::<Vec<_>>();

        Self {
            rects,
            window,
            scrolling: true,
        }
    }

    /// Total width of the rendered strip (sum of all column widths).
    pub(super) fn strip_width(&self) -> u16 {
        self.rects.iter().map(|r| r.width).sum()
    }

    /// Cell offset of column `idx`'s left edge within the strip.
    fn col_left(&self, idx: usize) -> u16 {
        self.rects[..idx.min(self.rects.len())]
            .iter()
            .map(|r| r.width)
            .sum()
    }

    /// The horizontal offset (in cells) the strip should rest at: the window's
    /// left column, pulled back by a peek so the previous column shows a sliver
    /// when there's more to the left. Clamped so the slice never runs past the
    /// strip. This is the *target* the animator glides toward.
    pub(super) fn target_offset(&self, board_width: u16) -> u16 {
        if !self.scrolling {
            return 0;
        }
        let left = self.col_left(self.window.start);
        let raw = left.saturating_sub(if self.window.more_left() {
            PEEK_WIDTH
        } else {
            0
        });
        let max_off = self.strip_width().saturating_sub(board_width);
        raw.min(max_off)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(width: u16) -> Rect {
        Rect {
            x: 0,
            y: 0,
            width,
            height: 40,
        }
    }

    #[test]
    fn fits_without_scrolling_keeps_one_window_and_no_offset() {
        // 4 columns at 200 cells: ~50 each, comfortably above MIN.
        let layout = ColumnLayout::compute(area(200), 4, Some(0), 0);
        assert!(!ColumnLayout::will_scroll(200, 4));
        assert!(!layout.scrolling);
        assert_eq!(layout.window.start, 0);
        assert_eq!(layout.rects.len(), 4);
        // Ratio fill spans the whole board; nothing to scroll.
        assert_eq!(layout.strip_width(), 200);
        assert_eq!(layout.target_offset(200), 0);
    }

    #[test]
    fn scrolls_when_columns_exceed_min_width() {
        // 6 columns at 150 cells: only 150/34 = 4 fit.
        assert!(ColumnLayout::will_scroll(150, 6));
        let layout = ColumnLayout::compute(area(150), 6, Some(0), 0);
        assert!(layout.scrolling);
        // Selection at the far left: window pinned to start, strip rests at 0.
        assert_eq!(layout.window.start, 0);
        assert!(layout.strip_width() > 150, "strip wider than the viewport");
        assert_eq!(layout.target_offset(150), 0);
    }

    #[test]
    fn window_follows_selection_and_reveals_a_left_peek_in_the_middle() {
        // Selecting column 4 (with 4 visible) scrolls to window starting at 1.
        let layout = ColumnLayout::compute(area(150), 6, Some(4), 0);
        assert_eq!(layout.window.start, 1);
        // The strip rests pulled back by one peek so column 0 shows a sliver.
        let col_width = layout.rects[0].width;
        assert_eq!(layout.target_offset(150), col_width - PEEK_WIDTH);
    }

    #[test]
    fn far_right_selection_clamps_to_the_end_of_the_strip() {
        let layout = ColumnLayout::compute(area(150), 6, Some(5), 0);
        // Window scrolled so the last column is visible.
        assert_eq!(layout.window.start, 2);
        let max_off = layout.strip_width() - 150;
        assert_eq!(layout.target_offset(150), max_off);
    }

    #[test]
    fn target_offset_always_stays_within_the_strip() {
        for sel in 0..6 {
            let layout = ColumnLayout::compute(area(150), 6, Some(sel), 0);
            let max_off = layout.strip_width().saturating_sub(150);
            assert!(
                layout.target_offset(150) <= max_off,
                "selection {sel} offset out of bounds"
            );
        }
    }
}

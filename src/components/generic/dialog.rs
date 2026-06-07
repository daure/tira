use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Block, Borders, Clear},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dialog<'a> {
    title: &'a str,
    width: u16,
    height: u16,
    y_offset: u16,
    border_style: Style,
}

impl<'a> Dialog<'a> {
    pub const fn new(title: &'a str, width: u16, height: u16) -> Self {
        Self {
            title,
            width,
            height,
            y_offset: 1,
            border_style: Style::new(),
        }
    }

    pub const fn y_offset(mut self, y_offset: u16) -> Self {
        self.y_offset = y_offset;
        self
    }

    pub const fn border_style(mut self, border_style: Style) -> Self {
        self.border_style = border_style;
        self
    }

    pub fn render(self, frame: &mut Frame<'_>, area: Rect) -> Rect {
        let dialog_area = self.area(area);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.border_style)
            .title(Line::from(self.title));
        let inner = block.inner(dialog_area);
        let padded_inner = Rect {
            x: inner.x.saturating_add(1),
            y: inner.y,
            width: inner.width.saturating_sub(2),
            height: inner.height,
        };

        frame.render_widget(Clear, dialog_area);
        frame.render_widget(block, dialog_area);

        padded_inner
    }

    fn area(self, area: Rect) -> Rect {
        let width = self.width.min(area.width).max(1);
        let height = self.height.min(area.height).max(1);
        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + self.y_offset.min(area.height.saturating_sub(height));

        Rect {
            x,
            y,
            width,
            height,
        }
    }
}

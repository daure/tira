use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders},
};

use crate::ui::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabAction {
    Previous,
    Next,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabsViewMode {
    Classic,
    Minimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabsState {
    selected: usize,
    view_mode: TabsViewMode,
}

impl TabsState {
    pub const fn new(selected: usize) -> Self {
        Self {
            selected,
            view_mode: TabsViewMode::Minimal,
        }
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn active_title<'a>(&self, tabs: &'a [&'a str]) -> Option<&'a str> {
        tabs.get(self.selected).copied()
    }

    pub fn set_selected(&mut self, selected: usize) {
        self.selected = selected;
    }

    pub fn view_mode(&self) -> TabsViewMode {
        self.view_mode
    }

    pub fn set_view_mode(&mut self, view_mode: TabsViewMode) {
        self.view_mode = view_mode;
    }

    pub fn is_active(&self, tabs: &[&str], title: &str) -> bool {
        self.active_title(tabs)
            .is_some_and(|active| active == title)
    }

    pub fn dispatch(&mut self, action: TabAction, tabs: &[&str]) {
        if tabs.is_empty() {
            self.selected = 0;
            return;
        }

        match action {
            TabAction::Previous => self.previous(tabs.len()),
            TabAction::Next => self.next(tabs.len()),
        }
    }

    fn previous(&mut self, tab_count: usize) {
        self.selected = self.selected.checked_sub(1).unwrap_or(tab_count - 1);
    }

    fn next(&mut self, tab_count: usize) {
        self.selected = (self.selected + 1) % tab_count;
    }
}

pub fn tabbed_frame(
    tabs: &[&str],
    active_tab: usize,
    view_mode: TabsViewMode,
    theme: &Theme,
) -> Block<'static> {
    match view_mode {
        TabsViewMode::Classic => Block::default().borders(Borders::ALL).title(tab_title_line(
            tabs,
            active_tab,
            TabsViewMode::Classic,
            theme,
        )),
        TabsViewMode::Minimal => Block::default()
            .borders(Borders::TOP)
            .border_set(ratatui::symbols::border::Set {
                top_left: ratatui::symbols::line::HORIZONTAL,
                top_right: ratatui::symbols::line::HORIZONTAL,
                ..ratatui::symbols::border::PLAIN
            })
            .title(tab_title_line(
                tabs,
                active_tab,
                TabsViewMode::Minimal,
                theme,
            )),
    }
}

fn tab_title_line(
    tabs: &[&str],
    active_tab: usize,
    view_mode: TabsViewMode,
    theme: &Theme,
) -> Line<'static> {
    let mut spans = Vec::with_capacity(tabs.len().saturating_mul(2).saturating_add(1));
    match view_mode {
        TabsViewMode::Classic => spans.push(Span::raw(" ")),
        TabsViewMode::Minimal => spans.push(Span::raw("─ ")),
    }
    for (index, title) in tabs.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" - ", Style::default().fg(theme.border_fg())));
        }

        let style = if index == active_tab {
            Style::default()
                .fg(theme.accent_fg())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.muted_fg())
        };
        spans.push(Span::styled((*title).to_owned(), style));
    }

    spans.push(Span::raw(" "));
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::{TabAction, TabsState};

    const TABS: &[&str] = &["One", "Two", "Three"];

    #[test]
    fn selected_tab_comes_from_supplied_tabs() {
        let tabs = TabsState::new(1);

        assert_eq!(tabs.active_title(TABS), Some("Two"));
        assert!(tabs.is_active(TABS, "Two"));
    }

    #[test]
    fn previous_and_next_wrap_with_supplied_tab_count() {
        let mut tabs = TabsState::new(0);

        tabs.dispatch(TabAction::Previous, TABS);
        assert_eq!(tabs.active_title(TABS), Some("Three"));
        tabs.dispatch(TabAction::Next, TABS);
        assert_eq!(tabs.active_title(TABS), Some("One"));
    }

    #[test]
    fn test_tabbed_frame_rendering() {
        use super::{TabsViewMode, tabbed_frame};
        let theme = crate::ui::theme::Theme::default();
        let _block_classic = tabbed_frame(TABS, 1, TabsViewMode::Classic, &theme);
        let _block_minimal = tabbed_frame(TABS, 1, TabsViewMode::Minimal, &theme);
    }
}

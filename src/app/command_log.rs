use super::App;

/// View state for the scrollable command-log dialog. Scroll position is kept
/// as a wrapped-line offset from the top; `follow` pins the viewport to the
/// latest entry (the bottom). The `last_*` metrics are written by the renderer
/// so keyboard/mouse scrolling can clamp without re-deriving the dialog width.
#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct CommandLogView {
    pub(crate) offset: std::cell::Cell<usize>,
    pub(crate) follow: std::cell::Cell<bool>,
    pub(crate) last_total: std::cell::Cell<usize>,
    pub(crate) last_viewport: std::cell::Cell<usize>,
    /// Set after a single `g`, so the next `g` jumps to the top (`gg`).
    pub(crate) go_to_start_pending: std::cell::Cell<bool>,
}

impl App {
    pub fn command_log_follows_tail(&self) -> bool {
        self.command_log_view.follow.get()
    }

    pub fn command_log_scroll(&self) -> usize {
        self.command_log_view.offset.get()
    }

    /// Records the wrapped-line total, viewport height, and clamped scroll
    /// offset from the latest render so keyboard/mouse scrolling can clamp
    /// without knowing the dialog width. Leaves `follow` untouched.
    pub fn cache_command_log_layout(&self, scroll: usize, total: usize, viewport: usize) {
        self.command_log_view.offset.set(scroll);
        self.command_log_view.last_total.set(total);
        self.command_log_view.last_viewport.set(viewport);
    }

    fn command_log_max_scroll(&self) -> usize {
        let viewport = self.command_log_view.last_viewport.get().max(1);
        self.command_log_view
            .last_total
            .get()
            .saturating_sub(viewport)
    }

    /// Scrolls the command log by `delta` wrapped lines, leaving follow mode
    /// when scrolling up and re-entering it once the bottom is reached again.
    pub(crate) fn scroll_command_log(&mut self, delta: isize) {
        self.command_log_view.go_to_start_pending.set(false);
        let max_scroll = self.command_log_max_scroll();
        let current = if self.command_log_view.follow.get() {
            max_scroll
        } else {
            self.command_log_view.offset.get().min(max_scroll)
        };
        let next = (current as isize + delta).clamp(0, max_scroll as isize) as usize;
        self.command_log_view.offset.set(next);
        self.command_log_view.follow.set(next >= max_scroll);
    }

    pub(crate) fn page_command_log(&mut self, direction: isize) {
        let viewport = self.command_log_view.last_viewport.get().max(1) as isize;
        self.scroll_command_log(direction * viewport);
    }

    /// Scrolls by half a viewport (Ctrl+U / Ctrl+D).
    pub(crate) fn half_page_command_log(&mut self, direction: isize) {
        let half = (self.command_log_view.last_viewport.get().max(2) / 2) as isize;
        self.scroll_command_log(direction * half);
    }

    pub(crate) fn command_log_to_start(&mut self) {
        self.command_log_view.go_to_start_pending.set(false);
        self.command_log_view.follow.set(false);
        self.command_log_view.offset.set(0);
    }

    pub(crate) fn command_log_to_end(&mut self) {
        self.command_log_view.go_to_start_pending.set(false);
        self.command_log_view.follow.set(true);
    }

    /// Handles a `g` keypress: the first arms the prefix, the second (`gg`)
    /// jumps to the top.
    pub(crate) fn command_log_arm_go_to_start(&mut self) {
        if self.command_log_view.go_to_start_pending.get() {
            self.command_log_to_start();
        } else {
            self.command_log_view.go_to_start_pending.set(true);
        }
    }
}

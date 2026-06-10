use super::{App, JiraLoadPurpose};
use crate::config::JiraCredentials;
use std::fmt;

/// Braille dot frames for the loading spinner, cycling clockwise.
const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
/// Plain-icon fallback frames (no Nerd/Unicode braille).
const SPINNER_FRAMES_PLAIN: [&str; 4] = ["|", "/", "-", "\\"];
/// Wall-clock time each spinner frame is shown.
const SPINNER_FRAME_INTERVAL: std::time::Duration = std::time::Duration::from_millis(80);

/// Tracks the current animated-spinner frame, advanced by elapsed wall time.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Spinner {
    frame: usize,
    elapsed: std::time::Duration,
}

impl Spinner {
    /// Advances the frame by the number of whole intervals in `dt`.
    pub(crate) fn tick(&mut self, dt: std::time::Duration) {
        self.elapsed += dt;
        while self.elapsed >= SPINNER_FRAME_INTERVAL {
            self.elapsed -= SPINNER_FRAME_INTERVAL;
            self.frame = self.frame.wrapping_add(1);
        }
    }

    /// The glyph for the current frame, honoring the plain-icon preference.
    pub(crate) fn glyph(&self) -> &'static str {
        if crate::ui::theme::prefers_plain_icons() {
            SPINNER_FRAMES_PLAIN[self.frame % SPINNER_FRAMES_PLAIN.len()]
        } else {
            SPINNER_FRAMES[self.frame % SPINNER_FRAMES.len()]
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupAction {
    NextField,
    PreviousField,
    Submit,
    Backspace,
    Quit,
    Text(char),
    None,
    MoveCursorStart,
    MoveCursorEnd,
    Clear,
    MoveCursorWordLeft,
    MoveCursorWordRight,
    DeleteWordLeft,
    DeleteWordRight,
    MoveCursorLeft,
    MoveCursorRight,
    DeleteToEnd,
    DeleteToStart,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialField {
    Site,
    Email,
    ApiKey,
    DefaultProject,
}

impl CredentialField {
    const ALL: [CredentialField; 4] = [
        CredentialField::Site,
        CredentialField::Email,
        CredentialField::ApiKey,
        CredentialField::DefaultProject,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CredentialField::Site => "Jira site",
            CredentialField::Email => "Email",
            CredentialField::ApiKey => "API token",
            CredentialField::DefaultProject => "Project key",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CredentialForm {
    site: String,
    email: String,
    api_key: String,
    default_project: String,
    active_field: usize,
    cursors: [usize; 4],
}

impl fmt::Debug for CredentialForm {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialForm")
            .field("site", &self.site)
            .field("email", &self.email)
            .field("api_key", &"<redacted>")
            .field("default_project", &self.default_project)
            .field("active_field", &self.active_field)
            .field("cursors", &self.cursors)
            .finish()
    }
}

impl Default for CredentialForm {
    fn default() -> Self {
        Self {
            site: String::new(),
            email: String::new(),
            api_key: String::new(),
            default_project: String::new(),
            active_field: 0,
            cursors: [0; 4],
        }
    }
}

impl CredentialForm {
    pub fn active_field(&self) -> CredentialField {
        CredentialField::ALL[self.active_field]
    }

    pub fn fields(&self) -> [(CredentialField, &str); 4] {
        [
            (CredentialField::Site, &self.site),
            (CredentialField::Email, &self.email),
            (CredentialField::ApiKey, &self.api_key),
            (CredentialField::DefaultProject, &self.default_project),
        ]
    }

    pub fn cursors(&self) -> [usize; 4] {
        self.cursors
    }

    pub fn active_field_idx(&self) -> usize {
        self.active_field
    }

    fn next_field(&mut self) {
        self.active_field = (self.active_field + 1) % CredentialField::ALL.len();
    }

    fn previous_field(&mut self) {
        self.active_field = self
            .active_field
            .checked_sub(1)
            .unwrap_or(CredentialField::ALL.len() - 1);
    }

    fn push(&mut self, c: char) {
        let field_idx = self.active_field;
        let mut cursor = self.cursors[field_idx];
        let val = self.active_value_mut();
        crate::components::generic::input::insert_char(val, &mut cursor, c);
        self.cursors[field_idx] = cursor;
    }

    fn backspace(&mut self) {
        let field_idx = self.active_field;
        let mut cursor = self.cursors[field_idx];
        let val = self.active_value_mut();
        crate::components::generic::input::delete_backwards(val, &mut cursor);
        self.cursors[field_idx] = cursor;
    }

    fn active_value_mut(&mut self) -> &mut String {
        match self.active_field() {
            CredentialField::Site => &mut self.site,
            CredentialField::Email => &mut self.email,
            CredentialField::ApiKey => &mut self.api_key,
            CredentialField::DefaultProject => &mut self.default_project,
        }
    }

    /// Runs an editing closure against the active field's value and a mutable
    /// copy of its cursor, persisting the cursor afterwards.
    fn with_active_cursor(&mut self, f: impl FnOnce(&mut String, &mut usize)) {
        let idx = self.active_field;
        let mut cursor = self.cursors[idx];
        let val = self.active_value_mut();
        f(val, &mut cursor);
        self.cursors[idx] = cursor;
    }

    fn credentials(&self) -> Option<JiraCredentials> {
        let credentials = JiraCredentials {
            site: self.site.trim().to_owned(),
            email: self.email.trim().to_owned(),
            api_key: self.api_key.trim().to_owned(),
            default_project: self.default_project.trim().to_owned(),
        };

        credentials.is_complete().then_some(credentials)
    }
}

impl App {
    pub fn dispatch_setup(&mut self, action: SetupAction) {
        use crate::components::generic::input;
        let field_idx = self.setup.active_field;
        match action {
            SetupAction::NextField => self.setup.next_field(),
            SetupAction::PreviousField => self.setup.previous_field(),
            SetupAction::Submit => self.submit_setup(),
            SetupAction::Backspace => self.setup.backspace(),
            SetupAction::Quit => self.running = false,
            SetupAction::Text(c) => self.setup.push(c),
            SetupAction::None => {}
            SetupAction::MoveCursorStart => {
                self.setup.cursors[field_idx] = 0;
            }
            SetupAction::MoveCursorEnd => {
                let val = self.setup.active_value_mut();
                self.setup.cursors[field_idx] = val.chars().count();
            }
            SetupAction::Clear => {
                let val = self.setup.active_value_mut();
                val.clear();
                self.setup.cursors[field_idx] = 0;
            }
            SetupAction::MoveCursorWordLeft => {
                self.setup
                    .with_active_cursor(|val, cursor| input::move_word_left(val, cursor));
            }
            SetupAction::MoveCursorWordRight => {
                self.setup
                    .with_active_cursor(|val, cursor| input::move_word_right(val, cursor));
            }
            SetupAction::DeleteWordLeft => {
                self.setup
                    .with_active_cursor(|val, cursor| input::delete_word_left(val, cursor));
            }
            SetupAction::DeleteWordRight => {
                let cursor = self.setup.cursors[field_idx];
                let val = self.setup.active_value_mut();
                input::delete_word_right(val, cursor);
            }
            SetupAction::MoveCursorLeft => {
                self.setup
                    .with_active_cursor(|_val, cursor| input::move_left(cursor));
            }
            SetupAction::MoveCursorRight => {
                self.setup
                    .with_active_cursor(|val, cursor| input::move_right(val, cursor));
            }
            SetupAction::DeleteToEnd => {
                let cursor = self.setup.cursors[field_idx];
                let val = self.setup.active_value_mut();
                input::delete_to_end(val, cursor);
            }
            SetupAction::DeleteToStart => {
                self.setup
                    .with_active_cursor(|val, cursor| input::delete_to_start(val, cursor));
            }
            SetupAction::Delete => {
                let cursor = self.setup.cursors[field_idx];
                let val = self.setup.active_value_mut();
                input::delete_forwards(val, cursor);
            }
        }
    }

    fn submit_setup(&mut self) {
        let Some(credentials) = self.setup.credentials() else {
            self.status = String::from("All Jira credential fields are required.");
            return;
        };

        self.status = String::from("Loading Jira issues");
        self.credentials = Some(credentials.clone());
        self.filtered_tree.set_jira_site(credentials.site.clone());
        self.queue_jira_load(
            JiraLoadPurpose::Setup,
            credentials,
            crate::services::jira::ROOT_PAGE_SIZE,
        );
    }
}

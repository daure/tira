//! First-class modal state for the app's overlay dialogs.
//!
//! Replaces the previous trio of `*_open` booleans with a single
//! `Option<ModalState>`, so "only one modal at a time" is enforced by the type
//! rather than by hand, and each modal's own view state (e.g. the help cursor)
//! lives with the modal instead of as loose `App` fields.

/// Identifies a modal dialog without its payload. Used by the open/close/toggle
/// API so call sites stay terse, e.g. `toggle_dialog(DialogKind::CommandLog)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DialogKind {
    CommandLog,
    Help,
    SprintDetails,
}

/// The currently-open modal together with its own view state. Stored as
/// `Option<ModalState>` on `App`; `None` means no modal is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ModalState {
    CommandLog,
    SprintDetails,
    Help { selected: usize },
}

impl ModalState {
    /// The payload-free identity of this modal.
    pub(crate) fn kind(&self) -> DialogKind {
        match self {
            ModalState::CommandLog => DialogKind::CommandLog,
            ModalState::SprintDetails => DialogKind::SprintDetails,
            ModalState::Help { .. } => DialogKind::Help,
        }
    }
}

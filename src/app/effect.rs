use crate::{
    config::JiraCredentials,
    services::jira::{
        BoardData, CommandLogEntry, FieldSummary, IssueSummary, JiraError, JiraLoadResult,
        ProjectSummary, TimelineData, UserSummary,
    },
    ui::theme::ThemeName,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEffect {
    LoadJiraProject {
        request_id: u64,
        purpose: JiraLoadPurpose,
        credentials: JiraCredentials,
        fields: String,
        /// Max root issues to fetch in the project load's root query. For a list
        /// reload this is sized to the already-loaded extent so it comes back in
        /// one page; otherwise it is the normal first-page size.
        root_max_results: u32,
    },
    /// Reload only the board (Greenhopper data), leaving the list view and its
    /// paging state untouched.
    ReloadBoardOnly {
        request_id: u64,
        credentials: JiraCredentials,
    },
    /// Load the Timeline tab's data (epics + sprints) on demand. Independent of
    /// the list/board load so switching to the tab does not disturb them.
    LoadTimeline {
        request_id: u64,
        credentials: JiraCredentials,
    },
    LoadMoreRoots {
        request_id: u64,
        credentials: JiraCredentials,
        fields: String,
        page_token: String,
        max_results: u32,
    },
    LoadChildren {
        request_id: u64,
        credentials: JiraCredentials,
        parent_key: String,
        fields: String,
    },
    /// Load the children of several parents in one batched query. Each parent
    /// carries its own request id so the existing per-parent result handling
    /// (stale-guard, soft-reload, expansion cascade) is reused unchanged.
    LoadChildrenBatch {
        credentials: JiraCredentials,
        parents: Vec<(String, u64)>,
        fields: String,
    },
    SearchIssues {
        request_id: u64,
        credentials: JiraCredentials,
        term: String,
        fields: String,
    },
    CopyToClipboard(String),
    SaveTheme(ThemeName),
    AssignIssue {
        request_id: u64,
        issue_key: String,
        assignee: Option<UserSummary>,
        credentials: JiraCredentials,
    },
    TransitionIssueStatus {
        request_id: u64,
        issue_key: String,
        status: String,
        status_id: Option<String>,
        credentials: JiraCredentials,
    },
    RankIssue {
        request_id: u64,
        issue_key: String,
        rank_before: Option<String>,
        rank_after: Option<String>,
        credentials: JiraCredentials,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEvent {
    JiraProjectLoaded {
        request_id: u64,
        purpose: JiraLoadPurpose,
        credentials: JiraCredentials,
        result: JiraProjectLoadResult,
    },
    RootsPageLoaded {
        request_id: u64,
        result: JiraLoadResult,
    },
    ChildrenLoaded {
        request_id: u64,
        parent_key: String,
        result: JiraLoadResult,
    },
    /// The board was reloaded on its own (no list reload).
    BoardReloaded {
        request_id: u64,
        board: Result<BoardData, JiraError>,
        logs: Vec<CommandLogEntry>,
    },
    /// The Timeline tab's data finished loading.
    TimelineLoaded {
        request_id: u64,
        timeline: Result<TimelineData, JiraError>,
        logs: Vec<CommandLogEntry>,
    },
    /// Children of several parents from one batched query. Each tuple is
    /// `(request_id, parent_key, children)`; `logs` carries the shared query
    /// log once for the whole batch.
    ChildrenBatchLoaded {
        results: Vec<(u64, String, Result<Vec<IssueSummary>, JiraError>)>,
        logs: Vec<CommandLogEntry>,
    },
    SearchLoaded {
        request_id: u64,
        term: String,
        result: JiraLoadResult,
    },
    CredentialsSaveFailed {
        request_id: u64,
        purpose: JiraLoadPurpose,
        error: String,
    },
    ThemeSaveFailed(String),
    IssueUrlCopied(String),
    IssueUrlCopyFailed {
        url: String,
        error: String,
    },
    IssueAssigned {
        request_id: u64,
        issue_key: String,
        assignee: Option<UserSummary>,
        result: Result<CommandLogEntry, (JiraError, CommandLogEntry)>,
    },
    IssueStatusChanged {
        request_id: u64,
        issue_key: String,
        status: String,
        status_id: Option<String>,
        result: Result<Vec<CommandLogEntry>, (JiraError, Vec<CommandLogEntry>)>,
    },
    IssueRanked {
        request_id: u64,
        issue_key: String,
        result: Result<CommandLogEntry, (JiraError, CommandLogEntry)>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JiraLoadPurpose {
    Initial,
    Setup,
    Reload,
    ReloadBoard,
    SwitchProject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JiraProjectLoadResult {
    pub issues: Result<Vec<IssueSummary>, JiraError>,
    pub board: Result<BoardData, JiraError>,
    pub next_page_token: Option<String>,
    pub fields: Result<Vec<FieldSummary>, JiraError>,
    pub projects: Result<Vec<ProjectSummary>, JiraError>,
    pub users: Result<Vec<UserSummary>, JiraError>,
    pub current_user: Result<UserSummary, JiraError>,
    /// Field IDs that actually carry a value on a sample of the project's most
    /// recently updated issues, used to hide instance-wide custom fields that
    /// are never populated here. `None` when the sample could not be taken, in
    /// which case every navigable field is offered.
    pub populated_fields: Option<std::collections::BTreeSet<String>>,
    pub logs: Vec<CommandLogEntry>,
}

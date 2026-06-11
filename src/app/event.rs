use super::*;

fn merge_board_issue_fields(board: &mut BoardData, list_issues: &[IssueSummary]) {
    for board_issue in &mut board.issues {
        let Some(list_issue) = list_issues
            .iter()
            .find(|list_issue| list_issue.key == board_issue.key)
        else {
            continue;
        };
        for (key, value) in &list_issue.field_values {
            if matches!(key.as_str(), "assignee" | "reporter") {
                board_issue.field_values.insert(key.clone(), value.clone());
            } else {
                board_issue
                    .field_values
                    .entry(key.clone())
                    .or_insert_with(|| value.clone());
            }
        }
    }
}

/// Keep only fields that belong in the column picker's selectable tail: those
/// neither pinned elsewhere nor (when a sample is available) empty across the
/// project's recent issues.
fn candidate_fields(
    fields: Vec<FieldSummary>,
    populated_fields: Option<&std::collections::BTreeSet<String>>,
) -> Vec<FieldSummary> {
    // Columns already shown elsewhere — `key`/`summary`/`priority` are
    // always-on fixed columns, and assignee/status/labels/issuetype are
    // pinned at the top of the picker — so their Jira fields must not be
    // re-offered as duplicates.
    const PINNED_IDS: [&str; 7] = [
        "key",
        "summary",
        "priority",
        "assignee",
        "status",
        "labels",
        "issuetype",
    ];

    fields
        .into_iter()
        .filter(|field| !PINNED_IDS.contains(&field.id.as_str()))
        .filter(|field| populated_fields.is_none_or(|ids| ids.contains(&field.id)))
        .collect()
}

/// Count how often each display name appears so callers can disambiguate
/// collisions with the field id. The pinned names are seeded so a custom
/// "Status" still collides.
fn name_disambiguation(candidates: &[FieldSummary]) -> std::collections::HashMap<String, usize> {
    let mut name_counts = std::collections::HashMap::new();
    for name in ["Assignee", "Status", "Labels", "Work type"] {
        name_counts.insert(name.to_owned(), 1);
    }
    for field in candidates {
        *name_counts.entry(field.name.clone()).or_insert(0) += 1;
    }
    name_counts
}

/// Order candidates (curated common fields first, then an alphabetical tail)
/// and build their picker columns, disambiguating shared names with the id.
fn ordered_columns(
    candidates: Vec<FieldSummary>,
    name_counts: &std::collections::HashMap<String, usize>,
) -> Vec<JiraIssueColumn> {
    // Common Jira fields surfaced (in this order) right after the pinned set,
    // ahead of the long tail of custom fields. Matched by stable system field
    // id first, then by display name for agile custom fields whose ids vary
    // per instance (Sprint, Story Points, …).
    const CURATED_IDS: [&str; 9] = [
        "reporter",
        "creator",
        "created",
        "updated",
        "duedate",
        "resolution",
        "fixVersions",
        "versions",
        "components",
    ];
    const CURATED_NAMES: [&str; 4] =
        ["Sprint", "Story Points", "Story point estimate", "Epic Link"];

    // Rank curated common fields by their listed order; everything else has no
    // rank and falls into the alphabetical tail.
    let curated_rank = |field: &FieldSummary| -> Option<usize> {
        CURATED_IDS
            .iter()
            .position(|id| *id == field.id)
            .or_else(|| {
                CURATED_NAMES
                    .iter()
                    .position(|name| *name == field.name)
                    .map(|pos| CURATED_IDS.len() + pos)
            })
    };
    let (mut curated, mut rest): (Vec<_>, Vec<_>) = candidates
        .into_iter()
        .partition(|field| curated_rank(field).is_some());
    curated.sort_by_key(|field| curated_rank(field).unwrap_or(usize::MAX));
    rest.sort_by_key(|field| field.name.to_lowercase());

    curated
        .into_iter()
        .chain(rest)
        .map(|field| {
            let label = if name_counts.get(&field.name).copied().unwrap_or(0) > 1 {
                format!("{} ({})", field.name, field.id)
            } else {
                field.name.clone()
            };
            JiraIssueColumn::Field {
                id: field.id,
                label,
            }
        })
        .collect()
}

impl App {
    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::JiraProjectLoaded {
                request_id,
                purpose,
                credentials,
                result,
            } => self.apply_jira_project_result(request_id, purpose, credentials, result),
            AppEvent::RootsPageLoaded { request_id, result } => {
                self.apply_roots_page_result(request_id, result)
            }
            AppEvent::ChildrenLoaded {
                request_id,
                parent_key,
                result,
            } => self.apply_children_result(request_id, parent_key, result),
            AppEvent::ChildrenBatchLoaded { results, logs } => {
                self.apply_children_batch_result(results, logs)
            }
            AppEvent::BoardReloaded {
                request_id,
                board,
                logs,
            } => self.apply_board_reloaded(request_id, board, logs),
            AppEvent::SearchLoaded {
                request_id,
                term,
                result,
            } => self.apply_search_result(request_id, term, result),
            AppEvent::CredentialsSaveFailed {
                request_id,
                purpose,
                error,
            } => self.apply_credentials_save_failed(request_id, purpose, error),
            AppEvent::ThemeSaveFailed(error) => self.notifications.push(Notification::error(
                "Theme not saved",
                format!("The theme changed for this session, but could not be saved: {error}"),
            )),
            AppEvent::IssueUrlCopied(url) => self
                .notifications
                .push(Notification::success("Issue URL copied", url)),
            AppEvent::IssueUrlCopyFailed { url, error } => {
                self.notifications.push(Notification::error(
                    "Issue URL not copied",
                    format!("Could not copy {url}: {error}"),
                ))
            }
            AppEvent::IssueAssigned {
                request_id,
                issue_key,
                assignee,
                result,
            } => self.apply_issue_assignment(request_id, issue_key, assignee, result),
        }
    }

    fn apply_issue_assignment(
        &mut self,
        request_id: u64,
        issue_key: String,
        assignee: Option<UserSummary>,
        result: Result<CommandLogEntry, (JiraError, CommandLogEntry)>,
    ) {
        if self.pending_assignment_requests.get(issue_key.as_str()) != Some(&request_id) {
            match result {
                Ok(log) => self.command_log.push(log),
                Err((_error, log)) => self.command_log.push(log),
            }
            return;
        }

        self.pending_assignment_requests.remove(issue_key.as_str());
        match result {
            Ok(log) => {
                self.command_log.push(log);
                let assignee_name = assignee.as_ref().map(|user| user.display_name.clone());
                let status = match assignee_name.as_ref() {
                    Some(name) => format!("{issue_key} assigned to {name}."),
                    None => format!("{issue_key} unassigned."),
                };
                self.filtered_tree
                    .update_assignee(issue_key.as_str(), assignee_name.clone());
                self.board
                    .update_assignee(issue_key.as_str(), assignee_name.clone());
                self.status = status.clone();
                self.notifications
                    .push(Notification::success("Assignee updated", status));
            }
            Err((error, log)) => {
                self.command_log.push(log);
                self.status = format!("Could not update {issue_key}: {}", error.0);
                self.notifications.push(Notification::error(
                    "Assignee not updated",
                    format!("Could not update {issue_key}: {}", error.0),
                ));
            }
        }
    }

    pub(crate) fn queue_jira_load(
        &mut self,
        purpose: JiraLoadPurpose,
        credentials: JiraCredentials,
        root_max_results: u32,
    ) {
        let request_id = self.next_request_id();
        self.active_load_request_id = Some(request_id);
        // A fresh project load resets browse/search/pagination state.
        self.view = ListView::Browse;
        self.next_root_page_token = None;
        self.pending_roots_request_id = None;
        self.search_request_id = None;
        self.applied_search_term = None;
        self.pending_child_requests.clear();
        // Default to no expansion restore; reload paths re-seed this afterwards.
        self.expansion_to_restore.clear();
        self.soft_reload_parents.clear();
        self.reload_root_ids = None;
        self.pending_reload_seamless = false;
        self.reload_children_buffer.clear();
        self.filtered_tree.set_flat(false);
        self.pending_effects.push(AppEffect::LoadJiraProject {
            request_id,
            purpose,
            credentials,
            fields: self.current_fields_param(),
            root_max_results,
        });
    }

    /// The `fields` query value for the current visible columns.
    pub(crate) fn current_fields_param(&self) -> String {
        JiraIssueColumn::fields_param(self.filtered_tree.visible_columns())
    }

    pub(crate) fn next_request_id(&mut self) -> u64 {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        request_id
    }

    fn is_current_load(&self, request_id: u64) -> bool {
        self.active_load_request_id == Some(request_id)
    }

    fn apply_credentials_save_failed(
        &mut self,
        request_id: u64,
        purpose: JiraLoadPurpose,
        error: String,
    ) {
        if !self.is_current_load(request_id) {
            return;
        }
        self.active_load_request_id = None;
        self.status = match purpose {
            JiraLoadPurpose::Setup => {
                format!("Could not save Jira credentials: {error}")
            }
            JiraLoadPurpose::SwitchProject => {
                format!("Could not save selected Jira project: {error}")
            }
            JiraLoadPurpose::Initial | JiraLoadPurpose::Reload | JiraLoadPurpose::ReloadBoard => {
                format!("Could not save Jira config: {error}")
            }
        };
    }

    fn apply_jira_project_result(
        &mut self,
        request_id: u64,
        purpose: JiraLoadPurpose,
        credentials: JiraCredentials,
        result: JiraProjectLoadResult,
    ) {
        if !self.is_current_load(request_id) {
            return;
        }
        self.active_load_request_id = None;
        if matches!(purpose, JiraLoadPurpose::Initial) {
            self.awaiting_initial_load = false;
        }

        self.command_log.extend(result.logs);

        // A list reload (Shift+R on the List tab) refreshes only the issue tree;
        // the board, projects, users, and field metadata are not refetched, so
        // we must not apply (empty) placeholders for them here.
        let list_only = matches!(purpose, JiraLoadPurpose::Reload);

        if !list_only {
            self.apply_loaded_fields(result.fields, result.populated_fields);
            self.apply_loaded_projects(result.projects);
        }

        let users = result.users;
        let current_user = result.current_user;
        let board = result.board;
        match result.issues {
            Ok(issues) => {
                let fallback_board_issues = issues.clone();
                if !list_only {
                    self.apply_loaded_users(users, current_user);
                }

                self.credentials = Some(credentials);
                if let Some(credentials) = &self.credentials {
                    self.filtered_tree.set_jira_site(credentials.site.clone());
                }
                self.filtered_tree.set_flat(false);
                self.view = ListView::Browse;
                let roots = tree_items_from_issues(issues);
                if std::mem::take(&mut self.pending_reload_seamless) {
                    self.begin_seamless_reload(roots, result.next_page_token);
                } else {
                    self.filtered_tree
                        .set_items(roots, &self.expansion_to_restore);
                    self.next_root_page_token = result.next_page_token;
                    // Lazy paging: the next page is pulled in on scroll
                    // (`maybe_prefetch_more_roots`), not eagerly up front.
                    // Re-open and re-fetch any roots whose expansion is being
                    // restored; nested levels follow as their children arrive.
                    self.drive_expansion_restore();
                }
                if !list_only {
                    self.apply_loaded_board(board, fallback_board_issues);
                }
                self.screen = Screen::Main;
                self.status = self.loaded_status_message(purpose);
            }
            Err(error) => {
                if !list_only && let Ok(board) = board {
                    self.board.set_data(board);
                }
                self.status = error.0;
            }
        }
    }

    fn apply_loaded_fields(
        &mut self,
        fields: Result<Vec<FieldSummary>, JiraError>,
        populated_fields: Option<std::collections::BTreeSet<String>>,
    ) {
        if let Ok(fields) = fields {
            self.apply_available_columns(fields, populated_fields);
        } else {
            self.notifications.push(Notification::error(
                "Jira fields not loaded",
                "Issue list is using built-in columns.",
            ));
        }
    }

    fn apply_loaded_projects(&mut self, projects: Result<Vec<ProjectSummary>, JiraError>) {
        if let Ok(projects) = projects {
            self.projects = projects;
        } else {
            self.notifications.push(Notification::error(
                "Jira projects not loaded",
                "Project switcher is unavailable until reload succeeds.",
            ));
        }
    }

    fn apply_loaded_users(
        &mut self,
        users: Result<Vec<UserSummary>, JiraError>,
        current_user: Result<UserSummary, JiraError>,
    ) {
        if let Ok(users) = users {
            self.users = users;
        } else {
            self.users.clear();
            self.notifications.push(Notification::error(
                "Jira users not loaded",
                "Assignee selector is unavailable until reload succeeds.",
            ));
        }

        if let Ok(current_user) = current_user {
            self.current_user = Some(current_user);
        } else {
            self.current_user = None;
            self.notifications.push(Notification::error(
                "Current Jira user not loaded",
                "Assign-to-me shortcut is unavailable until reload succeeds.",
            ));
        }
    }

    fn apply_loaded_board(
        &mut self,
        board: Result<BoardData, JiraError>,
        fallback_board_issues: Vec<IssueSummary>,
    ) {
        match board {
            Ok(mut board) => {
                normalize_board_user_fields(&mut board, &self.users, self.current_user.as_ref());
                merge_board_issue_fields(&mut board, &fallback_board_issues);
                self.board.set_data(board);
            }
            Err(error) => {
                let message = error.0;
                self.board
                    .set_data(BoardData::from_issues(fallback_board_issues));
                self.board.set_error(message.clone());
                self.notifications.push(Notification::error(
                    "Jira board not loaded",
                    "Board tab is grouped by issue status until the board endpoint succeeds.",
                ));
            }
        }
    }

    fn loaded_status_message(&self, purpose: JiraLoadPurpose) -> String {
        match purpose {
            JiraLoadPurpose::Initial | JiraLoadPurpose::Reload => {
                String::from("Jira issues loaded.")
            }
            JiraLoadPurpose::ReloadBoard => String::from("Jira board loaded."),
            JiraLoadPurpose::Setup => String::from("Jira credentials saved and issues loaded."),
            JiraLoadPurpose::SwitchProject => {
                format!("Jira project {} loaded.", self.current_project())
            }
        }
    }

    /// Applies a board-only reload: refreshes the board without touching the
    /// list view, its selection, or its paging state.
    fn apply_board_reloaded(
        &mut self,
        request_id: u64,
        board: Result<BoardData, JiraError>,
        logs: Vec<CommandLogEntry>,
    ) {
        if !self.is_current_load(request_id) {
            return;
        }
        self.active_load_request_id = None;
        self.command_log.extend(logs);
        match board {
            Ok(mut board) => {
                normalize_board_user_fields(&mut board, &self.users, self.current_user.as_ref());
                self.board.set_data(board);
                self.status = String::from("Jira board loaded.");
            }
            Err(error) => {
                self.board.set_error(error.0);
                self.notifications.push(Notification::error(
                    "Jira board not loaded",
                    "Board endpoint failed; the previous board is still shown.",
                ));
            }
        }
    }

    /// After the visible columns change, the required `fields` set changes too.
    /// Re-fetch the current view so newly-shown columns get populated.
    pub(crate) fn reload_current_view_fields(&mut self) {
        match &self.view {
            ListView::Browse => {
                let Some(credentials) = self.credentials.clone() else {
                    return;
                };
                self.queue_jira_load(
                    JiraLoadPurpose::Reload,
                    credentials,
                    crate::services::jira::ROOT_PAGE_SIZE,
                );
            }
            ListView::Search(term) => {
                let term = term.clone();
                self.run_search(term);
            }
        }
    }

    fn apply_available_columns(
        &mut self,
        fields: Vec<FieldSummary>,
        populated_fields: Option<std::collections::BTreeSet<String>>,
    ) {
        let mut columns = vec![
            JiraIssueColumn::Field {
                id: String::from("assignee"),
                label: String::from("Assignee"),
            },
            JiraIssueColumn::Status,
            JiraIssueColumn::labels_column(),
            JiraIssueColumn::IssueType,
        ];

        let candidates = candidate_fields(fields, populated_fields.as_ref());
        let name_counts = name_disambiguation(&candidates);
        columns.extend(ordered_columns(candidates, &name_counts));

        self.filtered_tree.set_available_columns(columns);
    }

    pub(crate) fn switch_project(&mut self, project: ProjectSummary) {
        let Some(mut credentials) = self.credentials.clone() else {
            self.status = String::from("No Jira credentials available for project switch.");
            return;
        };
        if credentials.default_project == project.key {
            return;
        }

        credentials.default_project = project.key.clone();
        self.status = format!("Loading Jira project {}", project.key);
        self.queue_jira_load(
            JiraLoadPurpose::SwitchProject,
            credentials,
            crate::services::jira::ROOT_PAGE_SIZE,
        );
    }

    pub(crate) fn reload_board(&mut self) {
        if self.active_tab() != ApplicationTab::Board {
            return;
        }

        let Some(credentials) = self.credentials.clone() else {
            self.status = String::from("No Jira credentials available for reload.");
            return;
        };

        self.status = String::from("Reloading Jira board...");
        let request_id = self.next_request_id();
        self.active_load_request_id = Some(request_id);
        self.pending_effects.push(AppEffect::ReloadBoardOnly {
            request_id,
            credentials,
        });
    }
}

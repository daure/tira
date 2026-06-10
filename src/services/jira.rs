use std::collections::{BTreeMap, HashSet};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use reqwest::Url;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::config::JiraCredentials;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueSummary {
    pub key: String,
    pub summary: String,
    pub status: String,
    pub issue_type: String,
    pub parent_key: Option<String>,
    pub has_children: bool,
    pub field_values: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldSummary {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSummary {
    pub key: String,
    pub name: String,
}

impl crate::ui::selector::HasShortcut for ProjectSummary {
    fn shortcut(&self, _keybindings: &crate::KeyBindings) -> Option<String> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserSummary {
    pub account_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandLogEntry {
    pub timestamp: String,
    pub method: &'static str,
    pub path: String,
    pub status: String,
    pub duration_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JiraError(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JiraLoadResult {
    pub issues: Result<Vec<IssueSummary>, JiraError>,
    pub next_page_token: Option<String>,
    pub logs: Vec<CommandLogEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JiraFieldsResult {
    pub fields: Result<Vec<FieldSummary>, JiraError>,
    pub log: CommandLogEntry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JiraProjectsResult {
    pub projects: Result<Vec<ProjectSummary>, JiraError>,
    pub log: CommandLogEntry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JiraUsersResult {
    pub users: Result<Vec<UserSummary>, JiraError>,
    pub current_user: Result<UserSummary, JiraError>,
    pub logs: Vec<CommandLogEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardLoadResult {
    pub board: Result<BoardData, JiraError>,
    pub logs: Vec<CommandLogEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardData {
    pub id: u64,
    pub name: String,
    pub columns: Vec<BoardColumnSummary>,
    pub swimlanes: Vec<BoardSwimlaneSummary>,
    pub issues: Vec<IssueSummary>,
    /// The board's active sprint, when the board is sprint-backed (scrum) and
    /// the payload carries one. Kanban boards and sprintless scrum boards have
    /// `None`, which the UI surfaces as "No active sprint".
    pub sprint: Option<SprintSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SprintSummary {
    pub name: String,
    pub goal: Option<String>,
    pub days_remaining: Option<i64>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

impl SprintSummary {
    /// Human-readable time remaining (e.g. `4 days left`), or `None` when the
    /// sprint carries no day count.
    pub fn days_left_label(&self) -> Option<String> {
        self.days_remaining.map(|days| match days {
            d if d < 0 => String::from("Sprint ended"),
            0 => String::from("Ends today"),
            1 => String::from("1 day left"),
            d => format!("{d} days left"),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BoardColumnSummary {
    pub name: String,
    pub statuses: Vec<String>,
    /// Configured WIP maximum for the column (Jira column constraint), if any.
    /// Shown as `count/max` in the ungrouped board header.
    pub max: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardSwimlaneSummary {
    pub id: Option<String>,
    pub name: String,
    pub issue_keys: Vec<String>,
}

impl BoardData {
    pub fn from_issues(issues: Vec<IssueSummary>) -> Self {
        let mut columns = Vec::new();
        let mut seen_statuses = std::collections::BTreeSet::new();
        for issue in &issues {
            if seen_statuses.insert(issue.status.clone()) {
                columns.push(BoardColumnSummary {
                    name: issue.status.clone(),
                    statuses: vec![issue.status.clone()],
                    max: None,
                });
            }
        }
        if columns.is_empty() {
            columns.push(BoardColumnSummary {
                name: String::from("Issues"),
                statuses: Vec::new(),
                max: None,
            });
        }

        Self {
            id: 0,
            name: String::from("Project board"),
            columns,
            swimlanes: vec![BoardSwimlaneSummary {
                id: None,
                name: String::from("Issues"),
                issue_keys: issues.iter().map(|issue| issue.key.clone()).collect(),
            }],
            issues,
            sprint: None,
        }
    }
}

/// Issues per page when walking the root issue list.
pub const ROOT_PAGE_SIZE: u32 = 100;
/// Upper bound for a single `/search/jql` response (child probe / child fetch,
/// and the one-shot root-extent refetch on reload).
pub const CHILD_PAGE_SIZE: u32 = 5_000;

/// Loads one page of top-level issues (no parent) for the default project,
/// most-recently-updated first, then probes which of them have children so the
/// tree can show expansion chevrons without fetching the children themselves.
pub fn load_root_issues(
    credentials: &JiraCredentials,
    fields: &str,
    page_token: Option<&str>,
    max_results: u32,
) -> JiraLoadResult {
    let jql = format!(
        "project = {} AND parent IS EMPTY ORDER BY updated DESC",
        credentials.default_project.trim()
    );
    let (issues, token, log) = run_search(credentials, &jql, fields, page_token, max_results);
    annotate_with_children(credentials, issues, token, log)
}

/// Loads the direct children of `parent_key`, most-recently-updated first, and
/// probes which of them have grandchildren so they can be expanded in turn.
pub fn load_child_issues(
    credentials: &JiraCredentials,
    parent_key: &str,
    fields: &str,
) -> JiraLoadResult {
    let jql = format!("parent = {parent_key} ORDER BY updated DESC");
    let (issues, token, log) = run_search(credentials, &jql, fields, None, CHILD_PAGE_SIZE);
    annotate_with_children(credentials, issues, token, log)
}

/// Maximum number of parent keys packed into a single `parent in (...)` query,
/// bounding the JQL/URL length. Larger parent sets are split across queries.
const BATCH_PARENT_CHUNK: usize = 50;

/// The direct children of several parents fetched together, grouped by parent.
pub struct ChildrenBatch {
    /// One entry per requested parent, in the same order as the input keys.
    /// `Ok(children)` (possibly empty) on success; `Err` when that parent's
    /// query chunk failed, so callers can leave its stale subtree in place.
    pub groups: Vec<(String, Result<Vec<IssueSummary>, JiraError>)>,
    pub logs: Vec<CommandLogEntry>,
}

/// Loads the direct children of many parents in as few queries as possible:
/// one `parent in (...)` search per chunk of parents (instead of one query per
/// parent), then a single grandchild probe across every returned child. The
/// children carry their own `parent` field, so the flat result is regrouped by
/// parent here. Parents with no children map to an empty vec so the caller can
/// clear a stale subtree.
pub fn load_children_batch(
    credentials: &JiraCredentials,
    parent_keys: &[String],
    fields: &str,
) -> ChildrenBatch {
    let mut logs = Vec::new();
    let mut all_issues: Vec<IssueSummary> = Vec::new();
    let mut failed: HashSet<String> = HashSet::new();
    let mut last_error: Option<JiraError> = None;

    for chunk in parent_keys.chunks(BATCH_PARENT_CHUNK) {
        let jql = format!("parent in ({}) ORDER BY updated DESC", chunk.join(","));
        let (issues, _token, log) = run_search(credentials, &jql, fields, None, CHILD_PAGE_SIZE);
        logs.push(log);
        match issues {
            Ok(mut chunk_issues) => all_issues.append(&mut chunk_issues),
            Err(error) => {
                for key in chunk {
                    failed.insert(key.clone());
                }
                last_error = Some(error);
            }
        }
    }

    // One grandchild probe across every fetched child, so each child still
    // shows an expansion chevron when it has children of its own.
    if !all_issues.is_empty() {
        let child_keys = all_issues
            .iter()
            .map(|issue| issue.key.clone())
            .collect::<Vec<_>>();
        let (parents_with_children, probe_log) = probe_children(credentials, &child_keys);
        logs.push(probe_log);
        for issue in &mut all_issues {
            issue.has_children = parents_with_children.contains(issue.key.as_str());
        }
    }

    // Regroup the flat result by each child's own parent key.
    let groups = group_children_for_parents(parent_keys, all_issues, &failed, last_error.as_ref());

    ChildrenBatch { groups, logs }
}

/// Buckets a flat list of children under their requested parents, in input
/// order. Parents present in `failed` map to that chunk's error so their stale
/// subtree survives; every other requested parent gets its children (or an
/// empty vec, which lets callers clear a subtree whose children all vanished).
/// Pure (no IO) so the grouping/empty/failure logic is unit-testable.
fn group_children_for_parents(
    parent_keys: &[String],
    issues: Vec<IssueSummary>,
    failed: &HashSet<String>,
    error: Option<&JiraError>,
) -> Vec<(String, Result<Vec<IssueSummary>, JiraError>)> {
    let mut by_parent: BTreeMap<String, Vec<IssueSummary>> = BTreeMap::new();
    for issue in issues {
        if let Some(parent) = issue.parent_key.clone() {
            by_parent.entry(parent).or_default().push(issue);
        }
    }
    parent_keys
        .iter()
        .map(|key| {
            if failed.contains(key) {
                let error = error
                    .cloned()
                    .unwrap_or_else(|| JiraError(String::from("child fetch failed")));
                (key.clone(), Err(error))
            } else {
                (key.clone(), Ok(by_parent.remove(key).unwrap_or_default()))
            }
        })
        .collect()
}

pub fn search_issues(
    credentials: &JiraCredentials,
    term: &str,
    fields: &str,
    page_token: Option<&str>,
) -> JiraLoadResult {
    let project = credentials.default_project.trim();
    let jql = match search_match_clause(term) {
        Some(clause) => format!("project = {project} AND {clause} ORDER BY updated DESC"),
        // Nothing searchable remained after sanitizing (e.g. only punctuation):
        // fall back to the plain project listing rather than a broken query.
        None => format!("project = {project} ORDER BY updated DESC"),
    };
    let (issues, next_page_token, log) =
        run_search(credentials, &jql, fields, page_token, ROOT_PAGE_SIZE);
    JiraLoadResult {
        issues,
        next_page_token,
        logs: vec![log],
    }
}

/// Builds the match portion of a search query from a raw term.
///
/// The term is split into words. Each word becomes a clause and all clauses are
/// AND-ed together so every typed word must match. A word that looks like an
/// issue key (`PROJ-123`) also matches the issue key exactly (case-insensitive),
/// since keys never appear in the summary text. Every other word is sanitized to
/// plain alphanumerics — so characters Lucene treats specially (`- + ! * ?` …)
/// can't become operators — and matched as a prefix wildcard, so partial words
/// like `adjustmen` still match `Adjustment` while the user is mid-type. Returns
/// `None` when no searchable token remains.
fn search_match_clause(term: &str) -> Option<String> {
    let clauses: Vec<String> = term
        .split_whitespace()
        .filter_map(word_match_clause)
        .collect();
    (!clauses.is_empty()).then(|| clauses.join(" AND "))
}

/// Builds the match clause for a single whitespace-delimited word.
fn word_match_clause(word: &str) -> Option<String> {
    let token: String = word.chars().filter(|ch| ch.is_alphanumeric()).collect();
    if token.is_empty() {
        return None;
    }
    match issue_key(word) {
        // Match the exact key (keys aren't in the summary) or a summary prefix.
        Some(key) => Some(format!("(key = \"{key}\" OR summary ~ \"{token}*\")")),
        None => Some(format!("summary ~ \"{token}*\"")),
    }
}

/// Recognizes a `PROJ-123` issue key and returns it upper-cased, or `None`.
fn issue_key(word: &str) -> Option<String> {
    let (project, number) = word.split_once('-')?;
    let project_ok = !project.is_empty() && project.chars().all(|ch| ch.is_ascii_alphabetic());
    let number_ok = !number.is_empty() && number.chars().all(|ch| ch.is_ascii_digit());
    (project_ok && number_ok).then(|| format!("{}-{number}", project.to_ascii_uppercase()))
}

/// Probes which of `parent_keys` have at least one child, then stamps
/// `has_children` onto the matching issues.
fn annotate_with_children(
    credentials: &JiraCredentials,
    issues: Result<Vec<IssueSummary>, JiraError>,
    next_page_token: Option<String>,
    search_log: CommandLogEntry,
) -> JiraLoadResult {
    let Ok(mut issues) = issues else {
        return JiraLoadResult {
            issues,
            next_page_token,
            logs: vec![search_log],
        };
    };
    if issues.is_empty() {
        return JiraLoadResult {
            issues: Ok(issues),
            next_page_token,
            logs: vec![search_log],
        };
    }

    let keys = issues
        .iter()
        .map(|issue| issue.key.clone())
        .collect::<Vec<_>>();
    let (parents_with_children, probe_log) = probe_children(credentials, &keys);
    for issue in &mut issues {
        issue.has_children = parents_with_children.contains(issue.key.as_str());
    }

    JiraLoadResult {
        issues: Ok(issues),
        next_page_token,
        logs: vec![search_log, probe_log],
    }
}

/// Returns the subset of `parent_keys` that have at least one child issue.
/// Best-effort: on failure the set is empty (chevrons stay hidden) but the log
/// records the error.
fn probe_children(
    credentials: &JiraCredentials,
    parent_keys: &[String],
) -> (HashSet<String>, CommandLogEntry) {
    let jql = format!(
        "parent in ({}) ORDER BY created DESC",
        parent_keys.join(",")
    );
    let (parsed, log) =
        fetch_search::<ParentProbeResponse>(credentials, &jql, "parent", None, CHILD_PAGE_SIZE);
    let parents = parsed
        .map(|payload| {
            payload
                .issues
                .into_iter()
                .filter_map(|issue| issue.fields.parent.map(|parent| parent.key))
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    (parents, log)
}

/// Executes a single `GET /search/jql` request and maps the page into issue
/// summaries plus a pagination cursor.
fn run_search(
    credentials: &JiraCredentials,
    jql: &str,
    fields: &str,
    page_token: Option<&str>,
    max_results: u32,
) -> (
    Result<Vec<IssueSummary>, JiraError>,
    Option<String>,
    CommandLogEntry,
) {
    let (parsed, log) =
        fetch_search::<SearchResponse>(credentials, jql, fields, page_token, max_results);
    match parsed {
        Ok(payload) => {
            let token = payload.next_page_token.filter(|_| !payload.is_last);
            let issues = payload
                .issues
                .into_iter()
                .map(issue_summary_from_search_issue)
                .collect();
            (Ok(issues), token, log)
        }
        Err(error) => (Err(error), None, log),
    }
}

/// Issues a single `GET /search/jql` request and deserializes the body into `T`.
/// Centralizes auth, URL building, timing, and logging for every search-shaped
/// query (roots, children, probe, search).
fn fetch_search<T: DeserializeOwned>(
    credentials: &JiraCredentials,
    jql: &str,
    fields: &str,
    page_token: Option<&str>,
    max_results: u32,
) -> (Result<T, JiraError>, CommandLogEntry) {
    let site = credentials.site.trim().trim_end_matches('/');
    let max_results = max_results.to_string();
    let mut query = vec![
        ("jql", jql),
        ("maxResults", max_results.as_str()),
        ("fields", fields),
    ];
    if let Some(token) = page_token {
        query.push(("nextPageToken", token));
    }
    let method = "GET";
    let timestamp = current_time_string();
    let url = match Url::parse_with_params(&format!("{site}/rest/api/3/search/jql"), &query) {
        Ok(url) => url,
        Err(error) => {
            return (
                Err(JiraError(format!("Invalid Jira site URL: {error}"))),
                failed_log(timestamp, method, "/search/jql"),
            );
        }
    };
    let started_at = Instant::now();

    let response = reqwest::blocking::Client::new()
        .get(url.clone())
        .basic_auth(credentials.email.trim(), Some(credentials.api_key.trim()))
        .send();

    let duration_ms = started_at.elapsed().as_millis();
    let path = log_path(&url);

    match response {
        Ok(response) => {
            let status = response.status();
            let log = CommandLogEntry {
                timestamp,
                method,
                path,
                status: status.as_u16().to_string(),
                duration_ms,
            };

            if !status.is_success() {
                return (Err(JiraError(format!("Jira returned HTTP {status}"))), log);
            }

            match response.json::<T>() {
                Ok(payload) => (Ok(payload), log),
                Err(error) => (
                    Err(JiraError(format!(
                        "Jira response could not be read: {error}"
                    ))),
                    log,
                ),
            }
        }
        Err(error) => (
            Err(JiraError(format!("Jira request failed: {error}"))),
            CommandLogEntry {
                timestamp,
                method,
                path,
                status: String::from("ERR"),
                duration_ms,
            },
        ),
    }
}

pub fn load_issue_fields(credentials: &JiraCredentials) -> JiraFieldsResult {
    let site = credentials.site.trim().trim_end_matches('/');
    let method = "GET";
    let timestamp = current_time_string();
    let url = match Url::parse(&format!("{site}/rest/api/3/field")) {
        Ok(url) => url,
        Err(error) => {
            return JiraFieldsResult {
                fields: Err(JiraError(format!("Invalid Jira site URL: {error}"))),
                log: failed_log(timestamp, method, "/field"),
            };
        }
    };
    let started_at = Instant::now();

    let response = reqwest::blocking::Client::new()
        .get(url.clone())
        .basic_auth(credentials.email.trim(), Some(credentials.api_key.trim()))
        .send();

    let duration_ms = started_at.elapsed().as_millis();
    let path = log_path(&url);

    match response {
        Ok(response) => {
            let status = response.status();
            let log = CommandLogEntry {
                timestamp,
                method,
                path,
                status: status.as_u16().to_string(),
                duration_ms,
            };

            if !status.is_success() {
                return JiraFieldsResult {
                    fields: Err(JiraError(format!("Jira returned HTTP {status}"))),
                    log,
                };
            }

            let fields = response
                .json::<Vec<FieldDetails>>()
                .map_err(|error| JiraError(format!("Jira fields could not be read: {error}")))
                .map(|payload| {
                    payload
                        .into_iter()
                        .filter(|field| field.navigable)
                        .map(|field| FieldSummary {
                            id: field.id,
                            name: field.name,
                        })
                        .collect()
                });

            JiraFieldsResult { fields, log }
        }
        Err(error) => JiraFieldsResult {
            fields: Err(JiraError(format!("Jira fields request failed: {error}"))),
            log: CommandLogEntry {
                timestamp,
                method,
                path,
                status: String::from("ERR"),
                duration_ms,
            },
        },
    }
}

pub fn load_projects(credentials: &JiraCredentials) -> JiraProjectsResult {
    let site = credentials.site.trim().trim_end_matches('/');
    let query = [("orderBy", "name")];
    let method = "GET";
    let timestamp = current_time_string();
    let url = match Url::parse_with_params(&format!("{site}/rest/api/3/project/search"), &query) {
        Ok(url) => url,
        Err(error) => {
            return JiraProjectsResult {
                projects: Err(JiraError(format!("Invalid Jira site URL: {error}"))),
                log: failed_log(timestamp, method, "/project/search"),
            };
        }
    };
    let started_at = Instant::now();

    let response = reqwest::blocking::Client::new()
        .get(url.clone())
        .basic_auth(credentials.email.trim(), Some(credentials.api_key.trim()))
        .send();

    let duration_ms = started_at.elapsed().as_millis();
    let path = log_path(&url);

    match response {
        Ok(response) => {
            let status = response.status();
            let log = CommandLogEntry {
                timestamp,
                method,
                path,
                status: status.as_u16().to_string(),
                duration_ms,
            };

            if !status.is_success() {
                return JiraProjectsResult {
                    projects: Err(JiraError(format!("Jira returned HTTP {status}"))),
                    log,
                };
            }

            let projects = response
                .json::<ProjectSearchResponse>()
                .map_err(|error| JiraError(format!("Jira projects could not be read: {error}")))
                .map(|payload| {
                    payload
                        .values
                        .into_iter()
                        .map(|project| ProjectSummary {
                            key: project.key,
                            name: project.name,
                        })
                        .collect()
                });

            JiraProjectsResult { projects, log }
        }
        Err(error) => JiraProjectsResult {
            projects: Err(JiraError(format!("Jira projects request failed: {error}"))),
            log: CommandLogEntry {
                timestamp,
                method,
                path,
                status: String::from("ERR"),
                duration_ms,
            },
        },
    }
}
pub fn load_assignable_users(credentials: &JiraCredentials) -> JiraUsersResult {
    let assignable = load_users_endpoint(
        credentials,
        "GET",
        "/user/assignable/search",
        Some(
            [
                ("project", credentials.default_project.trim()),
                ("maxResults", "1000"),
            ]
            .as_slice(),
        ),
    );
    let current = load_users_endpoint(credentials, "GET", "/myself", None);

    JiraUsersResult {
        users: assignable.users,
        current_user: current.current_user,
        logs: vec![assignable.log, current.log],
    }
}

pub fn load_project_board(credentials: &JiraCredentials) -> BoardLoadResult {
    let board_search = load_project_boards(credentials);
    let mut logs = vec![board_search.log];

    let board = match board_search.board {
        Ok(board) => board,
        Err(error) => {
            return BoardLoadResult {
                board: Err(error),
                logs,
            };
        }
    };

    let data = load_greenhopper_board_data(credentials, board.id, board.name);
    logs.push(data.log);
    BoardLoadResult {
        board: data.board,
        logs,
    }
}

struct BoardSearchLoad {
    board: Result<BoardDetails, JiraError>,
    log: CommandLogEntry,
}

struct BoardDataLoad {
    board: Result<BoardData, JiraError>,
    log: CommandLogEntry,
}

fn load_project_boards(credentials: &JiraCredentials) -> BoardSearchLoad {
    let site = credentials.site.trim().trim_end_matches('/');
    let project = credentials.default_project.trim();
    let query = [("projectKeyOrId", project), ("maxResults", "50")];
    let method = "GET";
    let timestamp = current_time_string();
    let url = match Url::parse_with_params(&format!("{site}/rest/agile/1.0/board"), &query) {
        Ok(url) => url,
        Err(error) => {
            return BoardSearchLoad {
                board: Err(JiraError(format!("Invalid Jira site URL: {error}"))),
                log: failed_log(timestamp, method, "/board"),
            };
        }
    };
    let started_at = Instant::now();
    let response = reqwest::blocking::Client::new()
        .get(url.clone())
        .basic_auth(credentials.email.trim(), Some(credentials.api_key.trim()))
        .send();
    let duration_ms = started_at.elapsed().as_millis();
    let path = log_path(&url);

    match response {
        Ok(response) => {
            let status = response.status();
            let log = CommandLogEntry {
                timestamp,
                method,
                path,
                status: status.as_u16().to_string(),
                duration_ms,
            };
            if !status.is_success() {
                return BoardSearchLoad {
                    board: Err(JiraError(format!("Jira returned HTTP {status}"))),
                    log,
                };
            }

            let board = response
                .json::<BoardSearchResponse>()
                .map_err(|error| JiraError(format!("Jira boards could not be read: {error}")))
                .and_then(|payload| {
                    payload.values.into_iter().next().ok_or_else(|| {
                        JiraError(format!("No Jira board found for project {project}"))
                    })
                });
            BoardSearchLoad { board, log }
        }
        Err(error) => BoardSearchLoad {
            board: Err(JiraError(format!("Jira boards request failed: {error}"))),
            log: CommandLogEntry {
                timestamp,
                method,
                path,
                status: String::from("ERR"),
                duration_ms,
            },
        },
    }
}

fn load_greenhopper_board_data(
    credentials: &JiraCredentials,
    board_id: u64,
    board_name: String,
) -> BoardDataLoad {
    let site = credentials.site.trim().trim_end_matches('/');
    let rapid_view_id = board_id.to_string();
    let query = [("rapidViewId", rapid_view_id.as_str())];
    let method = "GET";
    let timestamp = current_time_string();
    let url = match Url::parse_with_params(
        &format!("{site}/rest/greenhopper/1.0/xboard/work/allData.json"),
        &query,
    ) {
        Ok(url) => url,
        Err(error) => {
            return BoardDataLoad {
                board: Err(JiraError(format!("Invalid Jira site URL: {error}"))),
                log: failed_log(timestamp, method, "/xboard/work/allData.json"),
            };
        }
    };
    let started_at = Instant::now();
    let response = reqwest::blocking::Client::new()
        .get(url.clone())
        .basic_auth(credentials.email.trim(), Some(credentials.api_key.trim()))
        .send();
    let duration_ms = started_at.elapsed().as_millis();
    let path = log_path(&url);

    match response {
        Ok(response) => {
            let status = response.status();
            let log = CommandLogEntry {
                timestamp,
                method,
                path,
                status: status.as_u16().to_string(),
                duration_ms,
            };
            if !status.is_success() {
                return BoardDataLoad {
                    board: Err(JiraError(format!("Jira returned HTTP {status}"))),
                    log,
                };
            }

            let board = response
                .json::<serde_json::Value>()
                .map_err(|error| JiraError(format!("Jira board data could not be read: {error}")))
                .and_then(|payload| board_data_from_greenhopper(board_id, board_name, payload));
            BoardDataLoad { board, log }
        }
        Err(error) => BoardDataLoad {
            board: Err(JiraError(format!("Jira board request failed: {error}"))),
            log: CommandLogEntry {
                timestamp,
                method,
                path,
                status: String::from("ERR"),
                duration_ms,
            },
        },
    }
}

struct AssignableLoad {
    users: Result<Vec<UserSummary>, JiraError>,
    current_user: Result<UserSummary, JiraError>,
    log: CommandLogEntry,
}

fn load_users_endpoint(
    credentials: &JiraCredentials,
    method: &'static str,
    path: &str,
    query: Option<&[(&str, &str)]>,
) -> AssignableLoad {
    let site = credentials.site.trim().trim_end_matches('/');
    let timestamp = current_time_string();
    let url = match query {
        Some(query) => Url::parse_with_params(&format!("{site}/rest/api/3{path}"), query),
        None => Url::parse(&format!("{site}/rest/api/3{path}")),
    };
    let url = match url {
        Ok(url) => url,
        Err(error) => {
            let error = JiraError(format!("Invalid Jira site URL: {error}"));
            return AssignableLoad {
                users: Err(error.clone()),
                current_user: Err(error),
                log: failed_log(timestamp, method, path),
            };
        }
    };
    let started_at = Instant::now();
    let response = reqwest::blocking::Client::new()
        .get(url.clone())
        .basic_auth(credentials.email.trim(), Some(credentials.api_key.trim()))
        .send();
    let duration_ms = started_at.elapsed().as_millis();
    let log_path = log_path(&url);

    match response {
        Ok(response) => {
            let status = response.status();
            let log = CommandLogEntry {
                timestamp,
                method,
                path: log_path,
                status: status.as_u16().to_string(),
                duration_ms,
            };
            if !status.is_success() {
                let error = JiraError(format!("Jira returned HTTP {status}"));
                return AssignableLoad {
                    users: Err(error.clone()),
                    current_user: Err(error),
                    log,
                };
            }

            if path == "/myself" {
                let current_user = response
                    .json::<AssignableUser>()
                    .map_err(|error| {
                        JiraError(format!("Jira current user could not be read: {error}"))
                    })
                    .map(UserSummary::from);
                AssignableLoad {
                    users: Ok(Vec::new()),
                    current_user,
                    log,
                }
            } else {
                let users = response
                    .json::<Vec<AssignableUser>>()
                    .map_err(|error| JiraError(format!("Jira users could not be read: {error}")))
                    .map(|payload| payload.into_iter().map(UserSummary::from).collect());
                AssignableLoad {
                    users,
                    current_user: Err(JiraError(String::from("Current Jira user not loaded"))),
                    log,
                }
            }
        }
        Err(error) => {
            let error = JiraError(format!("Jira users request failed: {error}"));
            AssignableLoad {
                users: Err(error.clone()),
                current_user: Err(error),
                log: CommandLogEntry {
                    timestamp,
                    method,
                    path: log_path,
                    status: String::from("ERR"),
                    duration_ms,
                },
            }
        }
    }
}

pub fn assign_issue(
    credentials: &JiraCredentials,
    issue_key: &str,
    account_id: Option<&str>,
) -> Result<CommandLogEntry, (JiraError, CommandLogEntry)> {
    let site = credentials.site.trim().trim_end_matches('/');
    let method = "PUT";
    let path = format!("/issue/{issue_key}/assignee");
    let timestamp = current_time_string();
    let url = match Url::parse(&format!("{site}/rest/api/3{path}")) {
        Ok(url) => url,
        Err(error) => {
            return Err((
                JiraError(format!("Invalid Jira site URL: {error}")),
                failed_log(timestamp, method, path.as_str()),
            ));
        }
    };
    let started_at = Instant::now();

    let response = reqwest::blocking::Client::new()
        .put(url.clone())
        .basic_auth(credentials.email.trim(), Some(credentials.api_key.trim()))
        .json(&AssignIssuePayload { account_id })
        .send();

    let duration_ms = started_at.elapsed().as_millis();
    let log_path = log_path(&url);

    match response {
        Ok(response) => {
            let status = response.status();
            let log = CommandLogEntry {
                timestamp,
                method,
                path: log_path,
                status: status.as_u16().to_string(),
                duration_ms,
            };

            if status.is_success() {
                Ok(log)
            } else {
                Err((JiraError(format!("Jira returned HTTP {status}")), log))
            }
        }
        Err(error) => Err((
            JiraError(format!("Jira assignment request failed: {error}")),
            CommandLogEntry {
                timestamp,
                method,
                path: log_path,
                status: String::from("ERR"),
                duration_ms,
            },
        )),
    }
}
fn failed_log(timestamp: String, method: &'static str, path: &str) -> CommandLogEntry {
    CommandLogEntry {
        timestamp,
        method,
        path: path.to_owned(),
        status: String::from("ERR"),
        duration_ms: 0,
    }
}

fn log_path(url: &Url) -> String {
    let path = url.path().strip_prefix("/rest/api/3").unwrap_or(url.path());
    let Some(query) = url.query() else {
        return path.to_owned();
    };

    let decoded_query = url::form_urlencoded::parse(query.as_bytes())
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    format!("{path}?{decoded_query}")
}

fn current_time_string() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_secs()
        % 86_400;
    let hours = seconds / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let secs = seconds % 60;
    format!("{hours:02}:{minutes:02}:{secs:02}")
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssignableUser {
    account_id: String,
    display_name: String,
}

impl From<AssignableUser> for UserSummary {
    fn from(user: AssignableUser) -> Self {
        Self {
            account_id: user.account_id,
            display_name: user.display_name,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AssignIssuePayload<'a> {
    account_id: Option<&'a str>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchResponse {
    issues: Vec<SearchIssue>,
    #[serde(default)]
    next_page_token: Option<String>,
    #[serde(default)]
    is_last: bool,
}

/// Minimal response for the child-presence probe: only the `parent` field is
/// requested, so the issue's other fields (summary/status/type) are absent.
#[derive(Debug, Deserialize)]
struct ParentProbeResponse {
    issues: Vec<ParentProbeIssue>,
}

#[derive(Debug, Deserialize)]
struct ParentProbeIssue {
    fields: ParentProbeFields,
}

#[derive(Debug, Deserialize)]
struct ParentProbeFields {
    parent: Option<IssueParent>,
}

#[derive(Debug, Deserialize)]
struct SearchIssue {
    key: String,
    fields: IssueFields,
}

#[derive(Debug, Deserialize)]
struct IssueFields {
    summary: String,
    status: IssueStatus,
    #[serde(rename = "issuetype")]
    issue_type: IssueType,
    parent: Option<IssueParent>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct IssueStatus {
    name: String,
}

#[derive(Debug, Deserialize)]
struct IssueType {
    name: String,
}

#[derive(Debug, Deserialize)]
struct IssueParent {
    key: String,
}

#[derive(Debug, Deserialize)]
struct FieldDetails {
    id: String,
    name: String,
    navigable: bool,
}

#[derive(Debug, Deserialize)]
struct ProjectSearchResponse {
    values: Vec<ProjectDetails>,
}

#[derive(Debug, Deserialize)]
struct ProjectDetails {
    key: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct BoardSearchResponse {
    values: Vec<BoardDetails>,
}

#[derive(Debug, Deserialize)]
struct BoardDetails {
    id: u64,
    name: String,
}

fn issue_summary_from_search_issue(issue: SearchIssue) -> IssueSummary {
    let parent_key = issue
        .fields
        .parent
        .as_ref()
        .map(|parent| parent.key.clone());
    let mut field_values = issue
        .fields
        .extra
        .iter()
        .filter_map(|(id, value)| format_field_value(value).map(|text| (id.clone(), text)))
        .collect::<BTreeMap<_, _>>();
    if let Some(parent_key) = &parent_key {
        field_values.insert(String::from("parent"), parent_key.clone());
    }

    IssueSummary {
        key: issue.key,
        summary: issue.fields.summary,
        status: issue.fields.status.name,
        issue_type: issue.fields.issue_type.name,
        parent_key,
        has_children: false,
        field_values,
    }
}

fn board_data_from_greenhopper(
    board_id: u64,
    board_name: String,
    payload: serde_json::Value,
) -> Result<BoardData, JiraError> {
    let columns_value = payload
        .pointer("/columnsData/columns")
        .or_else(|| payload.get("columns"))
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| JiraError(String::from("Jira board data did not include columns")))?;
    let issues_value = payload
        .pointer("/issuesData/issues")
        .or_else(|| payload.pointer("/issuesData"))
        .or_else(|| payload.get("issuesData"))
        .ok_or_else(|| {
            JiraError(String::from(
                "Jira board data did not include issue details",
            ))
        })?;

    let mut columns = columns_value
        .iter()
        .map(board_column_from_value)
        .collect::<Vec<_>>();
    columns.sort_by_key(|(position, _)| *position);
    let columns = columns
        .into_iter()
        .map(|(_, column)| column)
        .collect::<Vec<_>>();

    let visible_issue_ids = board_visible_issue_ids(&payload);
    let mut issues = board_issues_from_value(Some(issues_value));
    filter_board_card_issues(&mut issues);
    filter_board_visible_issues(visible_issue_ids.as_ref(), &mut issues);
    let swimlane_values = payload
        .pointer("/swimlanesData/swimlanes")
        .or_else(|| payload.pointer("/swimlaneData/swimlanes"))
        .or_else(|| payload.get("swimlanes"))
        .and_then(serde_json::Value::as_array);
    let mut swimlanes = swimlane_values
        .map(|swimlanes| {
            let mut swimlanes = swimlanes
                .iter()
                .map(board_swimlane_from_value)
                .collect::<Vec<_>>();
            swimlanes.sort_by_key(|(position, _)| *position);
            swimlanes
                .into_iter()
                .map(|(_, mut swimlane)| {
                    if swimlane.issue_keys.is_empty()
                        && let Some(id) = swimlane.id.as_deref()
                    {
                        swimlane.issue_keys = issues
                            .iter()
                            .filter(|issue| {
                                issue.field_values.get("swimlane_id").map(String::as_str)
                                    == Some(id)
                            })
                            .map(|issue| issue.key.clone())
                            .collect();
                    }
                    swimlane
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    normalize_swimlane_issue_keys(&issues, &mut swimlanes);

    let assigned_count = swimlanes
        .iter()
        .map(|lane| lane.issue_keys.len())
        .sum::<usize>();
    if swimlanes.is_empty() || assigned_count == 0 {
        swimlanes.clear();
        swimlanes.push(BoardSwimlaneSummary {
            id: None,
            name: String::from("Issues"),
            issue_keys: issues.iter().map(|issue| issue.key.clone()).collect(),
        });
    } else {
        let mut assigned = std::collections::BTreeSet::new();
        for swimlane in &swimlanes {
            assigned.extend(swimlane.issue_keys.iter().cloned());
        }
        let other_keys = issues
            .iter()
            .filter(|issue| !assigned.contains(&issue.key))
            .map(|issue| issue.key.clone())
            .collect::<Vec<_>>();
        if !other_keys.is_empty() {
            swimlanes.push(BoardSwimlaneSummary {
                id: None,
                name: String::from("Other issues"),
                issue_keys: other_keys,
            });
        }
    }

    Ok(BoardData {
        id: board_id,
        name: board_name,
        columns,
        swimlanes,
        issues,
        sprint: board_sprint_from_value(&payload),
    })
}

/// Picks the sprint to summarise from the greenhopper payload: the ACTIVE
/// sprint if present, otherwise the first listed. Returns `None` for boards
/// without sprint data (e.g. Kanban).
fn board_sprint_from_value(payload: &serde_json::Value) -> Option<SprintSummary> {
    let sprints = payload
        .pointer("/sprintsData/sprints")
        .and_then(serde_json::Value::as_array)?;
    let sprint = sprints
        .iter()
        .find(|sprint| {
            text_property(sprint, &["state"])
                .is_some_and(|state| state.eq_ignore_ascii_case("active"))
        })
        .or_else(|| sprints.first())?;

    Some(SprintSummary {
        name: text_property(sprint, &["name"]).unwrap_or_else(|| String::from("Sprint")),
        goal: text_property(sprint, &["goal"]),
        days_remaining: sprint
            .get("daysRemaining")
            .and_then(serde_json::Value::as_i64),
        start_date: text_property(sprint, &["isoStartDate"])
            .as_deref()
            .and_then(format_iso_date),
        end_date: text_property(sprint, &["isoEndDate"])
            .as_deref()
            .and_then(format_iso_date),
    })
}

/// Formats the leading `YYYY-MM-DD` of an ISO 8601 timestamp into a short
/// human date such as `Jun 3, 2026`. Greenhopper encodes the zone offset
/// without a colon (`+0200`), which is not RFC3339-valid, so only the date
/// portion is parsed.
fn format_iso_date(iso: &str) -> Option<String> {
    let date = iso.get(..10)?;
    let mut parts = date.split('-');
    let year = parts.next()?;
    let month: usize = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    let month_name = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
    .get(month.checked_sub(1)?)?;
    Some(format!("{month_name} {day}, {year}"))
}

fn board_column_from_value(value: &serde_json::Value) -> (i64, BoardColumnSummary) {
    let name = text_property(value, &["name"]).unwrap_or_else(|| String::from("Column"));
    let statuses = value
        .get("statusIds")
        .or_else(|| value.get("statuses"))
        .and_then(serde_json::Value::as_array)
        .map(|statuses| {
            statuses
                .iter()
                .filter_map(|status| {
                    text_property(status, &["id", "statusId", "name"])
                        .or_else(|| format_field_value(status))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Jira exposes the per-column WIP maximum as `max` (0 / absent = no limit).
    // Be liberal about the encoding: greenhopper has shipped it as a JSON
    // number, a float, and a string across versions, and the key has also
    // appeared as `maxIssueCount`.
    let max = ["max", "maxIssueCount"]
        .iter()
        .find_map(|key| value.get(*key))
        .and_then(column_constraint_value)
        .filter(|max| *max > 0);

    (
        value
            .get("position")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0),
        BoardColumnSummary {
            name,
            statuses,
            max,
        },
    )
}

/// Parses a column constraint that Jira may encode as an integer, float or
/// numeric string into a count.
fn column_constraint_value(value: &serde_json::Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|n| u64::try_from(n).ok()))
        .or_else(|| value.as_f64().map(|n| n as u64))
        .or_else(|| value.as_str().and_then(|s| s.trim().parse().ok()))
}

fn board_swimlane_from_value(value: &serde_json::Value) -> (i64, BoardSwimlaneSummary) {
    let id = text_property(value, &["swimlaneId", "id"]);
    let name = text_property(value, &["name"]).unwrap_or_else(|| String::from("Swimlane"));
    let issue_keys = value
        .get("issueKeys")
        .or_else(|| value.get("issues"))
        .or_else(|| value.get("issueIds"))
        .and_then(serde_json::Value::as_array)
        .map(|issues| {
            issues
                .iter()
                .filter_map(|issue| {
                    issue
                        .as_str()
                        .map(str::to_owned)
                        .or_else(|| text_property(issue, &["key", "issueKey", "id"]))
                        .or_else(|| format_field_value(issue))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    (
        value
            .get("position")
            .or_else(|| value.get("pos"))
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0),
        BoardSwimlaneSummary {
            id,
            name,
            issue_keys,
        },
    )
}

fn normalize_swimlane_issue_keys(issues: &[IssueSummary], swimlanes: &mut [BoardSwimlaneSummary]) {
    let key_set = issues
        .iter()
        .map(|issue| issue.key.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let id_to_key = issues
        .iter()
        .filter_map(|issue| {
            issue
                .field_values
                .get("id")
                .map(|id| (id.as_str(), issue.key.as_str()))
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    for swimlane in swimlanes {
        for key in &mut swimlane.issue_keys {
            if !key_set.contains(key.as_str())
                && let Some(issue_key) = id_to_key.get(key.as_str())
            {
                *key = (*issue_key).to_owned();
            }
        }
        swimlane
            .issue_keys
            .retain(|key| key_set.contains(key.as_str()));
    }
}

fn board_visible_issue_ids(
    payload: &serde_json::Value,
) -> Option<std::collections::BTreeSet<String>> {
    let mut ids = std::collections::BTreeSet::new();
    collect_issue_ids_from_array(payload.get("issues"), &mut ids);
    if !ids.is_empty() {
        return Some(ids);
    }
    if let Some(columns) = payload
        .pointer("/columnsData/columns")
        .or_else(|| payload.get("columns"))
        .and_then(serde_json::Value::as_array)
    {
        for column in columns {
            collect_issue_ids_from_array(column.get("issues"), &mut ids);
            collect_issue_ids_from_array(column.get("issueIds"), &mut ids);
            collect_issue_ids_from_array(column.get("issueKeys"), &mut ids);
        }
    }
    (!ids.is_empty()).then_some(ids)
}

fn collect_issue_ids_from_array(
    value: Option<&serde_json::Value>,
    ids: &mut std::collections::BTreeSet<String>,
) {
    let Some(values) = value.and_then(serde_json::Value::as_array) else {
        return;
    };
    ids.extend(values.iter().filter_map(|value| {
        text_property(value, &["key", "issueKey", "id"]).or_else(|| format_field_value(value))
    }));
}

fn filter_board_card_issues(issues: &mut Vec<IssueSummary>) {
    // Jira issue-type hierarchy levels: epics (and higher) are >= 1, standard
    // issues are 0, and sub-tasks are -1. The board renders standard issues and
    // sub-tasks as cards (Jira nests sub-tasks under their parent), but not
    // epics, which act as a grouping rather than a card. So keep level <= 0.
    if issues
        .iter()
        .any(|issue| issue.field_values.contains_key("typeHierarchyLevel"))
    {
        issues.retain(|issue| {
            issue
                .field_values
                .get("typeHierarchyLevel")
                .and_then(|level| level.parse::<i64>().ok())
                .is_some_and(|level| level <= 0)
        });
    }
}
fn filter_board_visible_issues(
    visible_ids: Option<&std::collections::BTreeSet<String>>,
    issues: &mut Vec<IssueSummary>,
) {
    let Some(visible_ids) = visible_ids else {
        return;
    };
    issues.retain(|issue| {
        visible_ids.contains(&issue.key)
            || issue
                .field_values
                .get("id")
                .is_some_and(|id| visible_ids.contains(id))
    });
}

fn board_issues_from_value(value: Option<&serde_json::Value>) -> Vec<IssueSummary> {
    match value {
        Some(serde_json::Value::Object(issues)) => issues
            .iter()
            .map(|(id, value)| {
                let key = text_property(value, &["key", "issueKey"]).unwrap_or_else(|| id.clone());
                let mut issue = issue_summary_from_board_issue(key.as_str(), value);
                issue
                    .field_values
                    .entry(String::from("id"))
                    .or_insert_with(|| id.clone());
                issue
            })
            .collect(),
        Some(serde_json::Value::Array(issues)) => issues
            .iter()
            .filter_map(|issue| {
                text_property(issue, &["key", "issueKey", "id"])
                    .map(|key| issue_summary_from_board_issue(key.as_str(), issue))
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn issue_summary_from_board_issue(key: &str, value: &serde_json::Value) -> IssueSummary {
    let fields = value.get("fields").unwrap_or(value);
    let status = fields.get("status").or_else(|| value.get("status"));
    let issue_type = fields
        .get("issuetype")
        .or_else(|| fields.get("issueType"))
        .or_else(|| value.get("issueType"));
    let parent = fields.get("parent").or_else(|| value.get("parent"));
    let parent_key = parent
        .and_then(|parent| text_property(parent, &["key", "issueKey", "id"]))
        .or_else(|| text_property(value, &["parentKey", "parentIssueKey"]));
    let mut field_values = BTreeMap::new();
    collect_board_field_values(fields, &mut field_values);
    if let Some(epic_summary) = value
        .get("epicField")
        .or_else(|| fields.get("epicField"))
        .and_then(|epic| text_property(epic, &["summary", "epicKey"]))
    {
        field_values.insert(String::from("epic_summary"), epic_summary);
    }
    if !std::ptr::eq(fields, value) {
        collect_board_field_values(value, &mut field_values);
    }
    if let Some(status_id) = status
        .and_then(|status| text_property(status, &["id", "statusId"]))
        .or_else(|| text_property(fields, &["statusId"]))
        .or_else(|| text_property(value, &["statusId"]))
    {
        field_values.insert(String::from("status_id"), status_id);
    }
    if let Some(swimlane_id) =
        text_property(value, &["swimlaneId"]).or_else(|| text_property(fields, &["swimlaneId"]))
    {
        field_values.insert(String::from("swimlane_id"), swimlane_id);
    }
    if let Some(parent_key) = &parent_key {
        field_values.insert(String::from("parent"), parent_key.clone());
    }
    // Greenhopper supplies a resolved display name in `assigneeName` even when the
    // raw `assignee` token is an opaque id (accountId, `ug:` group, or a legacy
    // username). Prefer it so grouping/labels never show raw ids.
    if let Some(assignee_name) = field_values
        .get("assigneeName")
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
    {
        field_values.insert(String::from("assignee"), assignee_name);
    }

    IssueSummary {
        key: key.to_owned(),
        summary: text_property(fields, &["summary"])
            .or_else(|| text_property(value, &["summary"]))
            .unwrap_or_else(|| key.to_owned()),
        status: status
            .and_then(|status| text_property(status, &["name", "statusName"]))
            .or_else(|| text_property(value, &["statusName"]))
            .unwrap_or_else(|| String::from("Unknown")),
        issue_type: issue_type
            .and_then(|issue_type| text_property(issue_type, &["name", "typeName"]))
            .or_else(|| text_property(value, &["typeName", "issueTypeName"]))
            .unwrap_or_else(|| String::from("Issue")),
        parent_key,
        has_children: false,
        field_values,
    }
}

fn collect_board_field_values(
    value: &serde_json::Value,
    field_values: &mut BTreeMap<String, String>,
) {
    let Some(object) = value.as_object() else {
        return;
    };
    for (key, value) in object {
        if matches!(
            key.as_str(),
            "fields" | "summary" | "status" | "issuetype" | "issueType"
        ) {
            continue;
        }
        if let Some(text) = format_field_value(value) {
            field_values.entry(key.clone()).or_insert(text);
        }
    }
}

fn text_property(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(format_field_value))
}

fn format_field_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(text) => (!text.is_empty()).then(|| text.clone()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        serde_json::Value::Bool(value) => Some(value.to_string()),
        serde_json::Value::Array(values) => {
            let text = values
                .iter()
                .filter_map(format_field_value)
                .collect::<Vec<_>>()
                .join(", ");
            (!text.is_empty()).then_some(text)
        }
        serde_json::Value::Object(object) => {
            ["displayName", "name", "key", "value", "emailAddress"]
                .iter()
                .find_map(|key| object.get(*key).and_then(format_field_value))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::{
        IssueSummary, JiraError, ParentProbeResponse, SearchResponse, board_data_from_greenhopper,
        group_children_for_parents, issue_summary_from_search_issue, log_path, search_match_clause,
    };
    use reqwest::Url;
    use serde_json::json;
    use std::collections::{BTreeMap, HashSet};

    fn child_issue(key: &str, parent: &str) -> IssueSummary {
        let mut issue = IssueSummary {
            key: key.to_owned(),
            summary: String::from("child"),
            status: String::from("To Do"),
            issue_type: String::from("Story"),
            parent_key: Some(parent.to_owned()),
            has_children: false,
            field_values: BTreeMap::new(),
        };
        issue
            .field_values
            .insert(String::from("parent"), parent.to_owned());
        issue
    }

    #[test]
    fn group_children_buckets_by_parent_and_fills_empty_parents() {
        let parents = vec![
            String::from("KAN-1"),
            String::from("KAN-2"),
            String::from("KAN-3"),
        ];
        let issues = vec![
            child_issue("KAN-10", "KAN-1"),
            child_issue("KAN-11", "KAN-1"),
            child_issue("KAN-20", "KAN-2"),
        ];

        let groups = group_children_for_parents(&parents, issues, &HashSet::new(), None);

        let keys: Vec<&str> = groups.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, vec!["KAN-1", "KAN-2", "KAN-3"], "order matches input");
        let kan1 = groups[0].1.as_ref().expect("KAN-1 ok");
        assert_eq!(
            kan1.iter().map(|i| i.key.as_str()).collect::<Vec<_>>(),
            vec!["KAN-10", "KAN-11"]
        );
        assert_eq!(groups[1].1.as_ref().expect("KAN-2 ok").len(), 1);
        // KAN-3 had no children in the response -> empty, so a stale subtree clears.
        assert!(groups[2].1.as_ref().expect("KAN-3 ok").is_empty());
    }

    #[test]
    fn group_children_marks_failed_parents_as_errors() {
        let parents = vec![String::from("KAN-1"), String::from("KAN-2")];
        let mut failed = HashSet::new();
        failed.insert(String::from("KAN-2"));
        let error = JiraError(String::from("boom"));

        let groups = group_children_for_parents(
            &parents,
            vec![child_issue("KAN-10", "KAN-1")],
            &failed,
            Some(&error),
        );

        assert!(groups[0].1.is_ok(), "succeeding parent keeps its children");
        // The failed parent surfaces the error so its stale subtree is preserved.
        assert_eq!(groups[1].1.as_ref().unwrap_err().0, "boom");
    }

    #[test]
    fn search_clause_matches_each_word_as_a_prefix() {
        assert_eq!(
            search_match_clause("admin adjustmen").as_deref(),
            Some("summary ~ \"admin*\" AND summary ~ \"adjustmen*\"")
        );
    }

    #[test]
    fn search_clause_drops_punctuation_only_tokens() {
        // The lone "-" carries no searchable text and must not become an operator.
        assert_eq!(
            search_match_clause("admin - adjustment").as_deref(),
            Some("summary ~ \"admin*\" AND summary ~ \"adjustment*\"")
        );
    }

    #[test]
    fn search_clause_matches_issue_keys_by_key_or_summary() {
        assert_eq!(
            search_match_clause("dpp-2193").as_deref(),
            Some("(key = \"DPP-2193\" OR summary ~ \"dpp2193*\")")
        );
    }

    #[test]
    fn search_clause_is_none_when_nothing_searchable() {
        assert_eq!(search_match_clause("   ").as_deref(), None);
        assert_eq!(search_match_clause("-").as_deref(), None);
    }

    #[test]
    fn log_path_decodes_query_values_for_readable_command_log() {
        let url = Url::parse("https://example.atlassian.net/rest/api/3/search/jql?jql=project%20%3D%20KAN&fields=summary%2Cstatus").expect("url");

        assert_eq!(
            log_path(&url),
            "/search/jql?jql=project = KAN&fields=summary,status"
        );
    }

    #[test]
    fn issue_summary_preserves_dynamic_jira_fields() {
        let payload: SearchResponse = serde_json::from_value(json!({
            "issues": [{
                "key": "KAN-33",
                "fields": {
                    "summary": "Order history",
                    "status": { "name": "To Do" },
                    "issuetype": { "name": "Story" },
                    "parent": { "key": "KAN-21" },
                    "assignee": { "displayName": "Marlo Vlietstra" },
                    "fixVersions": [{ "name": "v1" }, { "name": "v2" }],
                    "timespent": 120
                }
            }]
        }))
        .expect("search response");

        let issue =
            issue_summary_from_search_issue(payload.issues.into_iter().next().expect("issue"));

        assert_eq!(issue.parent_key.as_deref(), Some("KAN-21"));
        assert_eq!(
            issue.field_values.get("parent").map(String::as_str),
            Some("KAN-21")
        );
        assert_eq!(
            issue.field_values.get("assignee").map(String::as_str),
            Some("Marlo Vlietstra")
        );
        assert_eq!(
            issue.field_values.get("fixVersions").map(String::as_str),
            Some("v1, v2")
        );
        assert_eq!(
            issue.field_values.get("timespent").map(String::as_str),
            Some("120")
        );
    }

    #[test]
    fn greenhopper_board_data_preserves_columns_swimlanes_and_status_mapping() {
        let board = board_data_from_greenhopper(
            7,
            String::from("Kanban"),
            json!({
                "columnsData": {
                    "columns": [
                        { "name": "To Do", "position": 0, "statusIds": ["100"] },
                        { "name": "Done", "position": 1, "statusIds": ["300"] }
                    ]
                },
                "issuesData": {
                    "issues": [
                        {
                            "id": 10001,
                            "key": "KAN-1",
                            "summary": "Browse catalog",
                            "statusId": "100",
                            "status": { "name": "To Do" },
                            "typeName": "Story",
                            "swimlaneId": "11"
                        },
                        {
                            "key": "KAN-2",
                            "summary": "Checkout",
                            "statusId": "300",
                            "status": { "name": "Done" },
                            "typeName": "Task",
                            "swimlaneId": "12"
                        }
                    ]
                },
                "swimlanesData": {
                    "swimlanes": [
                        { "swimlaneId": "11", "name": "Shopping cart", "position": 0, "issueIds": [10001] },
                        { "swimlaneId": "12", "name": "Payments", "position": 1 }
                    ]
                }
            }),
        )
        .expect("board data");

        assert_eq!(board.id, 7);
        assert_eq!(board.columns[0].name, "To Do");
        assert_eq!(board.columns[0].statuses, vec!["100"]);
        assert_eq!(board.swimlanes[0].name, "Shopping cart");
        assert_eq!(board.swimlanes[0].issue_keys, vec!["KAN-1"]);
        assert_eq!(board.swimlanes[1].issue_keys, vec!["KAN-2"]);
        assert_eq!(board.swimlanes.len(), 2);
        assert_eq!(
            board.issues[0]
                .field_values
                .get("status_id")
                .map(String::as_str),
            Some("100")
        );
    }

    #[test]
    fn greenhopper_board_data_reads_column_wip_maximum() {
        let board = board_data_from_greenhopper(
            7,
            String::from("Kanban"),
            json!({
                "columnsData": {
                    "columns": [
                        { "name": "To Do", "position": 0, "statusIds": ["100"], "max": 5 },
                        { "name": "In Progress", "position": 1, "statusIds": ["200"], "max": 0 },
                        { "name": "In Review", "position": 2, "statusIds": ["250"], "max": "8" },
                        { "name": "Staging", "position": 3, "statusIds": ["275"], "maxIssueCount": 4 },
                        { "name": "Done", "position": 4, "statusIds": ["300"] }
                    ]
                },
                "issuesData": { "issues": [] },
                "swimlanesData": { "swimlanes": [] }
            }),
        )
        .expect("board data");

        // A positive `max` is the WIP limit; 0 or absent means no limit. The
        // value may be a number, a numeric string, or under `maxIssueCount`.
        assert_eq!(board.columns[0].max, Some(5));
        assert_eq!(board.columns[1].max, None);
        assert_eq!(board.columns[2].max, Some(8));
        assert_eq!(board.columns[3].max, Some(4));
        assert_eq!(board.columns[4].max, None);
    }

    #[test]
    fn greenhopper_board_data_extracts_active_sprint_with_formatted_dates() {
        let board = board_data_from_greenhopper(
            7,
            String::from("Scrum"),
            json!({
                "columnsData": { "columns": [{ "name": "To Do", "position": 0, "statusIds": ["100"] }] },
                "issuesData": { "issues": [] },
                "swimlanesData": { "swimlanes": [] },
                "sprintsData": {
                    "sprints": [
                        {
                            "id": 1,
                            "name": "DICE Sprint 195",
                            "state": "CLOSED",
                            "isoStartDate": "2026-05-20T10:30:00+0200",
                            "isoEndDate": "2026-06-03T00:00:00+0200",
                            "daysRemaining": 0
                        },
                        {
                            "id": 2,
                            "name": "DICE Sprint 196",
                            "state": "ACTIVE",
                            "goal": "Publish offer drafts end-to-end.",
                            "isoStartDate": "2026-06-03T10:30:55+0200",
                            "isoEndDate": "2026-06-17T00:00:00+0200",
                            "daysRemaining": 4
                        }
                    ]
                }
            }),
        )
        .expect("board data");

        let sprint = board.sprint.expect("active sprint");
        assert_eq!(sprint.name, "DICE Sprint 196");
        assert_eq!(
            sprint.goal.as_deref(),
            Some("Publish offer drafts end-to-end.")
        );
        assert_eq!(sprint.days_remaining, Some(4));
        assert_eq!(sprint.start_date.as_deref(), Some("Jun 3, 2026"));
        assert_eq!(sprint.end_date.as_deref(), Some("Jun 17, 2026"));
    }

    #[test]
    fn greenhopper_board_data_has_no_sprint_when_payload_omits_one() {
        let board = board_data_from_greenhopper(
            7,
            String::from("Kanban"),
            json!({
                "columnsData": { "columns": [{ "name": "To Do", "position": 0, "statusIds": ["100"] }] },
                "issuesData": { "issues": [] },
                "swimlanesData": { "swimlanes": [] }
            }),
        )
        .expect("board data");

        assert_eq!(board.sprint, None);
    }

    #[test]
    fn greenhopper_board_data_resolves_assignee_from_assignee_name() {
        let board = board_data_from_greenhopper(
            7,
            String::from("Kanban"),
            json!({
                "columnsData": {
                    "columns": [
                        { "name": "To Do", "position": 0, "statusIds": ["100"] }
                    ]
                },
                "issuesData": {
                    "issues": [
                        {
                            "key": "KAN-1",
                            "summary": "Opaque group token",
                            "statusId": "100",
                            "status": { "name": "To Do" },
                            "typeName": "Bug",
                            "assignee": "ug:65a754c5-890a-4b03-9d35-42318da7416d",
                            "assigneeName": "Thang Nguyen The"
                        },
                        {
                            "key": "KAN-2",
                            "summary": "Legacy username token",
                            "statusId": "100",
                            "status": { "name": "To Do" },
                            "typeName": "Task",
                            "assignee": "astrid.leckebusch",
                            "assigneeName": "Astrid Leckebusch"
                        },
                        {
                            "key": "KAN-3",
                            "summary": "No assignee",
                            "statusId": "100",
                            "status": { "name": "To Do" },
                            "typeName": "Task"
                        }
                    ]
                }
            }),
        )
        .expect("board data");

        let by_key = |key: &str| {
            board
                .issues
                .iter()
                .find(|issue| issue.key == key)
                .unwrap_or_else(|| panic!("issue {key}"))
        };

        // Opaque assignee tokens are replaced with the display name greenhopper
        // already supplies in `assigneeName`, never shown raw.
        assert_eq!(
            by_key("KAN-1")
                .field_values
                .get("assignee")
                .map(String::as_str),
            Some("Thang Nguyen The")
        );
        assert_eq!(
            by_key("KAN-2")
                .field_values
                .get("assignee")
                .map(String::as_str),
            Some("Astrid Leckebusch")
        );
        // Unassigned issues keep no assignee value (fall through to "Unassigned").
        assert_eq!(by_key("KAN-3").field_values.get("assignee"), None);
    }

    #[test]
    fn greenhopper_board_data_uses_issue_key_when_issue_map_is_keyed_by_id() {
        let board = board_data_from_greenhopper(
            7,
            String::from("Kanban"),
            json!({
                "columnsData": {
                    "columns": [
                        { "name": "To Do", "position": 0, "statusIds": ["100"] }
                    ]
                },
                "issuesData": {
                    "issues": {
                        "10001": {
                            "id": 10001,
                            "key": "KAN-1",
                            "summary": "Browse catalog",
                            "statusId": "100",
                            "status": { "name": "To Do" },
                            "typeName": "Story"
                        }
                    }
                },
                "swimlanesData": {
                    "swimlanes": [
                        { "swimlaneId": "11", "name": "Shopping cart", "position": 0, "issueIds": [10001] }
                    ]
                }
            }),
        )
        .expect("board data");

        assert_eq!(board.issues[0].key, "KAN-1");
        assert_eq!(board.swimlanes[0].issue_keys, vec!["KAN-1"]);
    }

    #[test]
    fn greenhopper_board_data_keeps_only_root_visible_issues() {
        let board = board_data_from_greenhopper(
            7,
            String::from("Kanban"),
            json!({
                "columnsData": {
                    "columns": [
                        { "name": "To Do", "position": 0, "statusIds": ["100"] }
                    ]
                },
                "issues": [10001],
                "issuesData": {
                    "10001": {
                        "key": "KAN-1",
                        "summary": "Visible card",
                        "statusId": "100",
                        "status": { "name": "To Do" },
                        "typeName": "Story"
                    },
                    "10002": {
                        "key": "KAN-2",
                        "summary": "Hidden raw issue",
                        "statusId": "100",
                        "status": { "name": "To Do" },
                        "typeName": "Subtask"
                    }
                },
                "swimlanesData": {
                    "swimlanes": [
                        { "id": 11, "name": "Visible", "position": 0, "issues": [10001, 10002] }
                    ]
                }
            }),
        )
        .expect("board data");

        assert_eq!(
            board
                .issues
                .iter()
                .map(|issue| issue.key.as_str())
                .collect::<Vec<_>>(),
            vec!["KAN-1"]
        );
        assert_eq!(board.swimlanes[0].issue_keys, vec!["KAN-1"]);
    }

    #[test]
    fn greenhopper_board_data_keeps_cards_and_subtasks_but_drops_epics() {
        let board = board_data_from_greenhopper(
            7,
            String::from("Kanban"),
            json!({
                "columnsData": {
                    "columns": [
                        { "name": "To Do", "position": 0, "statusIds": ["100"] }
                    ]
                },
                "issuesData": {
                    "issues": [
                        {
                            "id": 10001,
                            "key": "KAN-1",
                            "summary": "Visible story",
                            "statusId": "100",
                            "status": { "name": "To Do" },
                            "typeName": "Story",
                            "typeHierarchyLevel": 0
                        },
                        {
                            "id": 10002,
                            "key": "KAN-2",
                            "summary": "Epic hidden from board card list",
                            "statusId": "100",
                            "status": { "name": "To Do" },
                            "typeName": "Epic",
                            "typeHierarchyLevel": 1
                        },
                        {
                            "id": 10003,
                            "key": "KAN-3",
                            "summary": "Subtask shown as a board card",
                            "statusId": "100",
                            "status": { "name": "To Do" },
                            "typeName": "Subtask",
                            "typeHierarchyLevel": -1
                        }
                    ]
                },
                "swimlanesData": {
                    "swimlanes": [
                        { "id": 11, "name": "Visible", "position": 0, "issues": [10001, 10002, 10003] }
                    ]
                }
            }),
        )
        .expect("board data");

        // Story and sub-task are cards; the epic is excluded.
        assert_eq!(
            board
                .issues
                .iter()
                .map(|issue| issue.key.as_str())
                .collect::<Vec<_>>(),
            vec!["KAN-1", "KAN-3"]
        );
        assert_eq!(board.swimlanes[0].issue_keys, vec!["KAN-1", "KAN-3"]);
    }
    fn search_response_reads_pagination_cursor() {
        let payload: SearchResponse = serde_json::from_value(json!({
            "issues": [],
            "nextPageToken": "abc123",
            "isLast": false
        }))
        .expect("search response");

        assert_eq!(payload.next_page_token.as_deref(), Some("abc123"));
        assert!(!payload.is_last);
    }

    #[test]
    fn search_response_defaults_pagination_when_absent() {
        let payload: SearchResponse =
            serde_json::from_value(json!({ "issues": [] })).expect("search response");

        assert_eq!(payload.next_page_token, None);
        assert!(!payload.is_last);
    }

    #[test]
    fn parent_probe_parses_parent_only_payload_and_collects_parents() {
        // The probe requests `fields=parent`, so issues have no summary/status/
        // issuetype. A strict issue parser would fail here, silently hiding all
        // chevrons; this minimal parser must succeed.
        let payload: ParentProbeResponse = serde_json::from_value(json!({
            "issues": [
                { "key": "KAN-2", "fields": { "parent": { "key": "KAN-1" } } },
                { "key": "KAN-3", "fields": { "parent": { "key": "KAN-1" } } },
                { "key": "KAN-9", "fields": { "parent": null } }
            ]
        }))
        .expect("parent probe response");

        let parents: Vec<Option<String>> = payload
            .issues
            .into_iter()
            .map(|issue| issue.fields.parent.map(|parent| parent.key))
            .collect();
        assert_eq!(
            parents,
            vec![
                Some(String::from("KAN-1")),
                Some(String::from("KAN-1")),
                None
            ]
        );
    }
}

use std::collections::{BTreeSet, HashSet};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use reqwest::Url;
use serde::de::DeserializeOwned;

use crate::config::JiraCredentials;

mod greenhopper;
mod protocol;
mod query;

pub use crate::domain::models::*;
pub use query::{CHILD_PAGE_SIZE, ChildrenBatch, ROOT_PAGE_SIZE};

use greenhopper::board_data_from_greenhopper;
use greenhopper::format_field_value;
use greenhopper::timeline_epics_from_greenhopper;
use protocol::{
    AssignIssuePayload, AssignableUser, BoardDetails, BoardSearchResponse, FieldDetails,
    ParentProbeResponse, ProjectSearchResponse, RankIssuePayload, SearchResponse,
    TransitionIssueId, TransitionIssuePayload, TransitionsResponse,
};
use query::{
    BATCH_PARENT_CHUNK, group_children_for_parents, issue_summary_from_search_issue, log_path,
    search_match_clause,
};

impl crate::ui::selector::HasShortcut for ProjectSummary {
    fn shortcut(&self, _keybindings: &crate::KeyBindings) -> Option<String> {
        None
    }
}

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
pub struct TimelineLoadResult {
    pub timeline: Result<TimelineData, JiraError>,
    pub logs: Vec<CommandLogEntry>,
}

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
            let log = response_log(timestamp, method, path, status, duration_ms);

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
            transport_error_log(timestamp, method, path, duration_ms),
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
            let log = response_log(timestamp, method, path, status, duration_ms);

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
            log: transport_error_log(timestamp, method, path, duration_ms),
        },
    }
}

/// Number of recently-updated issues sampled to discover which fields actually
/// carry a value for the project, so the column picker can hide the (often
/// dozens of) instance-wide custom fields that are never populated here.
const FIELD_SAMPLE_SIZE: u32 = 50;

/// Samples the most recently updated issues with every navigable field and
/// returns the set of field IDs that actually carry a value. Returns `None`
/// when the sample request fails so callers fall back to offering every field
/// rather than hiding them all.
pub fn load_populated_field_ids(
    credentials: &JiraCredentials,
) -> (Option<BTreeSet<String>>, CommandLogEntry) {
    let project = credentials.default_project.trim();
    let jql = format!("project = {project} ORDER BY updated DESC");
    let (result, log) =
        fetch_search::<SearchResponse>(credentials, &jql, "*navigable", None, FIELD_SAMPLE_SIZE);
    match result {
        Ok(payload) => {
            let ids = payload
                .issues
                .into_iter()
                .flat_map(|issue| issue.fields.extra.into_iter())
                .filter(|(_, value)| format_field_value(value).is_some())
                .map(|(id, _)| id)
                .collect();
            (Some(ids), log)
        }
        Err(_) => (None, log),
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
            let log = response_log(timestamp, method, path, status, duration_ms);

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
            log: transport_error_log(timestamp, method, path, duration_ms),
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

/// Loads the project's first board, then the timeline behind it: epics with
/// child counts (greenhopper backlog) joined to dated sprints (agile sprint
/// endpoint). Sprint dates are best-effort — if that fetch fails the epics and
/// their progress still render, just without axis bars.
pub fn load_project_timeline(credentials: &JiraCredentials) -> TimelineLoadResult {
    let board_search = load_project_boards(credentials);
    let mut logs = vec![board_search.log];

    let board = match board_search.board {
        Ok(board) => board,
        Err(error) => {
            return TimelineLoadResult {
                timeline: Err(error),
                logs,
            };
        }
    };

    // Fetch the (large) backlog epics and the sprint history concurrently —
    // both only need the board id, so there's no reason to serialize them.
    let (epics_load, (sprints, sprint_logs)) = std::thread::scope(|scope| {
        let epics = scope.spawn(|| load_greenhopper_timeline_epics(credentials, board.id));
        let sprints = scope.spawn(|| load_board_sprints(credentials, board.id));
        (
            epics.join().unwrap_or_else(|_| TimelineEpicsLoad {
                epics: Err(JiraError(String::from(
                    "Jira timeline worker thread panicked",
                ))),
                log: failed_log(
                    current_time_string(),
                    "GET",
                    "/xboard/plan/backlog/data.json",
                ),
            }),
            sprints.join().unwrap_or_else(|_| {
                (
                    Err(JiraError(String::from(
                        "Jira sprints worker thread panicked",
                    ))),
                    Vec::new(),
                )
            }),
        )
    });
    logs.push(epics_load.log);
    logs.extend(sprint_logs);
    let epics = match epics_load.epics {
        Ok(epics) => epics,
        Err(error) => {
            return TimelineLoadResult {
                timeline: Err(error),
                logs,
            };
        }
    };

    TimelineLoadResult {
        timeline: Ok(TimelineData {
            epics,
            sprints: sprints.unwrap_or_default(),
        }),
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

struct TimelineEpicsLoad {
    epics: Result<Vec<TimelineEpic>, JiraError>,
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
            let log = response_log(timestamp, method, path, status, duration_ms);
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
            log: transport_error_log(timestamp, method, path, duration_ms),
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
            let log = response_log(timestamp, method, path, status, duration_ms);
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
            log: transport_error_log(timestamp, method, path, duration_ms),
        },
    }
}

/// Fetches the greenhopper backlog data for a board and maps it into timeline
/// epics. Uses the backlog endpoint (not `work/allData`) because only the
/// backlog payload carries `epicData` with rolled-up child counts.
fn load_greenhopper_timeline_epics(
    credentials: &JiraCredentials,
    board_id: u64,
) -> TimelineEpicsLoad {
    let site = credentials.site.trim().trim_end_matches('/');
    let rapid_view_id = board_id.to_string();
    let query = [("rapidViewId", rapid_view_id.as_str())];
    let method = "GET";
    let timestamp = current_time_string();
    let url = match Url::parse_with_params(
        &format!("{site}/rest/greenhopper/1.0/xboard/plan/backlog/data.json"),
        &query,
    ) {
        Ok(url) => url,
        Err(error) => {
            return TimelineEpicsLoad {
                epics: Err(JiraError(format!("Invalid Jira site URL: {error}"))),
                log: failed_log(timestamp, method, "/xboard/plan/backlog/data.json"),
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
            let log = response_log(timestamp, method, path, status, duration_ms);
            if !status.is_success() {
                return TimelineEpicsLoad {
                    epics: Err(JiraError(format!("Jira returned HTTP {status}"))),
                    log,
                };
            }

            let epics = response
                .json::<serde_json::Value>()
                .map_err(|error| {
                    JiraError(format!("Jira timeline data could not be read: {error}"))
                })
                .map(|payload| timeline_epics_from_greenhopper(&payload));
            TimelineEpicsLoad { epics, log }
        }
        Err(error) => TimelineEpicsLoad {
            epics: Err(JiraError(format!("Jira timeline request failed: {error}"))),
            log: transport_error_log(timestamp, method, path, duration_ms),
        },
    }
}

/// Maximum number of agile sprint pages to fetch (50 sprints each). Caps the
/// request count for boards with long histories; recent sprints sit on the last
/// pages, so paging continues until `isLast`.
const SPRINT_PAGE_CAP: u32 = 12;
const SPRINT_PAGE_SIZE: u32 = 50;

/// Loads every dated sprint for a board from the agile sprint endpoint, paging
/// until `isLast` or the page cap. The endpoint returns oldest-first; the UI
/// windows to the relevant range. Best-effort: a failed page returns whatever
/// was gathered so far alongside the error log.
fn load_board_sprints(
    credentials: &JiraCredentials,
    board_id: u64,
) -> (Result<Vec<TimelineSprint>, JiraError>, Vec<CommandLogEntry>) {
    let site = credentials.site.trim().trim_end_matches('/');
    let method = "GET";
    let mut sprints = Vec::new();
    let mut logs = Vec::new();
    let mut start_at: u32 = 0;

    for _ in 0..SPRINT_PAGE_CAP {
        let start_at_str = start_at.to_string();
        let max_results = SPRINT_PAGE_SIZE.to_string();
        let query = [
            ("startAt", start_at_str.as_str()),
            ("maxResults", max_results.as_str()),
        ];
        let timestamp = current_time_string();
        let endpoint = format!("{site}/rest/agile/1.0/board/{board_id}/sprint");
        let url = match Url::parse_with_params(&endpoint, &query) {
            Ok(url) => url,
            Err(error) => {
                logs.push(failed_log(timestamp, method, "/board/sprint"));
                return (
                    Err(JiraError(format!("Invalid Jira site URL: {error}"))),
                    logs,
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

        let response = match response {
            Ok(response) => response,
            Err(error) => {
                logs.push(transport_error_log(timestamp, method, path, duration_ms));
                return (
                    Err(JiraError(format!("Jira sprints request failed: {error}"))),
                    logs,
                );
            }
        };
        let status = response.status();
        logs.push(response_log(timestamp, method, path, status, duration_ms));
        if !status.is_success() {
            return (Err(JiraError(format!("Jira returned HTTP {status}"))), logs);
        }

        let page = match response.json::<serde_json::Value>() {
            Ok(page) => page,
            Err(error) => {
                return (
                    Err(JiraError(format!(
                        "Jira sprints could not be read: {error}"
                    ))),
                    logs,
                );
            }
        };
        if let Some(values) = page.get("values").and_then(serde_json::Value::as_array) {
            sprints.extend(values.iter().filter_map(timeline_sprint_from_agile));
        }
        let is_last = page
            .get("isLast")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        if is_last {
            break;
        }
        start_at += SPRINT_PAGE_SIZE;
    }

    (Ok(sprints), logs)
}

/// Maps one agile sprint JSON object into a `TimelineSprint`, dropping sprints
/// without a usable id. Start/end dates are parsed to day-numbers; an undated
/// (e.g. future) sprint keeps `None` endpoints and is left off the axis.
fn timeline_sprint_from_agile(value: &serde_json::Value) -> Option<TimelineSprint> {
    let id = value.get("id").and_then(serde_json::Value::as_i64)?;
    let state = match value.get("state").and_then(serde_json::Value::as_str) {
        Some(state) if state.eq_ignore_ascii_case("active") => SprintState::Active,
        Some(state) if state.eq_ignore_ascii_case("closed") => SprintState::Closed,
        _ => SprintState::Future,
    };
    let day = |field: &str| {
        value
            .get(field)
            .and_then(serde_json::Value::as_str)
            .and_then(crate::domain::date::iso_to_days)
    };
    Some(TimelineSprint {
        id,
        name: value
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| String::from("Sprint")),
        state,
        start_day: day("startDate"),
        end_day: day("endDate").or_else(|| day("completeDate")),
    })
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
            let log = response_log(timestamp, method, log_path, status, duration_ms);
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
                log: transport_error_log(timestamp, method, log_path, duration_ms),
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
            let log = response_log(timestamp, method, log_path, status, duration_ms);

            if status.is_success() {
                Ok(log)
            } else {
                Err((JiraError(format!("Jira returned HTTP {status}")), log))
            }
        }
        Err(error) => Err((
            JiraError(format!("Jira assignment request failed: {error}")),
            transport_error_log(timestamp, method, log_path, duration_ms),
        )),
    }
}

pub fn transition_issue_to_status(
    credentials: &JiraCredentials,
    issue_key: &str,
    target_status: &str,
    target_status_id: Option<&str>,
) -> Result<Vec<CommandLogEntry>, (JiraError, Vec<CommandLogEntry>)> {
    let mut logs = Vec::new();
    let site = credentials.site.trim().trim_end_matches('/');
    let transitions_path = format!("/issue/{issue_key}/transitions");
    let timestamp = current_time_string();
    let transitions_url = match Url::parse(&format!("{site}/rest/api/3{transitions_path}")) {
        Ok(url) => url,
        Err(error) => {
            return Err((
                JiraError(format!("Invalid Jira site URL: {error}")),
                vec![failed_log(timestamp, "GET", transitions_path.as_str())],
            ));
        }
    };

    let started_at = Instant::now();
    let response = reqwest::blocking::Client::new()
        .get(transitions_url.clone())
        .basic_auth(credentials.email.trim(), Some(credentials.api_key.trim()))
        .send();
    let duration_ms = started_at.elapsed().as_millis();
    let transitions_log_path = log_path(&transitions_url);
    let response = match response {
        Ok(response) => response,
        Err(error) => {
            return Err((
                JiraError(format!("Jira transition lookup failed: {error}")),
                vec![transport_error_log(
                    timestamp,
                    "GET",
                    transitions_log_path,
                    duration_ms,
                )],
            ));
        }
    };
    let status = response.status();
    logs.push(response_log(
        timestamp,
        "GET",
        transitions_log_path,
        status,
        duration_ms,
    ));
    if !status.is_success() {
        return Err((JiraError(format!("Jira returned HTTP {status}")), logs));
    }
    let transitions = match response.json::<TransitionsResponse>() {
        Ok(payload) => payload.transitions,
        Err(error) => {
            return Err((
                JiraError(format!("Jira transitions could not be read: {error}")),
                logs,
            ));
        }
    };
    let transition = if let Some(target_status_id) = target_status_id {
        transitions
            .iter()
            .find(|transition| transition.to.id == target_status_id)
    } else {
        transitions.iter().find(|transition| {
            transition.to.name.eq_ignore_ascii_case(target_status)
                || transition.name.eq_ignore_ascii_case(target_status)
        })
    };
    let Some(transition) = transition else {
        return Err((
            JiraError(format!(
                "No Jira transition found for status {target_status}"
            )),
            logs,
        ));
    };

    let timestamp = current_time_string();
    let transition_id = transition.id.clone();
    let transition_url = match Url::parse(&format!("{site}/rest/api/3{transitions_path}")) {
        Ok(url) => url,
        Err(error) => {
            logs.push(failed_log(timestamp, "POST", transitions_path.as_str()));
            return Err((JiraError(format!("Invalid Jira site URL: {error}")), logs));
        }
    };
    let started_at = Instant::now();
    let response = reqwest::blocking::Client::new()
        .post(transition_url.clone())
        .basic_auth(credentials.email.trim(), Some(credentials.api_key.trim()))
        .json(&TransitionIssuePayload {
            transition: TransitionIssueId { id: &transition_id },
        })
        .send();
    let duration_ms = started_at.elapsed().as_millis();
    let log_path = log_path(&transition_url);
    match response {
        Ok(response) => {
            let status = response.status();
            logs.push(response_log(
                timestamp,
                "POST",
                log_path,
                status,
                duration_ms,
            ));
            if status.is_success() {
                Ok(logs)
            } else {
                Err((JiraError(format!("Jira returned HTTP {status}")), logs))
            }
        }
        Err(error) => {
            logs.push(transport_error_log(
                timestamp,
                "POST",
                log_path,
                duration_ms,
            ));
            Err((
                JiraError(format!("Jira transition request failed: {error}")),
                logs,
            ))
        }
    }
}

pub fn rank_issue(
    credentials: &JiraCredentials,
    issue_key: &str,
    rank_before: Option<&str>,
    rank_after: Option<&str>,
) -> Result<CommandLogEntry, (JiraError, CommandLogEntry)> {
    let site = credentials.site.trim().trim_end_matches('/');
    let method = "PUT";
    let path = "/issue/rank";
    let timestamp = current_time_string();
    let url = match Url::parse(&format!("{site}/rest/agile/1.0{path}")) {
        Ok(url) => url,
        Err(error) => {
            return Err((
                JiraError(format!("Invalid Jira site URL: {error}")),
                failed_log(timestamp, method, path),
            ));
        }
    };
    let started_at = Instant::now();
    let response = reqwest::blocking::Client::new()
        .put(url.clone())
        .basic_auth(credentials.email.trim(), Some(credentials.api_key.trim()))
        .json(&RankIssuePayload {
            issues: vec![issue_key],
            rank_before_issue: rank_before,
            rank_after_issue: rank_after,
        })
        .send();
    let duration_ms = started_at.elapsed().as_millis();
    let log_path = log_path(&url);
    match response {
        Ok(response) => {
            let status = response.status();
            let log = response_log(timestamp, method, log_path, status, duration_ms);
            if status.is_success() {
                Ok(log)
            } else {
                Err((JiraError(format!("Jira returned HTTP {status}")), log))
            }
        }
        Err(error) => Err((
            JiraError(format!("Jira rank request failed: {error}")),
            transport_error_log(timestamp, method, log_path, duration_ms),
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

fn response_log(
    timestamp: String,
    method: &'static str,
    path: String,
    status: reqwest::StatusCode,
    duration_ms: u128,
) -> CommandLogEntry {
    CommandLogEntry {
        timestamp,
        method,
        path,
        status: status.as_u16().to_string(),
        duration_ms,
    }
}

fn transport_error_log(
    timestamp: String,
    method: &'static str,
    path: String,
    duration_ms: u128,
) -> CommandLogEntry {
    CommandLogEntry {
        timestamp,
        method,
        path,
        status: String::from("ERR"),
        duration_ms,
    }
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

#[cfg(test)]
mod tests {
    use super::query::log_path;
    use reqwest::Url;

    #[test]
    fn log_path_decodes_query_values_for_readable_command_log() {
        let url = Url::parse("https://example.atlassian.net/rest/api/3/search/jql?jql=project%20%3D%20KAN&fields=summary%2Cstatus").expect("url");

        assert_eq!(
            log_path(&url),
            "/search/jql?jql=project = KAN&fields=summary,status"
        );
    }
}

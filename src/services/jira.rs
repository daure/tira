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

/// Issues per page when walking the root issue list.
pub const ROOT_PAGE_SIZE: u32 = 100;
/// Upper bound for a single child probe / child fetch response.
const CHILD_PAGE_SIZE: u32 = 5_000;

/// Loads one page of top-level issues (no parent) for the default project,
/// most-recently-updated first, then probes which of them have children so the
/// tree can show expansion chevrons without fetching the children themselves.
pub fn load_root_issues(
    credentials: &JiraCredentials,
    fields: &str,
    page_token: Option<&str>,
) -> JiraLoadResult {
    let jql = format!(
        "project = {} AND parent IS EMPTY ORDER BY updated DESC",
        credentials.default_project.trim()
    );
    let (issues, token, log) = run_search(credentials, &jql, fields, page_token, ROOT_PAGE_SIZE);
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

/// Runs a server-side text search over the default project. Results are a flat
/// list (no hierarchy), so children are not probed.
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
        Some([("project", credentials.default_project.trim())].as_slice()),
    );
    let current = load_users_endpoint(credentials, "GET", "/myself", None);

    JiraUsersResult {
        users: assignable.users,
        current_user: current.current_user,
        logs: vec![assignable.log, current.log],
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
        ParentProbeResponse, SearchResponse, issue_summary_from_search_issue, log_path,
        search_match_clause,
    };
    use reqwest::Url;
    use serde_json::json;

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

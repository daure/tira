use std::collections::BTreeMap;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use reqwest::Url;
use serde::Deserialize;

use crate::config::JiraCredentials;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueSummary {
    pub key: String,
    pub summary: String,
    pub status: String,
    pub issue_type: String,
    pub parent_key: Option<String>,
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
    pub log: CommandLogEntry,
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

pub fn load_project_issues(credentials: &JiraCredentials) -> JiraLoadResult {
    let site = credentials.site.trim().trim_end_matches('/');
    let jql = format!(
        "project = {} ORDER BY created DESC",
        credentials.default_project.trim()
    );
    let query = [
        ("jql", jql.as_str()),
        ("maxResults", "50"),
        ("fields", "*all"),
    ];
    let url = Url::parse_with_params(&format!("{site}/rest/api/3/search/jql"), &query)
        .expect("valid Jira URL");
    let method = "GET";
    let started_at = Instant::now();
    let timestamp = current_time_string();

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
                return JiraLoadResult {
                    issues: Err(JiraError(format!("Jira returned HTTP {status}"))),
                    log,
                };
            }

            let issues = response
                .json::<SearchResponse>()
                .map_err(|error| JiraError(format!("Jira response could not be read: {error}")))
                .map(|payload| {
                    payload
                        .issues
                        .into_iter()
                        .map(issue_summary_from_search_issue)
                        .collect()
                });

            JiraLoadResult { issues, log }
        }
        Err(error) => JiraLoadResult {
            issues: Err(JiraError(format!("Jira request failed: {error}"))),
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

pub fn load_issue_fields(credentials: &JiraCredentials) -> JiraFieldsResult {
    let site = credentials.site.trim().trim_end_matches('/');
    let url = Url::parse(&format!("{site}/rest/api/3/field")).expect("valid Jira URL");
    let method = "GET";
    let started_at = Instant::now();
    let timestamp = current_time_string();

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
    let url = Url::parse_with_params(&format!("{site}/rest/api/3/project/search"), &query)
        .expect("valid Jira URL");
    let method = "GET";
    let started_at = Instant::now();
    let timestamp = current_time_string();

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
struct SearchResponse {
    issues: Vec<SearchIssue>,
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
    use super::{SearchResponse, issue_summary_from_search_issue, log_path};
    use reqwest::Url;
    use serde_json::json;

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
}

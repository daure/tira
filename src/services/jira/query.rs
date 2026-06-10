use std::collections::{BTreeMap, HashSet};

use reqwest::Url;

use crate::domain::models::{CommandLogEntry, IssueSummary, JiraError};

use super::greenhopper::format_field_value;
use super::protocol::SearchIssue;

/// Issues per page when walking the root issue list.
pub const ROOT_PAGE_SIZE: u32 = 100;
/// Upper bound for a single `/search/jql` response (child probe / child fetch,
/// and the one-shot root-extent refetch on reload).
pub const CHILD_PAGE_SIZE: u32 = 5_000;

/// Maximum number of parent keys packed into a single `parent in (...)` query,
/// bounding the JQL/URL length. Larger parent sets are split across queries.
pub(super) const BATCH_PARENT_CHUNK: usize = 50;

/// The direct children of several parents fetched together, grouped by parent.
pub struct ChildrenBatch {
    /// One entry per requested parent, in the same order as the input keys.
    /// `Ok(children)` (possibly empty) on success; `Err` when that parent's
    /// query chunk failed, so callers can leave its stale subtree in place.
    pub groups: Vec<(String, Result<Vec<IssueSummary>, JiraError>)>,
    pub logs: Vec<CommandLogEntry>,
}

/// Buckets a flat list of children under their requested parents, in input
/// order. Parents present in `failed` map to that chunk's error so their stale
/// subtree survives; every other requested parent gets its children (or an
/// empty vec, which lets callers clear a subtree whose children all vanished).
/// Pure (no IO) so the grouping/empty/failure logic is unit-testable.
pub(super) fn group_children_for_parents(
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
pub(super) fn search_match_clause(term: &str) -> Option<String> {
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

pub(super) fn issue_summary_from_search_issue(issue: SearchIssue) -> IssueSummary {
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

pub(super) fn log_path(url: &Url) -> String {
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

#[cfg(test)]
mod tests {
    use super::{group_children_for_parents, issue_summary_from_search_issue, search_match_clause};
    use crate::domain::models::{IssueSummary, JiraError};
    use crate::services::jira::protocol::SearchResponse;
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

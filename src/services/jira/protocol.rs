use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::models::UserSummary;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AssignableUser {
    pub(super) account_id: String,
    pub(super) display_name: String,
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
pub(super) struct AssignIssuePayload<'a> {
    pub(super) account_id: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
pub(super) struct TransitionsResponse {
    pub(super) transitions: Vec<IssueTransition>,
}

#[derive(Debug, Deserialize)]
pub(super) struct IssueTransition {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) to: IssueTransitionStatus,
}

#[derive(Debug, Deserialize)]
pub(super) struct IssueTransitionStatus {
    pub(super) id: String,
    pub(super) name: String,
}

#[derive(Debug, Serialize)]
pub(super) struct TransitionIssuePayload<'a> {
    pub(super) transition: TransitionIssueId<'a>,
}

#[derive(Debug, Serialize)]
pub(super) struct TransitionIssueId<'a> {
    pub(super) id: &'a str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RankIssuePayload<'a> {
    pub(super) issues: Vec<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) rank_before_issue: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) rank_after_issue: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SearchResponse {
    pub(super) issues: Vec<SearchIssue>,
    #[serde(default)]
    pub(super) next_page_token: Option<String>,
    #[serde(default)]
    pub(super) is_last: bool,
}

/// Minimal response for the child-presence probe: only the `parent` field is
/// requested, so the issue's other fields (summary/status/type) are absent.
#[derive(Debug, Deserialize)]
pub(super) struct ParentProbeResponse {
    pub(super) issues: Vec<ParentProbeIssue>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ParentProbeIssue {
    pub(super) fields: ParentProbeFields,
}

#[derive(Debug, Deserialize)]
pub(super) struct ParentProbeFields {
    pub(super) parent: Option<IssueParent>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SearchIssue {
    pub(super) key: String,
    pub(super) fields: IssueFields,
}

#[derive(Debug, Deserialize)]
pub(super) struct IssueFields {
    pub(super) summary: String,
    pub(super) status: IssueStatus,
    #[serde(rename = "issuetype")]
    pub(super) issue_type: IssueType,
    pub(super) parent: Option<IssueParent>,
    #[serde(flatten)]
    pub(super) extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub(super) struct IssueStatus {
    pub(super) name: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct IssueType {
    pub(super) name: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct IssueParent {
    pub(super) key: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct FieldDetails {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) navigable: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct ProjectSearchResponse {
    pub(super) values: Vec<ProjectDetails>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ProjectDetails {
    pub(super) key: String,
    pub(super) name: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct BoardSearchResponse {
    pub(super) values: Vec<BoardDetails>,
}

#[derive(Debug, Deserialize)]
pub(super) struct BoardDetails {
    pub(super) id: u64,
    pub(super) name: String,
}

#[cfg(test)]
mod tests {
    use super::{ParentProbeResponse, SearchResponse};
    use serde_json::json;

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

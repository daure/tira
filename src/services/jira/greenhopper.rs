use std::collections::{BTreeMap, BTreeSet};

use crate::domain::models::{
    BoardColumnSummary, BoardData, BoardSwimlaneSummary, IssueSummary, JiraError, SprintSummary,
    TimelineEpic, TimelineEpicStats, TimelineIssue,
};

pub(super) fn board_data_from_greenhopper(
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

    let mut swimlanes = build_swimlanes(&payload, &issues);
    normalize_swimlane_issue_keys(&issues, &mut swimlanes);
    bucket_unassigned_issues(&issues, &mut swimlanes);

    Ok(BoardData {
        id: board_id,
        name: board_name,
        columns,
        swimlanes,
        issues,
        sprint: board_sprint_from_value(&payload),
    })
}

/// Builds the Timeline tab's epics from the greenhopper backlog payload
/// (`/xboard/plan/backlog/data.json`): every epic with its rolled-up child
/// counts and its child issues with the sprints they sit in. Sprint *dates*
/// come separately from the agile sprint endpoint and are joined in the
/// service, so this only reads the epic/issue structure. A missing `epicData`
/// section degrades to an empty list.
pub(super) fn timeline_epics_from_greenhopper(payload: &serde_json::Value) -> Vec<TimelineEpic> {
    let children_by_epic = timeline_children_by_epic(payload);
    let issues_by_key = timeline_issues_by_key(payload);
    payload
        .pointer("/epicData/epics")
        .and_then(serde_json::Value::as_array)
        .map(|epics| {
            epics
                .iter()
                .map(|epic| timeline_epic_from_value(epic, &children_by_epic, &issues_by_key))
                .collect()
        })
        .unwrap_or_default()
}

fn timeline_issues_by_key<'a>(
    payload: &'a serde_json::Value,
) -> BTreeMap<String, &'a serde_json::Value> {
    payload
        .get("issues")
        .and_then(serde_json::Value::as_array)
        .map(|issues| {
            issues
                .iter()
                .filter_map(|issue| {
                    text_property(issue, &["key", "issueKey"]).map(|key| (key, issue))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Groups the backlog issues under their epic (Jira's modern `parentKey`). Each
/// issue keeps the sprint ids it belongs to so the timeline can bucket it into
/// columns.
fn timeline_children_by_epic(payload: &serde_json::Value) -> BTreeMap<String, Vec<TimelineIssue>> {
    let mut grouped: BTreeMap<String, Vec<TimelineIssue>> = BTreeMap::new();
    let Some(issues) = payload.get("issues").and_then(serde_json::Value::as_array) else {
        return grouped;
    };
    for issue in issues {
        let Some(parent) = text_property(issue, &["parentKey"]) else {
            continue;
        };
        let Some(key) = text_property(issue, &["key", "issueKey"]) else {
            continue;
        };
        grouped.entry(parent).or_default().push(TimelineIssue {
            key,
            summary: text_property(issue, &["summary"]).unwrap_or_default(),
            status: text_property(issue, &["statusName", "status"]).unwrap_or_default(),
            issue_type: text_property(issue, &["typeName", "issueType"])
                .unwrap_or_else(|| String::from("Issue")),
            done: issue.get("done").and_then(serde_json::Value::as_bool) == Some(true),
            start_day: timeline_start_day(issue),
            end_day: timeline_end_day(issue),
            sprint_ids: timeline_sprint_ids(issue),
        });
    }
    grouped
}

fn timeline_sprint_ids(issue: &serde_json::Value) -> Vec<i64> {
    issue
        .get("sprintIds")
        .and_then(serde_json::Value::as_array)
        .map(|ids| ids.iter().filter_map(serde_json::Value::as_i64).collect())
        .unwrap_or_default()
}

fn timeline_start_day(issue: &serde_json::Value) -> Option<i64> {
    timeline_day(
        issue,
        &[
            "isoStartDate",
            "startDate",
            "start",
            "plannedStart",
            "plannedStartDate",
            "targetStart",
            "targetStartDate",
        ],
    )
}

fn timeline_end_day(issue: &serde_json::Value) -> Option<i64> {
    timeline_day(
        issue,
        &[
            "isoDueDate",
            "isoEndDate",
            "endDate",
            "dueDate",
            "duedate",
            "end",
            "plannedEnd",
            "plannedEndDate",
            "targetEnd",
            "targetEndDate",
        ],
    )
}

fn timeline_day(issue: &serde_json::Value, keys: &[&str]) -> Option<i64> {
    text_property(issue, keys).and_then(|date| crate::domain::date::iso_to_days(&date))
}

fn timeline_epic_from_value(
    epic: &serde_json::Value,
    children_by_epic: &BTreeMap<String, Vec<TimelineIssue>>,
    issues_by_key: &BTreeMap<String, &serde_json::Value>,
) -> TimelineEpic {
    let key = text_property(epic, &["key", "epicKey"]).unwrap_or_default();
    let issue = issues_by_key.get(&key).copied();
    let counts = epic.pointer("/epicStats/childrenIssueCount");
    let count = |field: &str| {
        counts
            .and_then(|counts| counts.get(field))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as u32
    };
    let stats = TimelineEpicStats {
        to_do: count("toDo"),
        in_progress: count("inProgress"),
        done: count("done"),
    };
    let done = epic.get("done").and_then(serde_json::Value::as_bool) == Some(true);
    let children = children_by_epic.get(&key).cloned().unwrap_or_default();

    TimelineEpic {
        status: timeline_epic_status(done, &stats),
        summary: text_property(epic, &["summary", "epicLabel"]).unwrap_or_else(|| key.clone()),
        key,
        done,
        start_day: timeline_start_day(epic).or_else(|| issue.and_then(timeline_start_day)),
        end_day: timeline_end_day(epic).or_else(|| issue.and_then(timeline_end_day)),
        sprint_ids: timeline_sprint_ids(epic)
            .into_iter()
            .chain(issue.map(timeline_sprint_ids).unwrap_or_default())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        stats,
        children,
    }
}

/// Greenhopper does not give an epic a status name in the backlog payload, only
/// a `done` flag and child counts, so derive a short status from those for the
/// timeline label.
fn timeline_epic_status(done: bool, stats: &TimelineEpicStats) -> String {
    if done {
        String::from("Done")
    } else if stats.in_progress > 0 {
        String::from("In Progress")
    } else {
        String::from("To Do")
    }
}

/// Builds the board's swimlanes from the greenhopper payload, sorted by
/// position. A swimlane that lists no issues of its own inherits the issues
/// whose `swimlane_id` matches its id.
fn build_swimlanes(
    payload: &serde_json::Value,
    issues: &[IssueSummary],
) -> Vec<BoardSwimlaneSummary> {
    let swimlane_values = payload
        .pointer("/swimlanesData/swimlanes")
        .or_else(|| payload.pointer("/swimlaneData/swimlanes"))
        .or_else(|| payload.get("swimlanes"))
        .and_then(serde_json::Value::as_array);
    swimlane_values
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
        .unwrap_or_default()
}

/// Ensures every board issue lands in a swimlane: with no usable swimlanes all
/// issues go into a single "Issues" lane, otherwise issues not claimed by any
/// lane are gathered into a trailing "Other issues" lane.
fn bucket_unassigned_issues(issues: &[IssueSummary], swimlanes: &mut Vec<BoardSwimlaneSummary>) {
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
        let mut assigned = BTreeSet::new();
        for swimlane in swimlanes.iter() {
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
        .collect::<BTreeSet<_>>();
    let id_to_key = issues
        .iter()
        .filter_map(|issue| {
            issue
                .field_values
                .get("id")
                .map(|id| (id.as_str(), issue.key.as_str()))
        })
        .collect::<BTreeMap<_, _>>();

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

fn board_visible_issue_ids(payload: &serde_json::Value) -> Option<BTreeSet<String>> {
    let mut ids = BTreeSet::new();
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

fn collect_issue_ids_from_array(value: Option<&serde_json::Value>, ids: &mut BTreeSet<String>) {
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
    visible_ids: Option<&BTreeSet<String>>,
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
    fn probe<'a>(
        fields: &'a serde_json::Value,
        value: &'a serde_json::Value,
        key: &str,
    ) -> Option<&'a serde_json::Value> {
        fields.get(key).or_else(|| value.get(key))
    }

    let fields = value.get("fields").unwrap_or(value);
    let status = probe(fields, value, "status");
    let issue_type = fields
        .get("issuetype")
        .or_else(|| probe(fields, value, "issueType"));
    let parent = probe(fields, value, "parent");
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

pub(super) fn text_property(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(format_field_value))
}

pub(super) fn format_field_value(value: &serde_json::Value) -> Option<String> {
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
    use super::board_data_from_greenhopper;
    use serde_json::json;

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

    #[test]
    fn timeline_epics_bucket_children_under_epics_with_stats() {
        let epics = super::timeline_epics_from_greenhopper(&json!({
            "epicData": {
                "epics": [
                    {
                        "key": "DPP-10",
                        "summary": "Checkout revamp",
                        "done": false,
                        "isoStartDate": "2026-06-10",
                        "isoDueDate": "2026-06-24",
                        "epicStats": { "childrenIssueCount": { "toDo": 1, "inProgress": 1, "done": 2 } }
                    },
                    {
                        "key": "DPP-20",
                        "summary": "Done epic",
                        "done": true,
                        "epicStats": { "childrenIssueCount": { "toDo": 0, "inProgress": 0, "done": 3 } }
                    }
                ]
            },
            "issues": [
                { "key": "DPP-10", "summary": "Checkout revamp", "statusName": "In Progress", "typeName": "Epic", "done": false, "sprintIds": [2] },
                { "key": "DPP-11", "summary": "Cart", "parentKey": "DPP-10", "statusName": "In Progress", "typeName": "Story", "done": false, "startDate": "2026-06-03", "dueDate": "2026-06-17", "sprintIds": [2] },
                { "key": "DPP-12", "summary": "Pay", "parentKey": "DPP-10", "statusName": "Done", "typeName": "Task", "done": true, "sprintIds": [1, 2] },
                { "key": "DPP-99", "summary": "Orphan", "statusName": "To Do", "typeName": "Task", "done": false, "sprintIds": [3] }
            ]
        }));

        assert_eq!(epics.len(), 2);
        let revamp = &epics[0];
        assert_eq!(revamp.key, "DPP-10");
        assert_eq!(revamp.status, "In Progress");
        assert_eq!(revamp.stats.total(), 4);
        assert_eq!(revamp.stats.percent_done(), 50);
        assert_eq!(
            revamp.start_day,
            crate::domain::date::iso_to_days("2026-06-10")
        );
        assert_eq!(
            revamp.end_day,
            crate::domain::date::iso_to_days("2026-06-24")
        );
        assert_eq!(revamp.sprint_ids, vec![2]);
        // Both children land under the epic; the orphan (no parentKey) does not.
        assert_eq!(revamp.children.len(), 2);
        assert_eq!(
            revamp.children[0].start_day,
            crate::domain::date::iso_to_days("2026-06-03")
        );
        assert_eq!(
            revamp.children[0].end_day,
            crate::domain::date::iso_to_days("2026-06-17")
        );
        assert_eq!(revamp.sprint_ids().into_iter().collect::<Vec<_>>(), vec![2]);
        assert_eq!(epics[1].status, "Done");
    }
}

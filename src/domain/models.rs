use std::collections::BTreeMap;

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

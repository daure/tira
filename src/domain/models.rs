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

/// The data behind the Timeline tab: the project's epics, the board's sprints,
/// and each epic's child issues bucketed by the sprint(s) they sit in. Built
/// from one greenhopper backlog payload. Because the DICE project sets no
/// start/due dates on epics, the timeline is sprint-scheduled (Jira's own
/// fallback) rather than a date-axis Gantt.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TimelineData {
    pub epics: Vec<TimelineEpic>,
    pub sprints: Vec<TimelineSprint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineEpic {
    pub key: String,
    pub summary: String,
    pub status: String,
    pub done: bool,
    pub stats: TimelineEpicStats,
    pub children: Vec<TimelineIssue>,
}

impl TimelineEpic {
    /// The distinct sprint ids this epic touches through its children. Drives
    /// the columns the epic's bar spans.
    pub fn sprint_ids(&self) -> std::collections::BTreeSet<i64> {
        self.children
            .iter()
            .flat_map(|child| child.sprint_ids.iter().copied())
            .collect()
    }
}

/// Child-issue counts rolled up for an epic's progress bar, taken straight from
/// greenhopper's `childrenIssueCount`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimelineEpicStats {
    pub to_do: u32,
    pub in_progress: u32,
    pub done: u32,
}

impl TimelineEpicStats {
    pub fn total(&self) -> u32 {
        self.to_do + self.in_progress + self.done
    }

    /// Percentage of child issues that are done; 0 when the epic has no
    /// children, so the bar stays empty instead of dividing by zero.
    pub fn percent_done(&self) -> u8 {
        let total = self.total();
        if total == 0 {
            return 0;
        }
        ((u64::from(self.done) * 100) / u64::from(total)) as u8
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineIssue {
    pub key: String,
    pub summary: String,
    pub status: String,
    pub issue_type: String,
    pub done: bool,
    pub sprint_ids: Vec<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SprintState {
    Active,
    Closed,
    Future,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineSprint {
    pub id: i64,
    pub name: String,
    pub state: SprintState,
    /// Sprint start as days since 1970-01-01, when the sprint is dated. Undated
    /// buckets (board backlog pseudo-sprints) are `None` and are left off the
    /// timeline axis entirely.
    pub start_day: Option<i64>,
    pub end_day: Option<i64>,
}

impl TimelineSprint {
    /// Compact label for a sprint pill on the timeline axis.
    pub fn short_label(&self) -> String {
        sprint_short_label(&self.name)
    }
}

/// Abbreviates a sprint name to a compact pill label:
/// - a name containing digits becomes `S` + its last digit run
///   (`DICE Sprint 190` -> `S190`), since the sprint number trails the name;
/// - otherwise a name of three or more words becomes the uppercased initial of
///   each word (`ready to pick up` -> `RTPU`);
/// - otherwise (one or two words, no number) the first three characters,
///   uppercased (`CDS backlog` -> `CDS`, `Archive` -> `ARC`).
pub fn sprint_short_label(name: &str) -> String {
    let name = name.trim();
    if let Some(number) = last_digit_run(name) {
        return format!("S{number}");
    }
    let words: Vec<&str> = name.split_whitespace().collect();
    if words.len() >= 3 {
        words
            .iter()
            .filter_map(|word| word.chars().next())
            .flat_map(char::to_uppercase)
            .collect()
    } else {
        name.chars()
            .take(3)
            .flat_map(char::to_uppercase)
            .collect()
    }
}

/// The last contiguous run of ASCII digits in `text`, if any.
fn last_digit_run(text: &str) -> Option<String> {
    let mut runs: Vec<String> = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
        } else if !current.is_empty() {
            runs.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        runs.push(current);
    }
    runs.pop()
}

#[cfg(test)]
mod tests {
    use super::sprint_short_label;

    #[test]
    fn numbered_sprint_uses_s_prefix_with_trailing_number() {
        assert_eq!(sprint_short_label("DICE Sprint 190"), "S190");
        assert_eq!(sprint_short_label("Sprint 5"), "S5");
        // The sprint number trails the name, so a leading number is ignored.
        assert_eq!(sprint_short_label("2026 Sprint 5"), "S5");
    }

    #[test]
    fn three_or_more_words_use_uppercased_initials() {
        assert_eq!(sprint_short_label("ready to pick up"), "RTPU");
        assert_eq!(sprint_short_label("get it done now"), "GIDN");
    }

    #[test]
    fn one_or_two_unnumbered_words_use_first_three_chars() {
        assert_eq!(sprint_short_label("CDS backlog"), "CDS");
        assert_eq!(sprint_short_label("Archive"), "ARC");
        assert_eq!(sprint_short_label("QA"), "QA");
    }
}

use crate::{app::board_issue_matches_search, services::jira::IssueSummary};

/// Whether a board card matches the search query. Delegates to the canonical
/// matcher in `app::board` so the cards rendered here always agree with the
/// cards navigation can select (no divergence between what you see and what
/// `j`/`k` can land on).
pub(super) fn board_issue_matches_filter(issue: &IssueSummary, search: &str) -> bool {
    board_issue_matches_search(issue, search)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn issue(key: &str, summary: &str) -> IssueSummary {
        IssueSummary {
            key: key.to_owned(),
            summary: summary.to_owned(),
            status: "To Do".to_owned(),
            issue_type: "Task".to_owned(),
            parent_key: None,
            has_children: false,
            field_values: BTreeMap::new(),
        }
    }

    #[test]
    fn filter_matches_non_contiguous_subsequence() {
        let issue = issue("KAN-1", "Improve navigation flow");
        // "imnav" is not a substring but is a fuzzy subsequence of the summary.
        assert!(board_issue_matches_filter(&issue, "imnav"));
        // Characters out of order never match.
        assert!(!board_issue_matches_filter(&issue, "vanimprove"));
    }

    #[test]
    fn filter_matches_assignee_initials() {
        let mut issue = issue("KAN-2", "Unrelated summary");
        issue
            .field_values
            .insert("assignee".to_owned(), "Marlo Vlietstra".to_owned());
        // The avatar shows "MV"; searching the initials still matches.
        assert!(board_issue_matches_filter(&issue, "mv"));
        // The avatar bubble shows "@MV"; the visible "@" prefix matches too.
        assert!(board_issue_matches_filter(&issue, "@mv"));
        assert!(board_issue_matches_filter(&issue, "@MV"));
        // The full name fuzzy-matches too.
        assert!(board_issue_matches_filter(&issue, "marlo"));
    }

    #[test]
    fn empty_filter_matches_everything() {
        let issue = issue("KAN-3", "anything");
        assert!(board_issue_matches_filter(&issue, ""));
        assert!(board_issue_matches_filter(&issue, "   "));
    }
}

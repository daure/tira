use std::collections::BTreeMap;

use super::{App, ApplicationTab, ModalState, TicketDialogAction};
use crate::{
    components::generic::tree::TreeItem,
    services::jira::{IssueSummary, TimelineEpic, TimelineIssue},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TicketDialogTab {
    Overview,
    Properties,
    Subtasks,
    LinkedWorkItems,
}

impl TicketDialogTab {
    pub const ALL: [Self; 4] = [
        Self::Overview,
        Self::Properties,
        Self::Subtasks,
        Self::LinkedWorkItems,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Properties => "Properties",
            Self::Subtasks => "Subtasks",
            Self::LinkedWorkItems => "Linked work items",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TicketDialogState {
    pub ticket: TicketDialogIssue,
    pub selected_tab: TicketDialogTab,
}

impl TicketDialogState {
    fn new(ticket: TicketDialogIssue) -> Self {
        Self {
            ticket,
            selected_tab: TicketDialogTab::Overview,
        }
    }

    pub fn dispatch(&mut self, action: TicketDialogAction) {
        let current = TicketDialogTab::ALL
            .iter()
            .position(|tab| *tab == self.selected_tab)
            .unwrap_or_default();
        let next = match action {
            TicketDialogAction::PreviousTab => current
                .checked_sub(1)
                .unwrap_or(TicketDialogTab::ALL.len() - 1),
            TicketDialogAction::NextTab => (current + 1) % TicketDialogTab::ALL.len(),
        };
        self.selected_tab = TicketDialogTab::ALL[next];
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TicketDialogIssue {
    pub key: String,
    pub summary: String,
    pub status: String,
    pub issue_type: String,
    pub parent_key: Option<String>,
    pub parent_issue_type: Option<String>,
    pub fields: BTreeMap<String, String>,
}

impl App {
    pub fn is_ticket_dialog_open(&self) -> bool {
        matches!(self.modal, Some(ModalState::Ticket(_)))
    }

    pub fn ticket_dialog(&self) -> Option<&TicketDialogState> {
        match &self.modal {
            Some(ModalState::Ticket(state)) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn open_selected_ticket_dialog(&mut self) {
        let Some(ticket) = self.selected_ticket_dialog_issue() else {
            self.status = String::from("No ticket selected.");
            return;
        };
        self.close_dropdowns();
        self.modal = Some(ModalState::Ticket(TicketDialogState::new(ticket)));
    }

    pub(crate) fn dispatch_ticket_dialog(&mut self, action: TicketDialogAction) {
        let Some(ModalState::Ticket(state)) = &mut self.modal else {
            return;
        };
        state.dispatch(action);
    }

    fn selected_ticket_dialog_issue(&self) -> Option<TicketDialogIssue> {
        match self.active_tab() {
            ApplicationTab::List => self.selected_list_ticket(),
            ApplicationTab::Board => self.selected_board_ticket(),
            ApplicationTab::Timeline => self.selected_timeline_ticket(),
        }
    }

    fn selected_list_ticket(&self) -> Option<TicketDialogIssue> {
        let key = self.filtered_tree.selected_item_id()?;
        let item = self
            .filtered_tree
            .items()
            .iter()
            .find(|item| item.id == key)?;
        Some(ticket_from_tree_item(item, self.filtered_tree.items()))
    }

    fn selected_board_ticket(&self) -> Option<TicketDialogIssue> {
        let key = self.board.selected_issue_key()?;
        let issues = &self.board.data()?.issues;
        let issue = issues.iter().find(|issue| issue.key == key)?;
        Some(ticket_from_issue_summary(issue, issues))
    }

    fn selected_timeline_ticket(&self) -> Option<TicketDialogIssue> {
        let key = self.timeline.tree().selected_item_id()?;
        let data = self.timeline.data()?;
        for epic in &data.epics {
            if epic.key == key {
                return Some(ticket_from_timeline_epic(epic));
            }
            if let Some(child) = epic.children.iter().find(|child| child.key == key) {
                return Some(ticket_from_timeline_issue(child, epic));
            }
        }
        None
    }
}

fn ticket_from_tree_item(item: &TreeItem, items: &[TreeItem]) -> TicketDialogIssue {
    let parent_issue_type = item.parent_id.as_ref().and_then(|parent_key| {
        items
            .iter()
            .find(|parent| parent.id == *parent_key)
            .map(|parent| parent.kind.clone())
    });
    TicketDialogIssue {
        key: item.id.clone(),
        summary: item.label.clone(),
        status: item.status.clone(),
        issue_type: item.kind.clone(),
        parent_key: item.parent_id.clone(),
        parent_issue_type,
        fields: item.field_values.clone(),
    }
}

fn ticket_from_issue_summary(issue: &IssueSummary, issues: &[IssueSummary]) -> TicketDialogIssue {
    let parent_issue_type = issue.parent_key.as_ref().and_then(|parent_key| {
        issues
            .iter()
            .find(|parent| parent.key == *parent_key)
            .map(|parent| parent.issue_type.clone())
    });
    TicketDialogIssue {
        key: issue.key.clone(),
        summary: issue.summary.clone(),
        status: issue.status.clone(),
        issue_type: issue.issue_type.clone(),
        parent_key: issue.parent_key.clone(),
        parent_issue_type,
        fields: issue.field_values.clone(),
    }
}

fn ticket_from_timeline_epic(epic: &TimelineEpic) -> TicketDialogIssue {
    let mut fields = BTreeMap::new();
    fields.insert(
        String::from("Progress"),
        format!("{}% done", epic.stats.percent_done()),
    );
    fields.insert(String::from("Subtasks"), epic.children.len().to_string());
    TicketDialogIssue {
        key: epic.key.clone(),
        summary: epic.summary.clone(),
        status: epic.status.clone(),
        issue_type: String::from("Epic"),
        parent_key: None,
        parent_issue_type: None,
        fields,
    }
}

fn ticket_from_timeline_issue(issue: &TimelineIssue, epic: &TimelineEpic) -> TicketDialogIssue {
    let mut fields = BTreeMap::new();
    fields.insert(
        String::from("Sprint count"),
        issue.sprint_ids.len().to_string(),
    );
    TicketDialogIssue {
        key: issue.key.clone(),
        summary: issue.summary.clone(),
        status: issue.status.clone(),
        issue_type: issue.issue_type.clone(),
        parent_key: Some(epic.key.clone()),
        parent_issue_type: Some(String::from("Epic")),
        fields,
    }
}

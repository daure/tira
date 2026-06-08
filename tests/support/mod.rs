#![allow(dead_code)]

use std::time::{SystemTime, UNIX_EPOCH};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use tira::services::jira::{IssueSummary, ProjectSummary};

pub fn rendered_text(terminal: &Terminal<TestBackend>) -> (String, String) {
    let buffer = terminal.backend().buffer();
    let screen = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    let bottom_row = buffer
        .content()
        .chunks(buffer.area().width as usize)
        .last()
        .expect("bottom row")
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    (screen, bottom_row)
}

pub fn test_issues(count: usize) -> Vec<IssueSummary> {
    (0..count)
        .map(|index| {
            issue(
                &format!("KAN-{}", index + 1),
                &format!("Issue {}", index + 1),
                "Task",
                None,
            )
        })
        .collect()
}

pub fn issue(key: &str, summary: &str, issue_type: &str, parent_key: Option<&str>) -> IssueSummary {
    IssueSummary {
        key: key.to_owned(),
        summary: summary.to_owned(),
        status: String::from("To Do"),
        issue_type: issue_type.to_owned(),
        parent_key: parent_key.map(str::to_owned),
        has_children: false,
        field_values: std::collections::BTreeMap::new(),
    }
}

pub fn project(key: &str, name: &str) -> ProjectSummary {
    ProjectSummary {
        key: key.to_owned(),
        name: name.to_owned(),
    }
}

pub fn temp_config_path() -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("tira-test-{suffix}/config.toml"))
}

pub fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

pub fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

pub fn shift(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::SHIFT)
}

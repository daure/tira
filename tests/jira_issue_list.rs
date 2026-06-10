mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tira::{App, KeyBindings};

use support::{issue, key};

#[test]
fn column_dropdown_opens_with_c_and_ctrl_space_hides_the_selected_column() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(vec![
        issue("KAN-1", "Catalog epic", "Epic", None),
        issue("KAN-2", "Catalog story", "Story", Some("KAN-1")),
    ]);

    app.handle_key(key('c'), &bindings);
    assert!(app.is_column_dropdown_open());
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &bindings);
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &bindings);
    assert!(!app.is_column_dropdown_open());

    app.handle_key(key('c'), &bindings);
    app.handle_key(
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL),
        &bindings,
    );
    app.handle_key(
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL),
        &bindings,
    );
    app.handle_key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL),
        &bindings,
    );
    assert!(app.is_column_dropdown_open());
    assert!(
        !app.visible_issue_columns()
            .contains(&tira::JiraIssueColumn::Field {
                id: String::from("labels"),
                label: String::from("Labels"),
            })
    );
}

#[test]
fn yanking_without_a_url_shows_a_notification_that_later_expires() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(vec![issue("KAN-1", "Catalog epic", "Epic", None)]);

    app.handle_key(key('y'), &bindings);
    assert!(app.notifications().is_empty());
    app.handle_key(key('y'), &bindings);
    assert_eq!(app.notifications()[0].title(), "Issue URL not copied");

    app.tick(std::time::Duration::from_secs(3));
    assert!(app.notifications().is_empty());
}

#[test]
fn collapse_all_keys_collapse_the_expanded_tree_to_its_roots() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(vec![
        issue("KAN-1", "Catalog epic", "Epic", None),
        issue("KAN-2", "Catalog story", "Story", Some("KAN-1")),
    ]);

    // Expand the selected epic to reveal its child.
    app.handle_key(key('l'), &bindings);
    assert_eq!(app.visible_issue_rows().len(), 2);

    // Ctrl-c collapses all loaded nodes.
    app.handle_key(
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        &bindings,
    );
    assert_eq!(app.visible_issue_rows().len(), 1);

    // Space re-expands the selected node.
    app.handle_key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        &bindings,
    );
    assert_eq!(app.visible_issue_rows().len(), 2);

    // `z` collapses all loaded nodes.
    app.handle_key(key('z'), &bindings);
    assert_eq!(app.visible_issue_rows().len(), 1);
}

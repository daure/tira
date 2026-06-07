mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tira::{App, KeyBindings};

use support::{issue, key, shift};

#[test]
fn column_picker_yank_and_expansion_keys_apply_to_issue_list() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(vec![
        issue("KAN-1", "Catalog epic", "Epic", None),
        issue("KAN-2", "Catalog story", "Story", Some("KAN-1")),
    ]);

    assert_eq!(app.visible_issue_rows().len(), 1);

    app.handle_key(key('c'), &bindings);
    assert!(app.is_column_dropdown_open());
    assert!(app.notifications().is_empty());
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &bindings);
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &bindings);
    assert!(!app.is_column_dropdown_open());
    app.handle_key(key('c'), &bindings);
    assert!(app.is_column_dropdown_open());
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &bindings);
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &bindings);
    assert!(!app.is_column_dropdown_open());

    app.handle_key(key('y'), &bindings);
    assert!(app.notifications().is_empty());
    app.handle_key(key('y'), &bindings);
    assert_eq!(app.notifications()[0].title(), "Issue URL not copied");
    app.tick(std::time::Duration::from_secs(3));
    assert!(app.notifications().is_empty());
    app.handle_key(key('c'), &bindings);
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
            .contains(&tira::JiraIssueColumn::Summary)
    );
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &bindings);
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &bindings);

    app.handle_key(shift('z'), &bindings);
    assert_eq!(app.visible_issue_rows().len(), 2);

    app.handle_key(
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        &bindings,
    );
    assert_eq!(app.visible_issue_rows().len(), 1);

    app.handle_key(shift('z'), &bindings);
    assert_eq!(app.visible_issue_rows().len(), 2);

    app.handle_key(key('z'), &bindings);
    assert_eq!(app.visible_issue_rows().len(), 1);
}

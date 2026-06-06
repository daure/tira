mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tira::{App, KeyBindings};

use support::{ctrl, issue, key, test_issues};

#[test]
fn list_tree_orders_orphans_before_epics_and_children_expand_with_space() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(vec![
        issue("KAN-1", "Unparented task", "Task", None),
        issue("KAN-2", "Catalog epic", "Epic", None),
        issue("KAN-3", "Catalog story", "Story", Some("KAN-2")),
    ]);

    let rows = app.visible_issue_rows();
    assert_eq!(app.issues()[rows[0].item_index].id, "KAN-1");
    assert_eq!(app.issues()[rows[1].item_index].id, "KAN-2");
    assert_eq!(rows.len(), 2);

    app.handle_key(key('j'), &bindings);
    app.handle_key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        &bindings,
    );

    let rows = app.visible_issue_rows();
    assert_eq!(app.issues()[rows[2].item_index].id, "KAN-3");
    assert_eq!(rows[2].depth, 1);
}

#[test]
fn slash_focuses_filter_and_filters_matching_issues() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(vec![
        issue("KAN-1", "Checkout work", "Task", None),
        issue("KAN-2", "Catalog work", "Task", None),
    ]);

    app.handle_key(key('/'), &bindings);
    app.handle_key(key('c'), &bindings);
    app.handle_key(key('a'), &bindings);
    app.handle_key(key('t'), &bindings);

    let rows = app.visible_issue_rows();
    assert!(app.is_filter_focused());
    assert_eq!(app.filter(), "cat");
    assert_eq!(rows.len(), 1);
    assert_eq!(app.issues()[rows[0].item_index].id, "KAN-2");
}

#[test]
fn filter_enter_blurs_and_keeps_current_clamped_selection() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(vec![
        issue("KAN-1", "Checkout work", "Task", None),
        issue("KAN-2", "Catalog work", "Task", None),
        issue("KAN-3", "Cart polish", "Task", None),
    ]);

    app.handle_key(key('j'), &bindings);
    app.handle_key(key('/'), &bindings);
    for ch in "catalog".chars() {
        app.handle_key(key(ch), &bindings);
    }
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &bindings);

    assert!(!app.is_filter_focused());
    assert_eq!(app.filter(), "catalog");
    assert_eq!(app.selected_issue_index(), 0);
    let rows = app.visible_issue_rows();
    assert_eq!(app.issues()[rows[0].item_index].id, "KAN-2");
}

#[test]
fn ctrl_slash_inside_filter_only_exits_focus() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(vec![
        issue("KAN-1", "Checkout work", "Task", None),
        issue("KAN-2", "Catalog work", "Task", None),
    ]);

    app.handle_key(key('/'), &bindings);
    app.handle_key(key('c'), &bindings);
    app.handle_key(key('a'), &bindings);
    app.handle_key(key('t'), &bindings);
    app.handle_key(
        KeyEvent::new(KeyCode::Char('/'), KeyModifiers::CONTROL),
        &bindings,
    );

    assert!(!app.is_filter_focused());
    assert_eq!(app.filter(), "cat");
    assert_eq!(app.visible_issue_rows().len(), 1);
    assert_eq!(app.selected_issue_index(), 0);
}

#[test]
fn only_escape_or_ctrl_left_bracket_clear_filter_in_tree() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(vec![
        issue("KAN-1", "Checkout work", "Task", None),
        issue("KAN-2", "Catalog work", "Task", None),
        issue("KAN-3", "Cart polish", "Task", None),
    ]);

    app.handle_key(key('/'), &bindings);
    app.handle_key(key('c'), &bindings);
    app.handle_key(key('a'), &bindings);
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &bindings);
    assert_eq!(app.filter(), "ca");

    app.handle_key(key('j'), &bindings);
    assert_eq!(app.selected_issue_index(), 1);

    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &bindings);
    assert_eq!(app.filter(), "ca");
    assert_eq!(app.visible_issue_rows().len(), 2);

    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &bindings);
    assert_eq!(app.filter(), "");
    assert_eq!(app.visible_issue_rows().len(), 3);
    assert_eq!(app.selected_issue_index(), 1);

    app.handle_key(key('/'), &bindings);
    app.handle_key(key('c'), &bindings);
    app.handle_key(key('a'), &bindings);
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &bindings);
    app.handle_key(
        KeyEvent::new(KeyCode::Char('['), KeyModifiers::CONTROL),
        &bindings,
    );

    assert_eq!(app.filter(), "");
    assert_eq!(app.visible_issue_rows().len(), 3);
    assert_eq!(app.selected_issue_index(), 1);
}

#[test]
fn tree_supports_h_l_g_and_gg_navigation() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(vec![
        issue("KAN-1", "Catalog epic", "Epic", None),
        issue("KAN-2", "Catalog story", "Story", Some("KAN-1")),
        issue("KAN-3", "Checkout task", "Task", None),
    ]);

    app.handle_key(key('l'), &bindings);
    assert_eq!(app.visible_issue_rows().len(), 3);
    assert_eq!(app.selected_issue_index(), 0);

    app.handle_key(key('l'), &bindings);
    assert_eq!(app.selected_issue_index(), 1);

    app.handle_key(key('h'), &bindings);
    assert_eq!(app.selected_issue_index(), 0);
    app.handle_key(key('G'), &bindings);
    assert_eq!(app.selected_issue_index(), 2);

    app.handle_key(
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::SHIFT),
        &bindings,
    );
    assert_eq!(app.selected_issue_index(), 2);

    app.handle_key(key('g'), &bindings);
    app.handle_key(key('g'), &bindings);
    assert_eq!(app.selected_issue_index(), 0);
}

#[test]
fn list_navigation_moves_selection_and_half_pages() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(test_issues(20));

    app.handle_key(key('j'), &bindings);
    assert_eq!(app.selected_issue_index(), 1);

    app.handle_key(ctrl('d'), &bindings);
    assert_eq!(app.selected_issue_index(), 11);

    app.handle_key(ctrl('u'), &bindings);
    assert_eq!(app.selected_issue_index(), 1);

    app.handle_key(key('k'), &bindings);
    assert_eq!(app.selected_issue_index(), 0);
}

#[test]
fn ctrl_q_quits_app_always_but_q_does_not_when_typing() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(Vec::new());

    app.handle_key(key('/'), &bindings);
    assert!(app.is_filter_focused());

    app.handle_key(key('q'), &bindings);
    assert!(app.is_filter_focused());
    assert_eq!(app.filter(), "q");
    assert!(app.is_running());

    app.handle_key(ctrl('q'), &bindings);
    assert!(!app.is_running());
}

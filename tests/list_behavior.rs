mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use tira::{App, KeyBindings};

use support::{ctrl, issue, key, project, test_issues};

fn left_click(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}
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
fn mouse_click_on_list_filter_focuses_search_input() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(test_issues(3));

    app.handle_mouse(left_click(2, 1), Rect::new(0, 0, 100, 20), &bindings);

    assert!(app.is_filter_focused());
}

#[test]
fn mouse_click_on_column_trigger_opens_column_dropdown() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(test_issues(3));

    app.handle_mouse(left_click(90, 1), Rect::new(0, 0, 100, 20), &bindings);

    assert!(app.is_column_dropdown_open());
}

#[test]
fn mouse_click_on_tree_chevron_toggles_expansion() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(vec![
        issue("KAN-1", "Catalog epic", "Epic", None),
        issue("KAN-2", "Catalog story", "Story", Some("KAN-1")),
    ]);

    assert_eq!(app.visible_issue_rows().len(), 1);
    app.handle_mouse(left_click(1, 3), Rect::new(0, 0, 100, 20), &bindings);

    assert_eq!(app.visible_issue_rows().len(), 2);
}

#[test]
fn mouse_click_on_status_project_opens_project_picker() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues_and_projects(
        vec![issue("KAN-1", "Catalog epic", "Epic", None)],
        vec![project("KAN", "Kanban"), project("OPS", "Operations")],
        "KAN",
    );

    app.handle_mouse(left_click(92, 19), Rect::new(0, 0, 100, 20), &bindings);

    assert!(app.is_project_dropdown_open());
}

#[test]
fn mouse_click_on_column_search_focuses_filter_without_toggling_selection() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(test_issues(3));

    app.handle_key(key('c'), &bindings);
    let selected_before = app
        .column_dropdown()
        .expect("column dropdown")
        .options()
        .iter()
        .filter(|option| option.selected)
        .count();
    app.handle_mouse(left_click(82, 3), Rect::new(0, 0, 100, 20), &bindings);

    let dropdown = app.column_dropdown().expect("column dropdown");
    let selected_after = dropdown
        .options()
        .iter()
        .filter(|option| option.selected)
        .count();
    assert!(app.is_column_dropdown_filter_focused());
    assert_eq!(selected_after, selected_before);
}

#[test]
fn focused_issue_filter_keeps_non_help_printable_global_bindings_as_text() {
    let bindings = KeyBindings::from_toml_str(
        r##"
        [global]
        reload_list = "R"
        "##,
    );
    let mut app = App::with_issues(vec![issue("KAN-1", "Catalog work", "Task", None)]);

    app.handle_key(key('/'), &bindings);
    app.handle_key(
        KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT),
        &bindings,
    );

    assert_eq!(app.filter(), "R");
}

#[test]
fn focused_issue_filter_still_allows_help_binding() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(vec![issue("KAN-1", "Catalog work", "Task", None)]);

    app.handle_key(key('/'), &bindings);
    app.handle_key(
        KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT),
        &bindings,
    );

    assert!(app.is_help_open());
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
    assert_eq!(app.selected_issue_key(), Some("KAN-3"));

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
    assert_eq!(app.selected_issue_key(), Some("KAN-3"));
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

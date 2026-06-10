mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use tira::{App, JiraIssueColumn, KeyBindings, draw};

use support::{ctrl, issue, key, test_issues};

fn left_click(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn scroll_down(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::ScrollDown,
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
fn slash_focuses_filter_and_captures_query_text() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(vec![
        issue("KAN-1", "Checkout work", "Task", None),
        issue("KAN-2", "Catalog work", "Task", None),
    ]);

    app.handle_key(key('/'), &bindings);
    app.handle_key(key('c'), &bindings);
    app.handle_key(key('a'), &bindings);
    app.handle_key(key('t'), &bindings);

    // Filtering is server-side: typing captures the query and focuses the
    // input, but does not locally narrow the loaded rows.
    assert!(app.is_filter_focused());
    assert_eq!(app.filter(), "cat");
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
fn mouse_click_on_open_column_trigger_closes_column_dropdown() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(test_issues(3));

    app.handle_mouse(left_click(90, 1), Rect::new(0, 0, 100, 20), &bindings);
    assert!(app.is_column_dropdown_open());

    app.handle_mouse(left_click(90, 1), Rect::new(0, 0, 100, 20), &bindings);
    assert!(!app.is_column_dropdown_open());
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
fn mouse_wheel_over_the_issue_list_scrolls_the_viewport_without_moving_selection() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(test_issues(20));

    app.handle_mouse(scroll_down(5, 3), Rect::new(0, 0, 100, 20), &bindings);

    assert_eq!(app.selected_issue_index(), 0);
    assert!(app.issue_scroll_offset() > 0);
}

#[test]
fn mouse_wheel_over_the_open_column_dropdown_scrolls_it_without_changing_selection() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(test_issues(20));

    app.handle_mouse(left_click(90, 1), Rect::new(0, 0, 100, 20), &bindings);
    assert!(app.is_column_dropdown_open());

    // Render through the real draw path so the dropdown's viewport is sized by
    // the terminal, rather than poking the layout via `visible_range`.
    let mut terminal = Terminal::new(TestBackend::new(100, 12)).expect("test terminal");
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");

    let before_selected = app
        .column_dropdown()
        .expect("column dropdown")
        .selected_index();
    let before_scroll = app
        .column_dropdown()
        .expect("column dropdown")
        .scroll_offset();

    app.handle_mouse(scroll_down(82, 4), Rect::new(0, 0, 100, 20), &bindings);

    let dropdown = app.column_dropdown().expect("column dropdown");
    assert_eq!(dropdown.selected_index(), before_selected);
    assert!(dropdown.scroll_offset() >= before_scroll);
}

#[test]
fn default_list_columns_use_fixed_order_and_column_selector_hides_fixed_columns() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(test_issues(5));

    assert_eq!(
        app.visible_issue_columns(),
        &[
            JiraIssueColumn::IssueKey,
            JiraIssueColumn::Field {
                id: String::from("priority"),
                label: String::from("Priority"),
            },
            JiraIssueColumn::Summary,
            JiraIssueColumn::Field {
                id: String::from("assignee"),
                label: String::from("Assignee"),
            },
            JiraIssueColumn::Status,
            JiraIssueColumn::Field {
                id: String::from("labels"),
                label: String::from("Labels"),
            },
        ]
    );

    app.handle_key(key('c'), &bindings);
    let labels = app
        .column_dropdown()
        .expect("column dropdown")
        .options()
        .iter()
        .map(|option| option.label.as_str())
        .collect::<Vec<_>>();

    assert!(!labels.contains(&"Work"));
    assert!(!labels.contains(&"Priority"));
    assert!(labels.contains(&"Assignee"));
    assert!(!labels.contains(&"Summary"));
    assert!(labels.contains(&"Status"));
    assert!(labels.contains(&"Labels"));
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
fn filter_enter_blurs_and_keeps_query_and_selection() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(vec![
        issue("KAN-1", "Checkout work", "Task", None),
        issue("KAN-2", "Catalog work", "Task", None),
        issue("KAN-3", "Cart polish", "Task", None),
    ]);

    app.handle_key(key('j'), &bindings);
    assert_eq!(app.selected_issue_key(), Some("KAN-2"));
    app.handle_key(key('/'), &bindings);
    for ch in "catalog".chars() {
        app.handle_key(key(ch), &bindings);
    }
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &bindings);

    // Enter blurs the input and keeps the query text; the search runs as a
    // background effect, so the loaded rows and selection are unchanged here.
    assert!(!app.is_filter_focused());
    assert_eq!(app.filter(), "catalog");
    assert_eq!(app.selected_issue_key(), Some("KAN-2"));
}

#[test]
fn ctrl_j_and_ctrl_k_navigate_list_while_filter_is_focused() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(vec![
        issue("KAN-1", "Checkout work", "Task", None),
        issue("KAN-2", "Catalog work", "Task", None),
        issue("KAN-3", "Cart polish", "Task", None),
    ]);

    // Focus the filter
    app.handle_key(key('/'), &bindings);
    assert!(app.is_filter_focused());
    assert_eq!(app.selected_issue_index(), 0);

    // Ctrl+J should move selection down
    app.handle_key(
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL),
        &bindings,
    );
    assert!(app.is_filter_focused());
    assert_eq!(app.selected_issue_index(), 1);

    // Ctrl+J again
    app.handle_key(
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL),
        &bindings,
    );
    assert_eq!(app.selected_issue_index(), 2);

    // Ctrl+K should move selection up
    app.handle_key(
        KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
        &bindings,
    );
    assert!(app.is_filter_focused());
    assert_eq!(app.selected_issue_index(), 1);
}

#[test]
fn delete_key_deletes_character_after_cursor_in_filter() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(vec![issue("KAN-1", "Catalog work", "Task", None)]);

    // Focus and type "cat"
    app.handle_key(key('/'), &bindings);
    app.handle_key(key('c'), &bindings);
    app.handle_key(key('a'), &bindings);
    app.handle_key(key('t'), &bindings);
    assert_eq!(app.filter(), "cat");

    // Move cursor left: "ca|t"
    app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), &bindings);

    // Press delete key: should delete 't' -> "ca"
    app.handle_key(
        KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE),
        &bindings,
    );
    assert_eq!(app.filter(), "ca");
}

#[test]
fn ctrl_enter_inside_filter_does_not_blur_filter() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(vec![issue("KAN-1", "Catalog work", "Task", None)]);

    // Focus and type
    app.handle_key(key('/'), &bindings);
    assert!(app.is_filter_focused());

    // Press Ctrl+Enter
    app.handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
        &bindings,
    );
    // Filter must remain focused!
    assert!(app.is_filter_focused());
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
}

#[test]
fn escape_blurs_filter_then_clears_query_text() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(vec![
        issue("KAN-1", "Checkout work", "Task", None),
        issue("KAN-2", "Catalog work", "Task", None),
        issue("KAN-3", "Cart polish", "Task", None),
    ]);

    app.handle_key(key('/'), &bindings);
    app.handle_key(key('c'), &bindings);
    app.handle_key(key('a'), &bindings);
    // First Escape blurs the focused input but keeps the query text.
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &bindings);
    assert!(!app.is_filter_focused());
    assert_eq!(app.filter(), "ca");

    // A second Escape in the tree clears the query text.
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &bindings);
    assert_eq!(app.filter(), "");

    // Ctrl-[ clears the query text the same way.
    app.handle_key(key('/'), &bindings);
    app.handle_key(key('c'), &bindings);
    app.handle_key(key('a'), &bindings);
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &bindings);
    app.handle_key(
        KeyEvent::new(KeyCode::Char('['), KeyModifiers::CONTROL),
        &bindings,
    );
    assert_eq!(app.filter(), "");
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
fn only_ctrl_q_quits_app() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(Vec::new());

    app.handle_key(key('q'), &bindings);
    assert!(app.is_running());

    app.handle_key(key('/'), &bindings);
    assert!(app.is_filter_focused());

    app.handle_key(key('q'), &bindings);
    assert!(app.is_filter_focused());
    assert_eq!(app.filter(), "q");

    app.handle_key(ctrl('q'), &bindings);
    assert!(!app.is_running());

    let configured_bindings = KeyBindings::from_toml_str(
        r##"
        [global]
        quit = "q"
        "##,
    );
    let mut configured_app = App::with_issues(Vec::new());
    configured_app.handle_key(key('q'), &configured_bindings);
    assert!(configured_app.is_running());
    let mut configured_ctrl_app = App::with_issues(Vec::new());
    configured_ctrl_app.handle_key(ctrl('q'), &configured_bindings);
    assert!(!configured_ctrl_app.is_running());
}

#[test]
fn ctrl_q_quits_even_after_pending_leader_key() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(Vec::new());

    app.handle_key(ctrl('x'), &bindings);
    app.handle_key(ctrl('q'), &bindings);

    assert!(!app.is_running());
}

#[test]
fn ctrl_q_quits_across_terminal_encodings() {
    let bindings = KeyBindings::default();
    let mut uppercase_app = App::with_issues(Vec::new());
    uppercase_app.handle_key(
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('Q'),
            crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::SHIFT,
        ),
        &bindings,
    );
    assert!(!uppercase_app.is_running());

    let mut control_code_app = App::with_issues(Vec::new());
    control_code_app.handle_key(
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('\u{11}'),
            crossterm::event::KeyModifiers::NONE,
        ),
        &bindings,
    );
    assert!(!control_code_app.is_running());
}

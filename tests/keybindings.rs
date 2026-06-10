mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tira::{
    Action, App, AppEffect, AppEvent, BoardAction, JiraFilteredTreeAction, JiraLoadPurpose,
    KeyBindings, Screen, TabAction,
    config::JiraCredentials,
    services::jira::{
        BoardColumnSummary, BoardData, BoardSwimlaneSummary, CommandLogEntry, SprintSummary,
        UserSummary,
    },
    ui::theme::ThemeName,
};

use support::{ctrl, key, shift};

#[test]
fn bracket_keys_move_between_tabs() {
    let bindings = KeyBindings::from_toml_str(
        r##"
        [tabs]
        previous_tab = "["
        next_tab = "]"
        "##,
    );
    let mut app = App::with_issues(Vec::new());
    assert_eq!(app.active_tab(), "List");

    app.dispatch(bindings.action_for(key(']')));
    assert_eq!(app.active_tab(), "Timeline");

    app.dispatch(bindings.action_for(key('[')));
    assert_eq!(app.active_tab(), "List");
}

#[test]
fn tab_navigation_wraps_around() {
    let mut app = App::with_issues(Vec::new());

    app.dispatch(Action::Tabs(TabAction::Previous));
    assert_eq!(app.active_tab(), "Board");

    app.dispatch(Action::Tabs(TabAction::Previous));
    assert_eq!(app.active_tab(), "Filters");

    app.dispatch(Action::Tabs(TabAction::Next));
    assert_eq!(app.active_tab(), "Board");
}

#[test]
fn configured_tab_keys_override_defaults() {
    let bindings = KeyBindings::from_toml_str(
        r##"
        [tabs]
        previous_tab = "h"
        next_tab = "l"
        "##,
    );

    assert_eq!(
        bindings.action_for(key('h')),
        Action::Tabs(TabAction::Previous)
    );
    assert_eq!(bindings.action_for(key('l')), Action::Tabs(TabAction::Next));
    assert_eq!(bindings.action_for(key('[')), Action::None);
    assert_eq!(bindings.action_for(key(']')), Action::None);
}

#[test]
fn board_navigation_uses_configurable_tree_keys_and_edges() {
    let bindings = KeyBindings::default();

    assert_eq!(
        bindings.board_action_for(key('h')),
        Action::Board(BoardAction::MoveLeft)
    );
    assert_eq!(
        bindings.board_action_for(key('l')),
        Action::Board(BoardAction::MoveRight)
    );
    assert_eq!(
        bindings.board_action_for(key('k')),
        Action::Board(BoardAction::MoveUp)
    );
    assert_eq!(
        bindings.board_action_for(key('j')),
        Action::Board(BoardAction::MoveDown)
    );
    assert_eq!(
        bindings.board_action_for(ctrl('u')),
        Action::Board(BoardAction::HalfPageUp)
    );
    assert_eq!(
        bindings.board_action_for(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE)),
        Action::Board(BoardAction::HalfPageUp)
    );
    assert_eq!(
        bindings.board_action_for(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)),
        Action::Board(BoardAction::HalfPageDown)
    );
    assert_eq!(
        bindings.board_action_for(ctrl('d')),
        Action::Board(BoardAction::HalfPageDown)
    );
    assert_eq!(
        bindings.board_action_for(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE)),
        Action::Board(BoardAction::GoToStart)
    );
    assert_eq!(
        bindings.board_action_for(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
        Action::Board(BoardAction::GoToEnd)
    );
    assert_eq!(
        bindings.board_action_for(key('g')),
        Action::Board(BoardAction::GoToStartPrefix)
    );
    assert_eq!(
        bindings.board_action_for(key('r')),
        Action::ToggleBoardGrouping
    );
    assert_eq!(
        bindings.board_action_for(shift('g')),
        Action::Board(BoardAction::GoToEnd)
    );
    assert_eq!(
        bindings.board_action_for(key('z')),
        Action::Board(BoardAction::CollapseAllGroups)
    );
    assert_eq!(
        bindings.board_action_for(shift('z')),
        Action::Board(BoardAction::ExpandAllGroups)
    );
}

#[test]
fn board_tab_keys_reach_all_cards() {
    let bindings = KeyBindings::default();
    let mut todo = support::issue("KAN-1", "Todo", "Task", None);
    todo.status = String::from("To Do");
    let mut doing = support::issue("KAN-2", "Doing", "Task", None);
    doing.status = String::from("In Progress");
    let mut done = support::issue("KAN-3", "Done", "Task", None);
    done.status = String::from("Done");
    let mut app = App::with_issues(vec![todo, doing, done]);
    app.dispatch(Action::Tabs(TabAction::Previous));

    app.handle_key(key('l'), &bindings);
    assert_eq!(app.selected_board_issue_key(), Some("KAN-2"));

    app.handle_key(key('l'), &bindings);
    assert_eq!(app.selected_board_issue_key(), Some("KAN-3"));

    app.handle_key(key('h'), &bindings);
    assert_eq!(app.selected_board_issue_key(), Some("KAN-2"));

    app.handle_key(key('h'), &bindings);
    assert_eq!(app.selected_board_issue_key(), Some("KAN-1"));

    app.handle_key(key('j'), &bindings);
    assert_eq!(app.selected_board_issue_key(), Some("KAN-1"));
}

#[test]
fn board_home_end_stay_within_the_focused_column() {
    let bindings = KeyBindings::default();
    let mut todo_top = support::issue("KAN-1", "Todo top", "Task", None);
    todo_top.status = String::from("To Do");
    let mut todo_bottom = support::issue("KAN-2", "Todo bottom", "Task", None);
    todo_bottom.status = String::from("To Do");
    let mut done = support::issue("KAN-3", "Done", "Task", None);
    done.status = String::from("Done");
    let mut app = App::with_issues(vec![todo_top, todo_bottom, done]);
    app.dispatch(Action::Tabs(TabAction::Previous));

    // End/Home move to the bottom/top of the focused column, not across all cards.
    app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), &bindings);
    assert_eq!(app.selected_board_issue_key(), Some("KAN-2"));
    app.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE), &bindings);
    assert_eq!(app.selected_board_issue_key(), Some("KAN-1"));

    // Moving to the Done column, End stays on that column's single card.
    app.handle_key(key('l'), &bindings);
    assert_eq!(app.selected_board_issue_key(), Some("KAN-3"));
    app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), &bindings);
    assert_eq!(app.selected_board_issue_key(), Some("KAN-3"));
}

#[test]
fn board_down_stops_at_end_of_swimlane_column() {
    let bindings = KeyBindings::default();
    let mut first = support::issue("KAN-1", "First lane", "Task", None);
    first.status = String::from("To Do");
    let mut second = support::issue("KAN-2", "Second lane", "Task", None);
    second.status = String::from("To Do");
    let board = BoardData {
        id: 2,
        name: String::from("Kanban"),
        columns: vec![BoardColumnSummary {
            name: String::from("To Do"),
            statuses: vec![String::from("To Do")],
            max: None,
        }],
        swimlanes: vec![
            BoardSwimlaneSummary {
                id: None,
                name: String::from("Lane A"),
                issue_keys: vec![String::from("KAN-1")],
            },
            BoardSwimlaneSummary {
                id: None,
                name: String::from("Lane B"),
                issue_keys: vec![String::from("KAN-2")],
            },
        ],
        issues: vec![first, second],
        sprint: None,
    };
    let mut app = App::with_board_data(board);
    app.dispatch(Action::Tabs(TabAction::Previous));
    app.handle_key(key('j'), &bindings);
    assert_eq!(app.selected_board_issue_key(), Some("KAN-1"));
}

#[test]
fn board_shift_g_jumps_to_end_and_r_opens_grouping() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(support::test_issues(5));
    app.dispatch(Action::Tabs(TabAction::Previous));

    app.handle_key(shift('g'), &bindings);
    assert_eq!(app.selected_board_issue_key(), Some("KAN-5"));

    app.handle_key(key('r'), &bindings);
    assert!(app.board_group_dropdown().is_some());
}

#[test]
fn board_empty_columns_are_focusable_when_navigating() {
    let mut todo = support::issue("KAN-1", "Todo card", "Task", None);
    todo.status = String::from("To Do");
    let mut review = support::issue("KAN-3", "Review card", "Task", None);
    review.status = String::from("In Review");
    let board = BoardData {
        id: 2,
        name: String::from("Kanban"),
        columns: vec![
            BoardColumnSummary {
                name: String::from("To Do"),
                statuses: vec![String::from("To Do")],
                max: None,
            },
            BoardColumnSummary {
                name: String::from("In Progress"),
                statuses: vec![String::from("In Progress")],
                max: None,
            },
            BoardColumnSummary {
                name: String::from("In Review"),
                statuses: vec![String::from("In Review")],
                max: None,
            },
        ],
        swimlanes: vec![BoardSwimlaneSummary {
            id: None,
            name: String::from("Issues"),
            issue_keys: vec![String::from("KAN-1"), String::from("KAN-3")],
        }],
        issues: vec![todo, review],
        sprint: None,
    };
    let mut app = App::with_board_data(board);
    app.dispatch(Action::Tabs(TabAction::Previous));

    // Initial selection sits on the first populated column.
    assert_eq!(app.selected_board_issue_key(), Some("KAN-1"));

    // Moving right lands on the empty middle column instead of skipping it.
    app.dispatch(Action::Board(BoardAction::MoveRight));
    assert_eq!(app.selected_board_issue_key(), None);
    assert_eq!(app.selected_board_empty_cell(), Some(("Issues", 1)));

    // Moving right again continues to the populated third column.
    app.dispatch(Action::Board(BoardAction::MoveRight));
    assert_eq!(app.selected_board_issue_key(), Some("KAN-3"));

    // And back left returns through the empty column.
    app.dispatch(Action::Board(BoardAction::MoveLeft));
    assert_eq!(app.selected_board_empty_cell(), Some(("Issues", 1)));
}

#[test]
fn board_vertical_navigation_preserves_column_across_group_headers() {
    // Alice has a card only in column 0; Bob has a card only in column 2.
    // Grouped by assignee, going up from Bob's column-2 card through Bob's
    // header should land on Alice's empty column 2 (preserving the column),
    // not jump to Alice's populated column 0.
    let bindings = KeyBindings::default();
    let mut alice = support::issue("KAN-A", "Alice todo", "Task", None);
    alice.status = String::from("To Do");
    alice
        .field_values
        .insert(String::from("assignee"), String::from("Alice"));
    let mut bob = support::issue("KAN-B", "Bob review", "Task", None);
    bob.status = String::from("In Review");
    bob.field_values
        .insert(String::from("assignee"), String::from("Bob"));
    let columns = vec![
        BoardColumnSummary {
            name: String::from("To Do"),
            statuses: vec![String::from("To Do")],
            max: None,
        },
        BoardColumnSummary {
            name: String::from("In Progress"),
            statuses: vec![String::from("In Progress")],
            max: None,
        },
        BoardColumnSummary {
            name: String::from("In Review"),
            statuses: vec![String::from("In Review")],
            max: None,
        },
    ];
    let board = BoardData {
        id: 2,
        name: String::from("Kanban"),
        columns,
        swimlanes: vec![BoardSwimlaneSummary {
            id: None,
            name: String::from("Issues"),
            issue_keys: vec![String::from("KAN-A"), String::from("KAN-B")],
        }],
        issues: vec![alice, bob],
        sprint: None,
    };
    let mut app = App::with_board_data(board);
    app.dispatch(Action::Tabs(TabAction::Previous));
    // Group by assignee.
    app.handle_key(key('r'), &bindings);
    app.handle_key(ctrl('j'), &bindings);
    app.handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
        &bindings,
    );

    // Expand into the lane (first right), then step to column 2; End jumps to
    // the bottom of that column (Bob's card).
    app.handle_key(key('l'), &bindings);
    app.handle_key(key('l'), &bindings);
    app.handle_key(key('l'), &bindings);
    app.dispatch(Action::Board(BoardAction::GoToEnd));
    assert_eq!(app.selected_board_issue_key(), Some("KAN-B"));

    // Up → Bob's header; up again → Alice's empty column 2 (column preserved).
    app.dispatch(Action::Board(BoardAction::MoveUp));
    assert_eq!(app.selected_board_group(), Some("Bob"));
    app.dispatch(Action::Board(BoardAction::MoveUp));
    assert_eq!(app.selected_board_empty_cell(), Some(("Alice", 2)));
}

#[test]
fn board_group_rows_can_be_selected_and_collapsed() {
    let bindings = KeyBindings::default();
    let mut assigned = support::issue("KAN-1", "Assigned", "Task", None);
    assigned.status = String::from("To Do");
    assigned
        .field_values
        .insert(String::from("assignee"), String::from("Marlo Vlietstra"));
    let mut unassigned = support::issue("KAN-2", "Unassigned", "Task", None);
    unassigned.status = String::from("To Do");
    let mut app = App::with_issues(vec![assigned, unassigned]);
    app.dispatch(Action::Tabs(TabAction::Previous));
    app.handle_key(key('r'), &bindings);
    app.handle_key(ctrl('j'), &bindings);
    app.handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
        &bindings,
    );

    app.dispatch(Action::Board(BoardAction::GoToStart));
    app.dispatch(Action::Board(BoardAction::MoveUp));
    assert_eq!(app.selected_board_group(), Some("Marlo Vlietstra"));

    app.handle_key(key(' '), &bindings);
    assert!(app.is_board_group_collapsed("Marlo Vlietstra"));
    app.handle_key(key('l'), &bindings);
    assert!(!app.is_board_group_collapsed("Marlo Vlietstra"));
    app.handle_key(key('z'), &bindings);
    assert!(app.is_board_group_collapsed("Marlo Vlietstra"));
    assert!(app.is_board_group_collapsed("Unassigned"));
    app.handle_key(shift('z'), &bindings);
    assert!(!app.is_board_group_collapsed("Marlo Vlietstra"));
    assert!(!app.is_board_group_collapsed("Unassigned"));
}

#[test]
fn board_horizontal_keys_move_between_group_rows_and_first_column() {
    let bindings = KeyBindings::default();
    let mut first = support::issue("KAN-1", "Assigned first column", "Task", None);
    first.status = String::from("To Do");
    first
        .field_values
        .insert(String::from("assignee"), String::from("Marlo Vlietstra"));
    let board = BoardData {
        id: 2,
        name: String::from("Kanban"),
        columns: vec![BoardColumnSummary {
            name: String::from("To Do"),
            statuses: vec![String::from("To Do")],
            max: None,
        }],
        swimlanes: vec![BoardSwimlaneSummary {
            id: None,
            name: String::from("Issues"),
            issue_keys: vec![String::from("KAN-1")],
        }],
        issues: vec![first],
        sprint: None,
    };
    let mut app = App::with_board_data(board);
    app.dispatch(Action::Tabs(TabAction::Previous));
    app.handle_key(key('r'), &bindings);
    app.handle_key(ctrl('j'), &bindings);
    app.handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
        &bindings,
    );

    app.dispatch(Action::Board(BoardAction::GoToStart));
    assert_eq!(app.selected_board_issue_key(), Some("KAN-1"));

    app.handle_key(key('h'), &bindings);
    assert_eq!(app.selected_board_group(), Some("Marlo Vlietstra"));

    app.handle_key(key('l'), &bindings);
    assert_eq!(app.selected_board_issue_key(), Some("KAN-1"));
}

#[test]
fn board_collapse_all_keeps_focus_on_current_group() {
    let bindings = KeyBindings::default();
    let mut assigned = support::issue("KAN-1", "Assigned", "Task", None);
    assigned.status = String::from("To Do");
    assigned
        .field_values
        .insert(String::from("assignee"), String::from("Marlo Vlietstra"));
    let mut unassigned = support::issue("KAN-2", "Unassigned", "Task", None);
    unassigned.status = String::from("To Do");
    let mut app = App::with_issues(vec![assigned, unassigned]);
    app.dispatch(Action::Tabs(TabAction::Previous));
    app.handle_key(key('r'), &bindings);
    app.handle_key(ctrl('j'), &bindings);
    app.handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
        &bindings,
    );

    app.dispatch(Action::Board(BoardAction::GoToStart));
    assert_eq!(app.selected_board_issue_key(), Some("KAN-1"));

    app.handle_key(key('z'), &bindings);
    assert_eq!(app.selected_board_group(), Some("Marlo Vlietstra"));
    assert!(app.is_board_group_collapsed("Marlo Vlietstra"));
    assert!(app.is_board_group_collapsed("Unassigned"));
}

#[test]
fn board_page_keys_transcend_groups_within_same_swimlane() {
    let bindings = KeyBindings::default();
    let mut assigned = support::issue("KAN-1", "Assigned card", "Task", None);
    assigned.status = String::from("To Do");
    assigned
        .field_values
        .insert(String::from("assignee"), String::from("Marlo Vlietstra"));
    let mut unassigned = support::issue("KAN-2", "Unassigned card", "Task", None);
    unassigned.status = String::from("To Do");
    let board = BoardData {
        id: 2,
        name: String::from("Kanban"),
        columns: vec![BoardColumnSummary {
            name: String::from("To Do"),
            statuses: vec![String::from("To Do")],
            max: None,
        }],
        swimlanes: vec![BoardSwimlaneSummary {
            id: None,
            name: String::from("Issues"),
            issue_keys: vec![String::from("KAN-1"), String::from("KAN-2")],
        }],
        issues: vec![assigned, unassigned],
        sprint: None,
    };
    let mut app = App::with_board_data(board);
    app.dispatch(Action::Tabs(TabAction::Previous));
    app.handle_key(key('r'), &bindings);
    app.handle_key(ctrl('j'), &bindings);
    app.handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
        &bindings,
    );

    app.dispatch(Action::Board(BoardAction::GoToStart));
    assert_eq!(app.selected_board_issue_key(), Some("KAN-1"));

    app.handle_key(ctrl('d'), &bindings);
    assert_eq!(app.selected_board_issue_key(), Some("KAN-2"));

    app.handle_key(ctrl('u'), &bindings);
    assert_eq!(app.selected_board_issue_key(), Some("KAN-1"));
}

#[test]
fn board_navigation_respects_active_search_filter() {
    let bindings = KeyBindings::default();
    let mut hidden = support::issue("KAN-1", "Hidden card", "Task", None);
    hidden.status = String::from("To Do");
    let mut visible = support::issue("KAN-2", "Visible card", "Task", None);
    visible.status = String::from("To Do");
    visible
        .field_values
        .insert(String::from("dueDate"), String::from("2026-06-10"));
    let mut app = App::with_issues(vec![hidden, visible]);
    app.dispatch(Action::Tabs(TabAction::Previous));

    app.handle_key(key('/'), &bindings);
    for c in "2026".chars() {
        app.handle_key(key(c), &bindings);
    }
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &bindings);
    app.handle_key(key('j'), &bindings);

    assert_eq!(app.selected_board_issue_key(), Some("KAN-2"));
}

#[test]
fn board_page_keys_move_across_many_cards() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(support::test_issues(12));
    app.dispatch(Action::Tabs(TabAction::Previous));

    app.handle_key(ctrl('d'), &bindings);
    assert_eq!(app.selected_board_issue_key(), Some("KAN-5"));

    app.handle_key(
        KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
        &bindings,
    );
    assert_eq!(app.selected_board_issue_key(), Some("KAN-9"));

    app.handle_key(ctrl('u'), &bindings);
    assert_eq!(app.selected_board_issue_key(), Some("KAN-5"));

    app.handle_key(
        KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
        &bindings,
    );
    assert_eq!(app.selected_board_issue_key(), Some("KAN-1"));
}

#[test]
fn board_page_keys_stop_at_swimlane_edges() {
    let bindings = KeyBindings::default();
    let mut first = support::issue("KAN-1", "First lane one", "Task", None);
    first.status = String::from("To Do");
    let mut second = support::issue("KAN-2", "First lane two", "Task", None);
    second.status = String::from("To Do");
    let mut third = support::issue("KAN-3", "Second lane one", "Task", None);
    third.status = String::from("To Do");
    let board = BoardData {
        id: 2,
        name: String::from("Kanban"),
        columns: vec![BoardColumnSummary {
            name: String::from("To Do"),
            statuses: vec![String::from("To Do")],
            max: None,
        }],
        swimlanes: vec![
            BoardSwimlaneSummary {
                id: None,
                name: String::from("Lane A"),
                issue_keys: vec![String::from("KAN-1"), String::from("KAN-2")],
            },
            BoardSwimlaneSummary {
                id: None,
                name: String::from("Lane B"),
                issue_keys: vec![String::from("KAN-3")],
            },
        ],
        issues: vec![first, second, third],
        sprint: None,
    };
    let mut app = App::with_board_data(board);
    app.dispatch(Action::Tabs(TabAction::Previous));

    app.handle_key(ctrl('d'), &bindings);
    assert_eq!(app.selected_board_issue_key(), Some("KAN-2"));

    app.handle_key(
        KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
        &bindings,
    );
    assert_eq!(app.selected_board_issue_key(), Some("KAN-2"));

    app.handle_key(ctrl('u'), &bindings);
    assert_eq!(app.selected_board_issue_key(), Some("KAN-1"));

    app.handle_key(
        KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
        &bindings,
    );
    assert_eq!(app.selected_board_issue_key(), Some("KAN-1"));
}

#[test]
fn filter_escape_blurs_but_keeps_value() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(support::test_issues(5));
    app.dispatch(Action::Tabs(TabAction::Previous));

    app.handle_key(key('/'), &bindings);
    for c in "KAN-5".chars() {
        app.handle_key(key(c), &bindings);
    }
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &bindings);

    assert!(!app.is_board_filter_focused());
    assert_eq!(app.board_filter(), "KAN-5");
}

#[test]
fn board_escape_clears_filter_and_selects_top_left() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(support::test_issues(5));
    app.dispatch(Action::Tabs(TabAction::Previous));
    app.handle_key(shift('g'), &bindings);
    assert_eq!(app.selected_board_issue_key(), Some("KAN-5"));

    app.handle_key(key('/'), &bindings);
    for c in "KAN-5".chars() {
        app.handle_key(key(c), &bindings);
    }
    // First Esc blurs the filter, keeping the value.
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &bindings);
    assert_eq!(app.board_filter(), "KAN-5");

    // Esc from the board itself clears the filter and selects the top-left card.
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &bindings);

    assert_eq!(app.board_filter(), "");
    assert_eq!(app.selected_board_issue_key(), Some("KAN-1"));
}
#[test]
fn issue_url_yank_binding_is_configurable_separately_from_columns() {
    let bindings = KeyBindings::from_toml_str(
        r##"
        [tree]
        open_columns = "c"
        yank_issue_url = "x"
        "##,
    );

    assert_eq!(
        bindings.action_for(key('x')),
        Action::JiraFilteredTree(JiraFilteredTreeAction::YankIssueUrlPrefix)
    );
    assert_eq!(bindings.action_for(key('y')), Action::None);
    assert_eq!(
        bindings.action_for(key('c')),
        Action::JiraFilteredTree(JiraFilteredTreeAction::OpenColumns)
    );
}

#[test]
fn leader_keys_are_mapped() {
    let bindings = KeyBindings::default();

    assert_eq!(
        bindings.global_action_for(shift('r')),
        Some(Action::ReloadList)
    );
    assert_eq!(bindings.global_action_for(ctrl('x')), Some(Action::Leader));
    assert_eq!(
        bindings.leader_action_for(key('c')),
        Action::ToggleCommandLog
    );
    assert_eq!(bindings.leader_action_for(key('l')), Action::GoToList);
    assert_eq!(bindings.leader_action_for(key('b')), Action::GoToBoard);
    assert_eq!(bindings.leader_action_for(key('t')), Action::GoToTimeline);
    assert_eq!(bindings.leader_action_for(key('f')), Action::GoToFilters);
    assert_eq!(
        bindings.leader_action_for(key('s')),
        Action::ToggleThemeDropdown
    );
    assert_eq!(
        bindings.global_action_for(key('r')),
        Some(Action::ReloadNode)
    );
}

#[test]
fn leader_tab_keys_jump_to_named_tabs() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(Vec::new());

    app.handle_key(ctrl('x'), &bindings);
    app.handle_key(key('b'), &bindings);
    assert_eq!(app.active_tab(), "Board");

    app.handle_key(ctrl('x'), &bindings);
    app.handle_key(key('t'), &bindings);
    assert_eq!(app.active_tab(), "Timeline");

    app.handle_key(ctrl('x'), &bindings);
    app.handle_key(key('f'), &bindings);
    assert_eq!(app.active_tab(), "Filters");

    app.handle_key(ctrl('x'), &bindings);
    app.handle_key(key('l'), &bindings);
    assert_eq!(app.active_tab(), "List");
}

#[test]
fn assignee_shortcut_opens_selector_and_assignment_event_updates_issue() {
    let bindings = KeyBindings::default();
    let marlo = UserSummary {
        account_id: String::from("account-1"),
        display_name: String::from("Marlo Vlietstra"),
    };
    let mut app = App::with_issues_projects_and_users(
        vec![support::issue("KAN-1", "Catalog work", "Task", None)],
        Vec::new(),
        vec![marlo.clone()],
        "KAN",
    );

    app.handle_key(key('a'), &bindings);
    assert!(app.is_assignee_dropdown_open());
    assert!(app.is_assignee_dropdown_filter_focused());

    app.handle_key(ctrl('j'), &bindings);
    app.handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
        &bindings,
    );

    let effects = app.take_effects();
    assert_eq!(effects.len(), 1);
    let AppEffect::AssignIssue {
        issue_key,
        assignee,
        ..
    } = &effects[0]
    else {
        panic!("expected assign issue effect");
    };
    assert_eq!(issue_key, "KAN-1");
    assert_eq!(assignee.as_ref(), Some(&marlo));

    app.handle_event(AppEvent::IssueAssigned {
        request_id: 1,
        issue_key: issue_key.clone(),
        assignee: assignee.clone(),
        result: Ok(CommandLogEntry {
            timestamp: String::from("10:00:00"),
            method: "PUT",
            path: String::from("/issue/KAN-1/assignee"),
            status: String::from("204"),
            duration_ms: 12,
        }),
    });

    assert_eq!(
        app.issues()[0]
            .field_values
            .get("assignee")
            .map(String::as_str),
        Some("Marlo Vlietstra")
    );
}

#[test]
fn assignee_selector_can_unassign_existing_assignee() {
    let bindings = KeyBindings::default();
    let marlo = UserSummary {
        account_id: String::from("account-1"),
        display_name: String::from("Marlo Vlietstra"),
    };
    let mut issue = support::issue("KAN-1", "Catalog work", "Task", None);
    issue
        .field_values
        .insert(String::from("assignee"), String::from("Marlo Vlietstra"));
    let mut app = App::with_issues_projects_and_users(vec![issue], Vec::new(), vec![marlo], "KAN");

    app.handle_key(key('a'), &bindings);
    app.handle_key(
        KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
        &bindings,
    );
    app.handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
        &bindings,
    );

    let effects = app.take_effects();
    let AppEffect::AssignIssue {
        issue_key,
        assignee,
        ..
    } = &effects[0]
    else {
        panic!("expected assign issue effect");
    };
    assert_eq!(issue_key, "KAN-1");
    assert_eq!(assignee, &None);
}

#[test]
fn assign_to_me_and_unassign_shortcuts_queue_assignment_effects() {
    let bindings = KeyBindings::default();
    let marlo = UserSummary {
        account_id: String::from("account-1"),
        display_name: String::from("Marlo Vlietstra"),
    };
    let mut app = App::with_issues_projects_and_users(
        vec![support::issue("KAN-1", "Catalog work", "Task", None)],
        Vec::new(),
        vec![marlo.clone()],
        "KAN",
    );

    app.handle_key(key('i'), &bindings);
    let effects = app.take_effects();
    let AppEffect::AssignIssue {
        issue_key,
        assignee,
        ..
    } = &effects[0]
    else {
        panic!("expected assign issue effect");
    };
    assert_eq!(issue_key, "KAN-1");
    assert_eq!(assignee.as_ref(), Some(&marlo));

    app.handle_key(key('u'), &bindings);
    let effects = app.take_effects();
    let AppEffect::AssignIssue {
        issue_key,
        assignee,
        ..
    } = &effects[0]
    else {
        panic!("expected unassign issue effect");
    };
    assert_eq!(issue_key, "KAN-1");
    assert_eq!(assignee, &None);
}

#[test]
fn board_assignment_shortcuts_update_selected_card_and_notify() {
    let bindings = KeyBindings::default();
    let marlo = UserSummary {
        account_id: String::from("account-1"),
        display_name: String::from("Marlo Vlietstra"),
    };
    let mut card = support::issue("KAN-1", "Board work", "Task", None);
    card.status = String::from("To Do");
    let mut app =
        App::with_issues_projects_and_users(vec![card], Vec::new(), vec![marlo.clone()], "KAN");
    app.dispatch(Action::Tabs(TabAction::Previous));

    app.handle_key(key('a'), &bindings);
    assert!(app.is_assignee_dropdown_open());
    let mut app = App::with_issues_projects_and_users(
        vec![support::issue("KAN-1", "Board work", "Task", None)],
        Vec::new(),
        vec![marlo.clone()],
        "KAN",
    );
    app.dispatch(Action::Tabs(TabAction::Previous));

    app.handle_key(key('i'), &bindings);
    let effects = app.take_effects();
    let AppEffect::AssignIssue {
        request_id,
        issue_key,
        assignee,
        ..
    } = &effects[0]
    else {
        panic!("expected assign issue effect");
    };
    assert_eq!(issue_key, "KAN-1");
    assert_eq!(assignee.as_ref(), Some(&marlo));

    app.handle_event(AppEvent::IssueAssigned {
        request_id: *request_id,
        issue_key: issue_key.clone(),
        assignee: assignee.clone(),
        result: Ok(CommandLogEntry {
            timestamp: String::from("10:00:00"),
            method: "PUT",
            path: String::from("/issue/KAN-1/assignee"),
            status: String::from("204"),
            duration_ms: 12,
        }),
    });

    let board_issue = &app.board().data().expect("board").issues[0];
    assert_eq!(
        board_issue.field_values.get("assignee").map(String::as_str),
        Some("Marlo Vlietstra")
    );
    assert_eq!(app.notifications()[0].title(), "Assignee updated");

    app.handle_key(key('u'), &bindings);
    let effects = app.take_effects();
    let AppEffect::AssignIssue {
        issue_key,
        assignee,
        ..
    } = &effects[0]
    else {
        panic!("expected unassign issue effect");
    };
    assert_eq!(issue_key, "KAN-1");
    assert_eq!(assignee, &None);
}
#[test]
fn old_global_project_theme_log_keys_are_unbound() {
    let bindings = KeyBindings::default();

    assert_eq!(bindings.global_action_for(shift('l')), None);
    assert_eq!(bindings.global_action_for(shift('p')), None);
    assert_eq!(bindings.global_action_for(shift('t')), None);
}

#[test]
fn help_items_use_configured_leader_bindings() {
    let bindings = KeyBindings::from_toml_str(
        r##"
        [global]
        leader = "ctrl+g"
        reload_list = "r"

        [leader]
        command_log = "o"
        project = "m"
        theme = "n"
        board = "b"
        list = "l"
        timeline = "t"
        filters = "f"
        "##,
    );

    let items = bindings.help_items(Screen::Main, "List", false);

    assert!(items.iter().any(|item| item.binding == "Ctrl+g o"));
    assert!(items.iter().any(|item| item.binding == "Ctrl+g m"));
    assert!(items.iter().any(|item| item.binding == "Ctrl+g n"));
    assert!(items.iter().any(|item| item.binding == "r"));
    assert!(items.iter().any(|item| item.binding == "Ctrl+g b"));
    assert!(items.iter().any(|item| item.binding == "Ctrl+g l"));
    assert!(items.iter().any(|item| item.binding == "Ctrl+g t"));
    assert!(items.iter().any(|item| item.binding == "Ctrl+g f"));
}

#[test]
fn reload_help_entries_are_tab_scoped() {
    let bindings = KeyBindings::default();

    let list_items = bindings.help_items(Screen::Main, "List", false);
    let board_items = bindings.help_items(Screen::Main, "Board", false);

    assert!(list_items.iter().any(|item| item.summary == "Reload list"));
    assert!(!list_items.iter().any(|item| item.summary == "Reload board"));
    assert!(
        board_items
            .iter()
            .any(|item| item.summary == "Reload board")
    );
    assert!(!board_items.iter().any(|item| item.summary == "Reload list"));
    assert!(
        board_items
            .iter()
            .any(|item| item.summary == "Search board")
    );
    assert!(
        board_items
            .iter()
            .any(|item| item.summary == "Move columns")
    );
    assert!(board_items.iter().any(|item| item.summary == "Page cards"));
}

#[test]
fn board_help_uses_custom_navigation_labels() {
    let bindings = KeyBindings::from_toml_str(
        r##"
        [board]
        move_up = "w"
        move_down = "s"
        move_left = "a"
        move_right = "d"
        page_up = "ctrl+b"
        page_down = "ctrl+f"
        first = "home"
        last = "end"
        "##,
    );

    let board_items = bindings.help_items(Screen::Main, "Board", false);

    assert!(board_items.iter().any(|item| item.binding == "a/d"));
    assert!(board_items.iter().any(|item| item.binding == "w/s"));
    assert!(
        board_items
            .iter()
            .any(|item| item.binding == "Ctrl+b / Ctrl+f")
    );
}

#[test]
fn copy_help_entry_shows_yy_and_respects_custom_binding() {
    let bindings = KeyBindings::default();
    let list_items = bindings.help_items(Screen::Main, "List", false);
    let copy_item = list_items
        .iter()
        .find(|item| item.summary == "Copy issue URL")
        .unwrap();
    assert_eq!(copy_item.binding, "yy");

    let custom_bindings = KeyBindings::from_toml_str(
        r##"
        [tree]
        yank_issue_url = "x"
        "##,
    );
    let custom_list_items = custom_bindings.help_items(Screen::Main, "List", false);
    let custom_copy_item = custom_list_items
        .iter()
        .find(|item| item.summary == "Copy issue URL")
        .unwrap();
    assert_eq!(custom_copy_item.binding, "xx");
}

#[test]
fn help_navigation_bindings_are_configurable() {
    let bindings = KeyBindings::from_toml_str(
        r##"
        [help]
        move_up = "w"
        move_down = "s"
        first = "a"
        last = "d"
        close = "x"
        "##,
    );

    assert_eq!(
        bindings.help_dialog_action_for(key('w')),
        tira::keymap::HelpDialogAction::Up
    );
    assert_eq!(
        bindings.help_dialog_action_for(key('s')),
        tira::keymap::HelpDialogAction::Down
    );
    assert_eq!(
        bindings.help_dialog_action_for(key('a')),
        tira::keymap::HelpDialogAction::First
    );
    assert_eq!(
        bindings.help_dialog_action_for(key('d')),
        tira::keymap::HelpDialogAction::Last
    );
    assert_eq!(
        bindings.help_dialog_action_for(key('x')),
        tira::keymap::HelpDialogAction::Close
    );
}

#[test]
fn configured_open_help_key_opens_and_closes_help_dialog() {
    let bindings = KeyBindings::from_toml_str(
        r##"
        [global]
        open_help = "!"
        "##,
    );
    let mut app = App::with_issues(Vec::new());

    app.handle_key(key('!'), &bindings);
    assert!(app.is_help_open());

    app.handle_key(key('!'), &bindings);
    assert!(!app.is_help_open());
}

#[test]
fn ctrl_j_and_ctrl_k_navigate_dropdown_options_while_filter_is_focused() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(Vec::new());

    // Open theme picker (which has a search/filter input)
    app.handle_key(ctrl('x'), &bindings);
    app.handle_key(key('s'), &bindings);
    assert!(app.is_theme_dropdown_open());

    assert!(app.is_theme_dropdown_filter_focused());

    let initial = app
        .theme_dropdown()
        .expect("theme dropdown")
        .selected_index();

    app.handle_key(
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL),
        &bindings,
    );
    assert!(app.is_theme_dropdown_filter_focused());
    let dropdown = app.theme_dropdown().expect("theme dropdown");
    assert!(dropdown.selected_index() > initial);

    app.handle_key(
        KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
        &bindings,
    );
    assert!(app.is_theme_dropdown_filter_focused());
    let dropdown = app.theme_dropdown().expect("theme dropdown");
    assert_eq!(dropdown.selected_index(), initial);
}

#[test]
fn help_dialog_during_dropdown_does_not_close_dropdown_and_shows_dropdown_help() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(Vec::new());

    // Open theme picker
    app.handle_key(ctrl('x'), &bindings);
    app.handle_key(key('s'), &bindings);
    assert!(app.is_theme_dropdown_open());

    // Press ? to open help
    app.handle_key(key('?'), &bindings);
    assert!(app.is_help_open());
    // Dropdown MUST remain open!
    assert!(app.is_theme_dropdown_open());

    // The help items should contain dropdown-specific help
    let items = bindings.help_items(app.screen(), app.active_tab(), app.is_any_dropdown_open());
    assert!(items.iter().any(|item| item.summary == "Toggle selection"));
    assert!(items.iter().any(|item| item.summary == "Do selection"));

    // Close help
    app.handle_key(key('?'), &bindings);
    assert!(!app.is_help_open());
    // Dropdown is still open
    assert!(app.is_theme_dropdown_open());
}

#[test]
fn configured_theme_picker_switches_theme() {
    let bindings = KeyBindings::from_toml_str(
        r##"
        [global]
        leader = "ctrl+g"

        [leader]
        theme = "T"
        "##,
    );
    let mut app = App::with_issues(Vec::new());

    app.handle_key(ctrl('g'), &bindings);
    app.handle_key(shift('t'), &bindings);
    assert!(app.is_theme_dropdown_open());
    app.handle_key(key('c'), &bindings);
    app.handle_key(key('a'), &bindings);
    app.handle_key(key('t'), &bindings);
    app.handle_key(
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::CONTROL,
        ),
        &bindings,
    );

    assert_eq!(app.theme().name(), ThemeName::Catppuccin);
    assert!(app.status().contains("Catppuccin"));
}

#[test]
fn theme_picker_focuses_current_theme() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(Vec::new());
    app.set_theme(tira::ui::theme::Theme::named(ThemeName::Catppuccin));

    app.handle_key(ctrl('x'), &bindings);
    app.handle_key(key('s'), &bindings);
    app.handle_key(
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::CONTROL,
        ),
        &bindings,
    );

    assert_eq!(app.theme().name(), ThemeName::Catppuccin);
}

#[test]
fn quick_switcher_enter_with_no_results_does_not_run_stale_action() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(Vec::new());

    app.handle_key(ctrl('k'), &bindings);
    app.handle_key(key('/'), &bindings);
    app.handle_key(key('z'), &bindings);
    app.handle_key(
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ),
        &bindings,
    );

    assert!(app.is_quick_switcher_open());
    assert!(!app.is_command_log_open());
}

#[test]
fn theme_picker_previews_focus_and_reverts_when_closed() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(Vec::new());

    app.handle_key(ctrl('x'), &bindings);
    app.handle_key(key('s'), &bindings);
    app.handle_key(key('c'), &bindings);
    app.handle_key(key('a'), &bindings);
    app.handle_key(key('t'), &bindings);
    assert_eq!(app.theme().name(), ThemeName::Catppuccin);

    app.handle_key(
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ),
        &bindings,
    );
    app.handle_key(
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ),
        &bindings,
    );

    assert_eq!(app.theme().name(), ThemeName::TokyoNight);
}

#[test]
fn quick_switcher_opens_command_log_and_jumps_to_tabs() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(Vec::new());

    app.handle_key(ctrl('k'), &bindings);
    assert!(app.is_quick_switcher_open());
    app.handle_key(
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::CONTROL,
        ),
        &bindings,
    );
    assert!(app.is_command_log_open());

    app.dispatch(Action::CloseCommandLog);
    app.handle_key(ctrl('k'), &bindings);
    for _ in 0..6 {
        app.handle_key(
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL),
            &bindings,
        );
    }
    app.handle_key(
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::CONTROL,
        ),
        &bindings,
    );

    assert_eq!(app.active_tab(), "Timeline");
}

#[test]
fn quick_switcher_lists_tab_scoped_reload_actions() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(Vec::new());

    app.handle_key(ctrl('k'), &bindings);
    let list_options = app.quick_switcher().expect("quick switcher").options();
    assert!(
        list_options
            .iter()
            .any(|option| option.label == "Reload list")
    );
    assert!(
        !list_options
            .iter()
            .any(|option| option.label == "Reload board")
    );
    let list_shortcuts = list_options
        .iter()
        .map(|option| (option.label.clone(), option.value))
        .collect::<Vec<_>>();

    app.dispatch(Action::Tabs(TabAction::Previous));
    app.handle_key(ctrl('k'), &bindings);
    let board_options = app.quick_switcher().expect("quick switcher").options();
    assert!(
        board_options
            .iter()
            .any(|option| option.label == "Reload board")
    );
    assert!(
        !board_options
            .iter()
            .any(|option| option.label == "Reload list")
    );
    let board_shortcuts = board_options
        .iter()
        .map(|option| (option.label.clone(), option.value))
        .collect::<Vec<_>>();
    for (label, action) in list_shortcuts.iter().chain(board_shortcuts.iter()) {
        assert!(
            !bindings.quick_action_shortcut_label(*action).is_empty(),
            "{label} is missing a shortcut",
        );
    }
}

#[test]
fn shift_r_reloads_board_on_board_tab() {
    let bindings = KeyBindings::default();
    let credentials = JiraCredentials {
        site: String::from("https://example.atlassian.net"),
        email: String::from("test@example.com"),
        api_key: String::from("test"),
        default_project: String::from("KAN"),
    };
    let mut app = App::from_credentials(credentials);
    app.take_effects();
    app.dispatch(Action::Tabs(TabAction::Previous));

    app.handle_key(shift('r'), &bindings);

    assert_eq!(app.status(), "Reloading Jira board...");
    let effects = app.take_effects();
    let AppEffect::LoadJiraProject { purpose, .. } = effects.first().expect("reload effect") else {
        panic!("expected Jira reload effect");
    };
    assert_eq!(*purpose, JiraLoadPurpose::ReloadBoard);
}

#[test]
fn r_opens_grouping_on_board_tab() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(support::test_issues(3));
    app.dispatch(Action::Tabs(TabAction::Previous));

    app.handle_key(key('r'), &bindings);

    assert!(app.board_group_dropdown().is_some());
}

#[test]
fn quick_switcher_reload_board_queues_board_reload() {
    let bindings = KeyBindings::default();
    let credentials = JiraCredentials {
        site: String::from("https://example.atlassian.net"),
        email: String::from("test@example.com"),
        api_key: String::from("test"),
        default_project: String::from("KAN"),
    };
    let mut app = App::from_credentials(credentials);
    app.take_effects();
    app.dispatch(Action::Tabs(TabAction::Previous));

    app.handle_key(ctrl('k'), &bindings);
    for c in "Reload board".chars() {
        app.handle_key(key(c), &bindings);
    }
    app.handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
        &bindings,
    );

    assert_eq!(app.status(), "Reloading Jira board...");
    let effects = app.take_effects();
    let AppEffect::LoadJiraProject { purpose, .. } = effects.first().expect("reload effect") else {
        panic!("expected Jira reload effect");
    };
    assert_eq!(*purpose, JiraLoadPurpose::ReloadBoard);
}

#[test]
fn quick_switcher_filter_accepts_text_after_focus() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(Vec::new());

    app.handle_key(ctrl('k'), &bindings);
    assert!(app.is_quick_switcher_filter_focused());
    app.handle_key(key('b'), &bindings);
    app.handle_key(key('o'), &bindings);
    app.handle_key(
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::CONTROL,
        ),
        &bindings,
    );

    assert_eq!(app.active_tab(), "Board");
}

fn board_with_active_sprint() -> BoardData {
    BoardData {
        id: 7,
        name: String::from("DICE Development Scrum Board"),
        columns: vec![BoardColumnSummary {
            name: String::from("To Do"),
            statuses: vec![String::from("To Do")],
            max: None,
        }],
        swimlanes: vec![BoardSwimlaneSummary {
            id: None,
            name: String::from("Issues"),
            issue_keys: vec![String::from("KAN-1")],
        }],
        issues: vec![support::issue("KAN-1", "Sprint task", "Task", None)],
        sprint: Some(SprintSummary {
            name: String::from("DICE Sprint 196"),
            goal: Some(String::from("Deal Makers can publish offer drafts end-to-end.")),
            days_remaining: Some(4),
            start_date: Some(String::from("Jun 3, 2026")),
            end_date: Some(String::from("Jun 17, 2026")),
        }),
    }
}

#[test]
fn details_key_toggles_sprint_details_on_board() {
    let bindings = KeyBindings::default();
    let mut app = App::with_board_data(board_with_active_sprint());
    app.dispatch(Action::Tabs(TabAction::Previous));
    assert_eq!(app.active_tab(), "Board");

    assert!(!app.is_sprint_details_open());
    app.handle_key(key('d'), &bindings);
    assert!(app.is_sprint_details_open());
    app.handle_key(key('d'), &bindings);
    assert!(!app.is_sprint_details_open());
}

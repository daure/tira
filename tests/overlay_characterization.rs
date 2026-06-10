mod support;

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use tira::services::jira::UserSummary;
use tira::{Action, App, KeyBindings};

use support::{key, project, test_issues};

const AREA: Rect = Rect {
    x: 0,
    y: 0,
    width: 100,
    height: 20,
};

fn wheel(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn left_click(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn full_app() -> App {
    App::with_issues_projects_and_users(
        test_issues(20),
        vec![project("KAN", "Kanban"), project("OPS", "Operations")],
        vec![UserSummary {
            account_id: String::from("account-1"),
            display_name: String::from("Marlo Vlietstra"),
        }],
        "KAN",
    )
}

// ---- Per-overlay mouse scroll routing (mirror project-dropdown pattern) ----

#[test]
fn scroll_wheel_over_open_theme_dropdown_leaves_board_unscrolled() {
    let bindings = KeyBindings::default();
    let mut app = full_app();
    app.dispatch(Action::GoToBoard);
    app.dispatch(Action::ToggleThemeDropdown);
    assert!(app.is_theme_dropdown_open());

    app.handle_mouse(wheel(MouseEventKind::ScrollDown, 50, 10), AREA, &bindings);

    assert_eq!(app.board().scroll_offset.get(), 0);
}

#[test]
fn scroll_wheel_over_open_quick_switcher_leaves_board_unscrolled() {
    let bindings = KeyBindings::default();
    let mut app = full_app();
    app.dispatch(Action::GoToBoard);
    app.dispatch(Action::ToggleQuickSwitcher);
    assert!(app.is_quick_switcher_open());

    app.handle_mouse(wheel(MouseEventKind::ScrollDown, 50, 10), AREA, &bindings);

    assert_eq!(app.board().scroll_offset.get(), 0);
}

#[test]
fn scroll_wheel_over_open_assignee_dropdown_leaves_board_unscrolled() {
    let bindings = KeyBindings::default();
    let mut app = full_app();
    app.dispatch(Action::GoToBoard);
    app.dispatch(Action::ToggleAssigneeDropdown);
    assert!(app.is_assignee_dropdown_open());

    app.handle_mouse(wheel(MouseEventKind::ScrollDown, 50, 10), AREA, &bindings);

    assert_eq!(app.board().scroll_offset.get(), 0);
}

// The board-group dropdown is NOT wired into the mouse open-dropdown routing
// (handle_open_dropdown_scroll / handle_open_dropdown_mouse omit it), so a wheel
// over it falls through to the board and scrolls it. Characterizing reality.
#[test]
fn scroll_wheel_over_open_board_group_dropdown_scrolls_board() {
    let bindings = KeyBindings::default();
    let mut app = full_app();
    app.dispatch(Action::GoToBoard);
    app.dispatch(Action::ToggleBoardGrouping);
    assert!(app.is_board_group_dropdown_open());

    app.handle_mouse(wheel(MouseEventKind::ScrollDown, 50, 10), AREA, &bindings);

    assert!(app.board().scroll_offset.get() > 0);
}

// ---- Per-overlay mouse click routing: a click on an option commits/closes ----

#[test]
fn left_click_inside_open_theme_dropdown_commits_and_closes() {
    let bindings = KeyBindings::default();
    let mut app = full_app();
    app.dispatch(Action::GoToBoard);
    app.dispatch(Action::ToggleThemeDropdown);
    assert!(app.is_theme_dropdown_open());

    app.handle_mouse(left_click(50, 10), AREA, &bindings);

    assert!(!app.is_theme_dropdown_open());
}

#[test]
fn left_click_inside_open_quick_switcher_commits_and_closes() {
    let bindings = KeyBindings::default();
    let mut app = full_app();
    app.dispatch(Action::GoToBoard);
    app.dispatch(Action::ToggleQuickSwitcher);
    assert!(app.is_quick_switcher_open());

    app.handle_mouse(left_click(50, 10), AREA, &bindings);

    assert!(!app.is_quick_switcher_open());
}

#[test]
fn left_click_inside_open_assignee_dropdown_commits_and_closes() {
    let bindings = KeyBindings::default();
    let mut app = full_app();
    app.dispatch(Action::GoToBoard);
    app.dispatch(Action::ToggleAssigneeDropdown);
    assert!(app.is_assignee_dropdown_open());

    app.handle_mouse(left_click(50, 10), AREA, &bindings);

    assert!(!app.is_assignee_dropdown_open());
}

// The board-group dropdown is NOT mouse-managed by the centered-dropdown click
// routing, so a click in the board body does not commit/close it. Reality lock.
#[test]
fn left_click_inside_open_board_group_dropdown_does_not_close_it() {
    let bindings = KeyBindings::default();
    let mut app = full_app();
    app.dispatch(Action::GoToBoard);
    app.dispatch(Action::ToggleBoardGrouping);
    assert!(app.is_board_group_dropdown_open());

    app.handle_mouse(left_click(50, 10), AREA, &bindings);

    assert!(app.is_board_group_dropdown_open());
}

// ---- Bare-`q` quit semantics while each overlay is open (filter focused) ----

#[test]
fn bare_q_does_not_quit_while_quick_switcher_open() {
    let bindings = KeyBindings::default();
    let mut app = full_app();
    app.dispatch(Action::ToggleQuickSwitcher);
    assert!(app.is_quick_switcher_filter_focused());

    app.handle_key(key('q'), &bindings);

    assert!(app.is_running());
}

#[test]
fn bare_q_does_not_quit_while_project_dropdown_open() {
    let bindings = KeyBindings::default();
    let mut app = full_app();
    app.dispatch(Action::ToggleProjectDropdown);
    assert!(app.is_project_dropdown_filter_focused());

    app.handle_key(key('q'), &bindings);

    assert!(app.is_running());
}

#[test]
fn bare_q_does_not_quit_while_theme_dropdown_open() {
    let bindings = KeyBindings::default();
    let mut app = full_app();
    app.dispatch(Action::ToggleThemeDropdown);
    assert!(app.is_theme_dropdown_filter_focused());

    app.handle_key(key('q'), &bindings);

    assert!(app.is_running());
}

#[test]
fn bare_q_does_not_quit_while_assignee_dropdown_open() {
    let bindings = KeyBindings::default();
    let mut app = full_app();
    app.dispatch(Action::ToggleAssigneeDropdown);
    assert!(app.is_assignee_dropdown_filter_focused());

    app.handle_key(key('q'), &bindings);

    assert!(app.is_running());
}

#[test]
fn bare_q_does_not_quit_while_board_group_dropdown_open() {
    let bindings = KeyBindings::default();
    let mut app = full_app();
    app.dispatch(Action::ToggleBoardGrouping);
    assert!(app.is_board_group_dropdown_filter_focused());

    app.handle_key(key('q'), &bindings);

    assert!(app.is_running());
}

#[test]
fn bare_q_does_not_quit_while_list_filter_focused() {
    let bindings = KeyBindings::default();
    let mut app = full_app();
    app.handle_key(key('/'), &bindings);
    assert!(app.is_filter_focused());

    app.handle_key(key('q'), &bindings);

    assert!(app.is_running());
}

#[test]
fn bare_q_does_not_quit_while_board_filter_focused() {
    let bindings = KeyBindings::default();
    let mut app = full_app();
    app.dispatch(Action::GoToBoard);
    app.dispatch(Action::FocusBoardFilter);
    assert!(app.is_board_filter_focused());

    app.handle_key(key('q'), &bindings);

    assert!(app.is_running());
}

// ---- Mutual exclusivity: opening overlay B closes overlay A ----

#[test]
fn opening_quick_switcher_closes_theme_dropdown() {
    let mut app = full_app();
    app.dispatch(Action::ToggleThemeDropdown);
    assert!(app.is_theme_dropdown_open());

    app.dispatch(Action::ToggleQuickSwitcher);

    assert!(!app.is_theme_dropdown_open());
    assert!(app.is_quick_switcher_open());
}

#[test]
fn opening_project_dropdown_closes_quick_switcher() {
    let mut app = full_app();
    app.dispatch(Action::ToggleQuickSwitcher);
    assert!(app.is_quick_switcher_open());

    app.dispatch(Action::ToggleProjectDropdown);

    assert!(!app.is_quick_switcher_open());
    assert!(app.is_project_dropdown_open());
}

#[test]
fn opening_board_group_dropdown_closes_theme_dropdown() {
    let mut app = full_app();
    app.dispatch(Action::ToggleThemeDropdown);
    assert!(app.is_theme_dropdown_open());

    app.dispatch(Action::ToggleBoardGrouping);

    assert!(!app.is_theme_dropdown_open());
    assert!(app.is_board_group_dropdown_open());
}

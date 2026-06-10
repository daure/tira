mod support;

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use tira::{Action, App, KeyBindings};

use support::{issue, project, test_issues};

const AREA: Rect = Rect {
    x: 0,
    y: 0,
    width: 100,
    height: 20,
};

fn wheel(kind: MouseEventKind, column: u16, row: u16, modifiers: KeyModifiers) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers,
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

fn board_app() -> App {
    let mut app = App::with_issues(test_issues(20));
    app.dispatch(Action::GoToBoard);
    app
}

#[test]
fn scroll_wheel_over_board_scrolls_rows() {
    let bindings = KeyBindings::default();
    let mut app = board_app();
    assert_eq!(app.board().scroll_offset.get(), 0);

    app.handle_mouse(
        wheel(MouseEventKind::ScrollDown, 10, 6, KeyModifiers::NONE),
        AREA,
        &bindings,
    );

    assert!(app.board().scroll_offset.get() > 0);
}

#[test]
fn shift_scroll_wheel_over_board_pans_columns_not_rows() {
    let bindings = KeyBindings::default();
    let mut app = board_app();

    app.handle_mouse(
        wheel(MouseEventKind::ScrollDown, 10, 6, KeyModifiers::SHIFT),
        AREA,
        &bindings,
    );

    assert!(app.board().manual_h_scroll.get());
    assert_eq!(app.board().scroll_offset.get(), 0);
}

#[test]
fn scroll_wheel_over_open_dropdown_leaves_board_unscrolled() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues_and_projects(
        test_issues(20),
        vec![project("KAN", "Kanban"), project("OPS", "Operations")],
        "KAN",
    );
    app.dispatch(Action::GoToBoard);
    app.dispatch(Action::ToggleProjectDropdown);
    assert!(app.is_project_dropdown_open());

    app.handle_mouse(
        wheel(MouseEventKind::ScrollDown, 50, 10, KeyModifiers::NONE),
        AREA,
        &bindings,
    );

    assert_eq!(app.board().scroll_offset.get(), 0);
}

#[test]
fn left_click_on_status_project_opens_then_keeps_project_dropdown() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues_and_projects(
        vec![issue("KAN-1", "Catalog epic", "Epic", None)],
        vec![project("KAN", "Kanban"), project("OPS", "Operations")],
        "KAN",
    );

    app.handle_mouse(left_click(92, AREA.height - 1), AREA, &bindings);
    assert!(app.is_project_dropdown_open());

    // A second click on the status-bar project indicator keeps it open.
    app.handle_mouse(left_click(92, AREA.height - 1), AREA, &bindings);
    assert!(app.is_project_dropdown_open());
}

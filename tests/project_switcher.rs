mod support;

use ratatui::{Terminal, backend::TestBackend};
use tira::{Action, App, JiraFilteredTreeAction, KeyBindings, draw};

use support::{issue, key, project, rendered_text, shift};

#[test]
fn shift_p_toggles_project_switcher() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues_and_projects(
        vec![issue("KAN-1", "Catalog epic", "Epic", None)],
        vec![project("KAN", "Kanban"), project("OPS", "Operations")],
        "KAN",
    );

    app.handle_key(shift('p'), &bindings);
    assert!(app.is_project_dropdown_open());

    app.handle_key(shift('p'), &bindings);
    assert!(!app.is_project_dropdown_open());
}

#[test]
fn opening_project_switcher_closes_column_picker() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues_and_projects(
        vec![issue("KAN-1", "Catalog epic", "Epic", None)],
        vec![project("KAN", "Kanban"), project("OPS", "Operations")],
        "KAN",
    );

    app.handle_key(key('c'), &bindings);
    assert!(app.is_column_dropdown_open());

    app.handle_key(shift('p'), &bindings);

    assert!(app.is_project_dropdown_open());
    assert!(!app.is_column_dropdown_open());
}

#[test]
fn opening_column_picker_closes_project_switcher() {
    let mut app = App::with_issues_and_projects(
        vec![issue("KAN-1", "Catalog epic", "Epic", None)],
        vec![project("KAN", "Kanban"), project("OPS", "Operations")],
        "KAN",
    );

    app.dispatch(Action::ToggleProjectDropdown);
    assert!(app.is_project_dropdown_open());

    app.dispatch(Action::JiraFilteredTree(
        JiraFilteredTreeAction::OpenColumns,
    ));

    assert!(!app.is_project_dropdown_open());
    assert!(app.is_column_dropdown_open());
}

#[test]
fn project_switcher_search_uses_filter_input_when_focused() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues_and_projects(
        vec![issue("KAN-1", "Catalog epic", "Epic", None)],
        vec![project("KAN", "Kanban"), project("OPS", "Operations")],
        "KAN",
    );

    app.handle_key(shift('p'), &bindings);
    app.handle_key(key('/'), &bindings);
    app.handle_key(key('o'), &bindings);
    app.handle_key(key('p'), &bindings);

    let dropdown = app.project_dropdown().expect("project dropdown");
    assert!(app.is_project_dropdown_filter_focused());
    assert_eq!(dropdown.filter(), "op");
    assert_eq!(dropdown.visible_options()[0].1.value.key, "OPS");
}

#[test]
fn project_switcher_renders_focus_and_checkmark() {
    let backend = TestBackend::new(100, 18);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let bindings = KeyBindings::default();
    let mut app = App::with_issues_and_projects(
        Vec::new(),
        vec![project("KAN", "Kanban"), project("OPS", "Operations")],
        "KAN",
    );

    app.handle_key(shift('p'), &bindings);
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");

    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains("KAN  Kanban"));
    assert!(screen.contains("OPS  Operations"));
    assert!(screen.contains("› KAN  Kanban"));
    assert!(screen.contains("✓"));
}

#[test]
fn project_switcher_filter_focus_shows_insert_mode_in_status_bar() {
    let backend = TestBackend::new(100, 18);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let bindings = KeyBindings::default();
    let mut app = App::with_issues_and_projects(
        Vec::new(),
        vec![project("KAN", "Kanban"), project("OPS", "Operations")],
        "KAN",
    );

    app.handle_key(shift('p'), &bindings);
    app.handle_key(key('/'), &bindings);
    assert!(app.is_project_dropdown_filter_focused());

    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");
    let (_, bottom_row) = rendered_text(&terminal);
    assert!(bottom_row.contains("INSERT"));
}

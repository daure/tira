mod support;

use ratatui::{Terminal, backend::TestBackend};
use tira::{Action, App, KeyBindings, TabAction, draw};

use support::{issue, rendered_text};

#[test]
fn list_render_shows_filtered_tree_as_table_with_columns() {
    let backend = TestBackend::new(160, 12);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let app = App::with_issues(vec![issue(
        "KAN-1",
        "Legacy placeholder task",
        "Task",
        None,
    )]);

    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw app");

    let (screen, _bottom_row) = rendered_text(&terminal);

    assert!(screen.contains("KAN-1"));
    assert!(screen.contains("Legacy placeholder task"));
    assert!(screen.contains("To Do"));
    assert!(screen.contains("Work"));
    assert!(screen.contains("Summary"));
    assert!(screen.contains("Work type"));
    assert!(screen.contains("Status"));
}

#[test]
fn command_log_dialog_renders_for_current_session() {
    let backend = TestBackend::new(160, 18);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut app = App::with_issues(Vec::new());
    app.dispatch(Action::ToggleCommandLog);

    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw app");

    let (screen, bottom_row) = rendered_text(&terminal);
    assert!(screen.contains("Command log"));
    assert!(bottom_row.contains("NORMAL"));
}

#[test]
fn status_bar_uses_configured_help_binding() {
    let backend = TestBackend::new(120, 12);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let app = App::with_issues(Vec::new());
    let bindings = KeyBindings::from_toml_str(
        r##"
        [global]
        open_help = "!"
        "##,
    );

    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");

    let (_, bottom_row) = rendered_text(&terminal);
    assert!(bottom_row.contains("! help"));
}

#[test]
fn command_log_opens_even_while_filter_is_focused() {
    let bindings = tira::KeyBindings::default();
    let mut app = App::with_issues(Vec::new());

    app.handle_key(support::key('/'), &bindings);
    assert!(app.is_filter_focused());

    app.handle_key(support::key('L'), &bindings);

    assert!(app.is_command_log_open());
}

#[test]
fn opening_command_log_closes_dropdowns() {
    let bindings = tira::KeyBindings::default();
    let mut app = App::with_issues_and_projects(
        vec![issue("KAN-1", "Catalog epic", "Epic", None)],
        vec![support::project("KAN", "Kanban")],
        "KAN",
    );

    app.handle_key(support::key('c'), &bindings);
    assert!(app.is_column_dropdown_open());
    app.dispatch(Action::ToggleCommandLog);
    assert!(app.is_command_log_open());
    assert!(!app.is_column_dropdown_open());

    app.dispatch(Action::ToggleCommandLog);
    app.handle_key(support::shift('p'), &bindings);
    assert!(app.is_project_dropdown_open());
    app.dispatch(Action::ToggleCommandLog);
    assert!(app.is_command_log_open());
    assert!(!app.is_project_dropdown_open());
}

#[test]
fn filter_render_uses_previous_search_icon_without_colon() {
    let backend = TestBackend::new(80, 12);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let app = App::with_issues(Vec::new());

    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw app");

    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains(" Search"));
}

#[test]
fn board_tab_stays_empty_for_now() {
    let backend = TestBackend::new(80, 12);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut app = App::with_issues(vec![issue(
        "KAN-1",
        "Legacy placeholder task",
        "Task",
        None,
    )]);
    app.dispatch(Action::Tabs(TabAction::Previous));

    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw app");

    let (screen, _) = rendered_text(&terminal);

    assert!(screen.contains("Board"));
    assert!(!screen.contains("KAN-1"));
}

#[test]
fn list_render_truncates_long_description_with_ellipsis() {
    let backend = TestBackend::new(40, 10);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let app = App::with_issues(vec![issue(
        "KAN-1",
        "A extremely long description that should definitely be truncated",
        "Task",
        None,
    )]);

    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw app");
    let (screen, _bottom_row) = rendered_text(&terminal);

    assert!(screen.contains("..."));
    assert!(!screen.contains("A extremely long description that should definitely be truncated"));
}

#[test]
fn code_column_header_spacing_is_conditional() {
    let backend = TestBackend::new(160, 10);
    let mut terminal = Terminal::new(backend).expect("test terminal");

    let app_no_exp = App::with_issues(vec![issue("KAN-1", "Task 1", "Task", None)]);
    terminal
        .draw(|frame| draw(frame, &app_no_exp, &KeyBindings::default()))
        .expect("draw");
    let (screen_no_exp, _) = rendered_text(&terminal);
    let chars_no_exp: Vec<char> = screen_no_exp.chars().collect();
    let line_2_no_exp: String = chars_no_exp[320..480].iter().collect();
    assert!(line_2_no_exp.contains("Work "));

    let app_has_exp = App::with_issues(vec![
        issue("KAN-2", "Epic 1", "Epic", None),
        issue("KAN-3", "Story 1", "Story", Some("KAN-2")),
    ]);
    terminal
        .draw(|frame| draw(frame, &app_has_exp, &KeyBindings::default()))
        .expect("draw");
    let (screen_has_exp, _) = rendered_text(&terminal);
    let chars_has_exp: Vec<char> = screen_has_exp.chars().collect();
    let line_2_has_exp: String = chars_has_exp[320..480].iter().collect();
    assert!(line_2_has_exp.contains("  Work "));
}

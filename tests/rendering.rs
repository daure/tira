mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend};
use tira::{
    Action, App, AppEffect, AppEvent, JiraLoadPurpose, JiraProjectLoadResult, KeyBindings,
    TabAction,
    components::jira::work_item_key,
    config::JiraCredentials,
    draw,
    services::jira::{
        BoardColumnSummary, BoardData, BoardSwimlaneSummary, CommandLogEntry, FieldSummary,
        JiraError, SprintSummary, UserSummary,
    },
    ui::theme::{Theme, ThemeName},
};

use support::{ctrl, issue, key, rendered_text};

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
    assert!(screen.contains("Status"));
    assert!(screen.contains("Labels"));
    assert!(!screen.contains("Work type"));
}

#[test]
fn narrow_table_scrolls_horizontally_with_animation() {
    // A terminal too narrow for the columns forces the scrollable strip path.
    let backend = TestBackend::new(40, 8);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut app = App::with_issues(vec![issue(
        "KAN-1",
        "A deliberately very long summary that overflows the narrow viewport width",
        "Task",
        None,
    )]);

    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw app");
    let (before, _) = rendered_text(&terminal);
    assert!(
        before.contains("KAN-1") && before.contains("deliberately"),
        "the work key and summary start are visible before scrolling"
    );

    // Pan to the far right and let the glide settle, then redraw.
    app.scroll_table_horizontal(1000);
    app.tick(std::time::Duration::from_secs(2));
    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw app");
    let (after, _) = rendered_text(&terminal);

    assert!(
        !after.contains("deliberately"),
        "scrolling right pans the summary out of view"
    );
    assert!(
        after.contains("KAN-1"),
        "the work column stays pinned while the rest scrolls"
    );
}

#[test]
fn shift_h_and_l_pan_the_table_horizontally() {
    let backend = TestBackend::new(40, 8);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(vec![issue(
        "KAN-1",
        "A deliberately very long summary that overflows the narrow viewport width",
        "Task",
        None,
    )]);

    let shift = |c: char| KeyEvent::new(KeyCode::Char(c), KeyModifiers::SHIFT);

    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");
    assert!(rendered_text(&terminal).0.contains("deliberately"));

    // Shift+L pans right until the summary scrolls off (Work stays pinned).
    for _ in 0..20 {
        app.handle_key(shift('L'), &bindings);
    }
    app.tick(std::time::Duration::from_secs(2));
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");
    let scrolled = rendered_text(&terminal).0;
    assert!(!scrolled.contains("deliberately"), "Shift+L pans right");
    assert!(scrolled.contains("KAN-1"), "the work column stays pinned");

    // Shift+H pans back left to the start.
    for _ in 0..20 {
        app.handle_key(shift('H'), &bindings);
    }
    app.tick(std::time::Duration::from_secs(2));
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");
    assert!(
        rendered_text(&terminal).0.contains("deliberately"),
        "Shift+H pans the table back left"
    );
}

#[test]
fn quick_actions_menu_uses_current_name() {
    let backend = TestBackend::new(100, 12);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut app = App::with_issues(Vec::new());
    let bindings = KeyBindings::default();
    app.handle_key(ctrl('k'), &bindings);

    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");

    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains("Quick actions"));
    assert!(!screen.contains("Quick switcher"));
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

/// Pushes a command-log entry with the given long path by replaying an
/// assignment result (no pending request, so it only records the log line).
fn push_log_entry(app: &mut App, path: &str) {
    app.handle_event(AppEvent::IssueAssigned {
        request_id: 999,
        issue_key: String::from("KAN-1"),
        assignee: None,
        result: Ok(CommandLogEntry {
            timestamp: String::from("10:00:00"),
            method: "GET",
            path: path.to_owned(),
            status: String::from("200"),
            duration_ms: 5,
        }),
    });
}

#[test]
fn command_log_wraps_paths_and_opens_scrolled_to_latest() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(Vec::new());

    // More long-path entries than the viewport can show, so opening must scroll
    // to the bottom (latest) and the long paths must wrap to stay fully visible.
    for letter in 'a'..='o' {
        push_log_entry(
            &mut app,
            &format!(
                "/rest/api/2/issue/ZONE{letter}/segment/one/two/three/four/five/six/tail-{letter}"
            ),
        );
    }
    app.dispatch(Action::ToggleCommandLog);

    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");
    let (screen, _) = rendered_text(&terminal);

    // The latest entry is shown in full (its wrapped tail is visible), proving
    // both the scroll-to-bottom on open and the cell wrapping of the path.
    assert!(
        screen.contains("ZONEo"),
        "latest entry visible at the bottom"
    );
    assert!(
        screen.contains("tail-o"),
        "long path wraps to show its tail"
    );
    // The earliest entry has scrolled off the top.
    assert!(
        !screen.contains("ZONEa"),
        "oldest entry scrolled out of view"
    );

    // Jumping to the top reveals the earliest entry.
    app.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE), &bindings);
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");
    let (screen, _) = rendered_text(&terminal);
    assert!(
        screen.contains("ZONEa"),
        "scrolling to the top shows the oldest entry"
    );
}

#[test]
fn command_log_keyboard_navigation_jumps_and_pages() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(Vec::new());
    for letter in 'a'..='o' {
        push_log_entry(
            &mut app,
            &format!(
                "/rest/api/2/issue/ZONE{letter}/segment/one/two/three/four/five/six/tail-{letter}"
            ),
        );
    }
    app.dispatch(Action::ToggleCommandLog);

    // `gg` jumps to the top: the first `g` arms the prefix, the second jumps.
    app.handle_key(key('g'), &bindings);
    app.handle_key(key('g'), &bindings);
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");
    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains("ZONEa"), "gg jumps to the oldest entry");
    assert!(!screen.contains("ZONEo"), "latest is scrolled off after gg");

    // A lone `g` must not jump; the following PageDown still scrolls down.
    app.handle_key(key('g'), &bindings);
    app.handle_key(
        KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
        &bindings,
    );
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");
    let (screen, _) = rendered_text(&terminal);
    assert!(
        !screen.contains("ZONEa"),
        "PageDown after a lone g scrolls down instead of jumping to the top"
    );

    // `G` jumps back to the bottom (latest entry).
    app.handle_key(
        KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT),
        &bindings,
    );
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");
    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains("ZONEo"), "G jumps back to the latest entry");
}

#[test]
fn command_log_highlights_path_but_not_query_string() {
    let backend = TestBackend::new(120, 10);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(Vec::new());
    push_log_entry(&mut app, "/agile/board?projectKeyOrId=DPP");
    app.dispatch(Action::ToggleCommandLog);
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");

    let highlight = app.theme().selected_bg();
    let buffer = terminal.backend().buffer();
    let width = buffer.area().width as usize;
    let height = buffer.area().height as usize;
    let content = buffer.content();
    let row = (0..height)
        .find(|&y| {
            let line: String = content[y * width..(y + 1) * width]
                .iter()
                .map(|cell| cell.symbol())
                .collect();
            line.contains("board?projectKeyOrId")
        })
        .expect("row with the path");
    let line: String = content[row * width..(row + 1) * width]
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    let path_col = line.find("board").expect("path segment");
    let query_col = line.find("projectKeyOrId").expect("query segment");

    assert_eq!(
        content[row * width + path_col].bg,
        highlight,
        "the path is background-highlighted"
    );
    assert_ne!(
        content[row * width + query_col].bg,
        highlight,
        "the query string is not highlighted"
    );
}

#[test]
fn status_bar_uses_configured_help_binding() {
    let backend = TestBackend::new(160, 12);
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
fn status_bar_omits_active_tab_name() {
    let backend = TestBackend::new(160, 12);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let app = App::with_issues(vec![issue("KAN-1", "Work", "Task", None)]);
    let bindings = KeyBindings::default();

    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");

    let (_, bottom_row) = rendered_text(&terminal);
    assert!(bottom_row.contains("NORMAL"));
    // The footer no longer prefixes the status with the active tab name.
    assert!(!bottom_row.contains("List ·"));
}

#[test]
fn help_dialog_renders_local_and_global_sections() {
    let backend = TestBackend::new(160, 20);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut app = App::with_issues(Vec::new());
    let bindings = KeyBindings::default();
    app.dispatch(Action::OpenHelp);

    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");

    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains("Local"));
    assert!(screen.contains("Columns"));
    assert!(screen.contains("├"));
    assert!(screen.contains("┤"));

    // Press End to scroll to the bottom
    app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), &bindings);

    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");

    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains("Global"));
    assert!(screen.contains("Close help"));
    assert!(screen.contains("Leader key"));
}

#[test]
fn column_dropdown_separator_connects_to_border() {
    let backend = TestBackend::new(120, 20);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let credentials = JiraCredentials {
        site: String::from("https://example.atlassian.net"),
        email: String::from("test@example.com"),
        api_key: String::from("test"),
        default_project: String::from("KAN"),
    };
    let mut app = App::from_credentials(credentials.clone());
    let AppEffect::LoadJiraProject { request_id, .. } = app.take_effects().remove(0) else {
        panic!("expected Jira load effect");
    };
    app.handle_event(AppEvent::JiraProjectLoaded {
        request_id,
        purpose: JiraLoadPurpose::Initial,
        credentials,
        result: JiraProjectLoadResult {
            next_page_token: None,
            issues: Ok(vec![issue(
                "KAN-1",
                "Legacy placeholder task",
                "Task",
                None,
            )]),
            board: Err(JiraError(String::from("board unavailable"))),
            fields: Ok((0..20)
                .map(|index| FieldSummary {
                    id: format!("customfield_{index}"),
                    name: format!("Field {index}"),
                })
                .collect()),
            projects: Ok(Vec::new()),
            users: Ok(Vec::new()),
            current_user: Err(tira::services::jira::JiraError(String::new())),
            logs: Vec::new(),
        },
    });

    app.handle_key(key('c'), &KeyBindings::default());
    app.handle_key(key(' '), &KeyBindings::default());

    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw app");

    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains("█"));
}

#[test]
fn duplicate_field_labels_append_field_id_to_differentiate() {
    let backend = TestBackend::new(120, 20);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let credentials = JiraCredentials {
        site: String::from("https://example.atlassian.net"),
        email: String::from("test@example.com"),
        api_key: String::from("test"),
        default_project: String::from("KAN"),
    };
    let mut app = App::from_credentials(credentials.clone());
    let AppEffect::LoadJiraProject { request_id, .. } = app.take_effects().remove(0) else {
        panic!("expected Jira load effect");
    };
    app.handle_event(AppEvent::JiraProjectLoaded {
        request_id,
        purpose: JiraLoadPurpose::Initial,
        credentials,
        result: JiraProjectLoadResult {
            issues: Ok(Vec::new()),
            board: Err(JiraError(String::from("board unavailable"))),
            next_page_token: None,
            fields: Ok(vec![
                FieldSummary {
                    id: String::from("project"),
                    name: String::from("Project"),
                },
                FieldSummary {
                    id: String::from("customfield_10001"),
                    name: String::from("Project"),
                },
            ]),
            projects: Ok(Vec::new()),
            users: Ok(Vec::new()),
            current_user: Err(tira::services::jira::JiraError(String::new())),
            logs: Vec::new(),
        },
    });

    app.handle_key(key('c'), &KeyBindings::default());
    app.handle_key(key(' '), &KeyBindings::default());

    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw app");

    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains("Project (project)"));
    assert!(screen.contains("Project (customfield_10001)"));
}

#[test]
fn priority_and_assignee_fields_render_with_components() {
    let backend = TestBackend::new(160, 16);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let credentials = JiraCredentials {
        site: String::from("https://example.atlassian.net"),
        email: String::from("test@example.com"),
        api_key: String::from("test"),
        default_project: String::from("KAN"),
    };
    let mut issue = issue("KAN-1", "Legacy placeholder task", "Task", None);
    issue
        .field_values
        .insert(String::from("priority"), String::from("Highest"));
    issue.field_values.insert(
        String::from("assignee"),
        String::from("Johan van der Brink"),
    );
    issue
        .field_values
        .insert(String::from("labels"), String::from("frontend, urgent"));
    let mut app = App::from_credentials(credentials.clone());
    let AppEffect::LoadJiraProject { request_id, .. } = app.take_effects().remove(0) else {
        panic!("expected Jira load effect");
    };
    app.handle_event(AppEvent::JiraProjectLoaded {
        request_id,
        purpose: JiraLoadPurpose::Initial,
        credentials,
        result: JiraProjectLoadResult {
            issues: Ok(vec![issue]),
            board: Err(JiraError(String::from("board unavailable"))),
            next_page_token: None,
            fields: Ok(vec![
                FieldSummary {
                    id: String::from("priority"),
                    name: String::from("Priority"),
                },
                FieldSummary {
                    id: String::from("assignee"),
                    name: String::from("Assignee"),
                },
                FieldSummary {
                    id: String::from("labels"),
                    name: String::from("Labels"),
                },
            ]),
            projects: Ok(Vec::new()),
            users: Ok(Vec::new()),
            current_user: Err(tira::services::jira::JiraError(String::new())),
            logs: Vec::new(),
        },
    });

    let bindings = KeyBindings::default();
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");

    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains("󰄿"));
    assert!(screen.contains("@JB"));
    assert!(screen.contains("⟦frontend⟧⟦urgent⟧"));
    assert!(!screen.contains("Highest"));
}

#[test]
fn command_log_opens_even_while_filter_is_focused() {
    let bindings = tira::KeyBindings::default();
    let mut app = App::with_issues(Vec::new());

    app.handle_key(support::key('/'), &bindings);
    assert!(app.is_filter_focused());

    app.handle_key(support::ctrl('x'), &bindings);
    app.handle_key(support::key('c'), &bindings);

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
    app.handle_key(support::ctrl('x'), &bindings);
    app.handle_key(support::key('p'), &bindings);
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
fn board_tab_renders_jira_cards() {
    let backend = TestBackend::new(80, 18);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut issue = issue("KAN-1", "Legacy placeholder task", "Task", None);
    issue.field_values.insert(
        String::from("epic_summary"),
        String::from("[Shopping Cart] Checkout"),
    );
    issue
        .field_values
        .insert(String::from("labels"), String::from("BE, FE"));
    issue
        .field_values
        .insert(String::from("dueDate"), String::from("2026-06-10"));
    issue
        .field_values
        .insert(String::from("priorityName"), String::from("Lowest"));
    issue
        .field_values
        .insert(String::from("assignee"), String::from("Marlo Vlietstra"));
    let mut app = App::with_issues(vec![issue]);
    app.dispatch(Action::Tabs(TabAction::Previous));

    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw app");

    let (screen, _) = rendered_text(&terminal);

    assert!(screen.contains("TO DO"));
    assert!(screen.contains(&format!("{} KAN-1", work_item_key::icon("Task"))));
    assert!(screen.contains("KAN-1"));
    assert!(!screen.contains("Issues"));
    assert!(screen.contains("╔"));
    assert!(screen.contains("╚"));
    assert!(
        screen.contains(" [Shopping Cart] Checkout")
            || screen.contains("⚡ [Shopping Cart] Checkout")
    );
    assert!(screen.contains("⟦BE⟧⟦FE⟧"));
    assert!(screen.contains("2026-06-10"));
    assert!(screen.contains("@MV"));
    assert!(!screen.contains("@UN"));
}

#[test]
fn board_tab_search_filters_visible_cards() {
    let backend = TestBackend::new(100, 18);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut cart = issue("KAN-1", "Cart search result", "Story", None);
    cart.status = String::from("To Do");
    let mut profile = issue("KAN-2", "Profile settings", "Story", None);
    profile.status = String::from("To Do");
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
        issues: vec![cart, profile],
        sprint: None,
    };
    let bindings = KeyBindings::default();
    let mut app = App::with_board_data(board);
    app.handle_key(key('/'), &bindings);
    for c in "profile".chars() {
        app.handle_key(key(c), &bindings);
    }
    app.dispatch(Action::Tabs(TabAction::Previous));

    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");
    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains(" Search"));
    assert!(screen.contains("KAN-1"));
    assert!(screen.contains("KAN-2"));

    app.handle_key(key('/'), &bindings);
    for c in "cart".chars() {
        app.handle_key(key(c), &bindings);
    }

    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");

    let (screen, _) = rendered_text(&terminal);
    assert!(
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|cell| cell.bg == app.theme().highlight_bg())
    );
    assert!(screen.contains(" cart"));
    assert!(screen.contains("KAN-1"));
    assert!(!screen.contains("KAN-2"));
}

#[test]
fn board_tab_date_search_filters_visible_cards() {
    let backend = TestBackend::new(100, 18);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut dated = issue("KAN-1", "Dated card", "Story", None);
    dated.status = String::from("To Do");
    dated
        .field_values
        .insert(String::from("dueDate"), String::from("2026-06-10"));
    let mut undated = issue("KAN-2", "Undated card", "Story", None);
    undated.status = String::from("To Do");
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(vec![dated, undated]);
    app.dispatch(Action::Tabs(TabAction::Previous));
    app.handle_key(key('/'), &bindings);
    for c in "2026".chars() {
        app.handle_key(key(c), &bindings);
    }

    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");

    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains("KAN-1"));
    assert!(!screen.contains("KAN-2"));
    assert!(
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|cell| cell.bg == app.theme().highlight_bg())
    );
}

#[test]
fn board_card_footer_highlights_key_and_avatar_matches() {
    let backend = TestBackend::new(80, 18);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut issue = issue("KAN-1", "Footer highlight", "Story", None);
    issue.status = String::from("To Do");
    issue
        .field_values
        .insert(String::from("assignee"), String::from("Marlo Vlietstra"));
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(vec![issue]);
    app.dispatch(Action::Tabs(TabAction::Previous));
    app.handle_key(key('/'), &bindings);
    for c in "mv".chars() {
        app.handle_key(key(c), &bindings);
    }

    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");

    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains("@MV"));
    assert!(
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|cell| cell.bg == app.theme().highlight_bg())
    );
}

#[test]
fn board_card_footer_highlights_avatar_bubble_for_at_prefixed_search() {
    let backend = TestBackend::new(80, 18);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut issue = issue("KAN-1", "Footer highlight", "Story", None);
    issue.status = String::from("To Do");
    issue
        .field_values
        .insert(String::from("assignee"), String::from("Marlo Vlietstra"));
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(vec![issue]);
    app.dispatch(Action::Tabs(TabAction::Previous));
    app.handle_key(key('/'), &bindings);
    // The avatar renders as "@MV"; searching the visible "@MV" text should
    // highlight the bubble just like searching the bare "MV" initials does.
    for c in "@mv".chars() {
        app.handle_key(key(c), &bindings);
    }

    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");

    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains("@MV"));
    // The "@MV" bubble cells themselves must carry the highlight background.
    let buffer = terminal.backend().buffer();
    let area = *buffer.area();
    let highlight_bg = app.theme().highlight_bg();
    let mut bubble_highlighted = false;
    for y in 0..area.height {
        for x in 0..area.width.saturating_sub(2) {
            if buffer[(x, y)].symbol() == "@"
                && buffer[(x + 1, y)].symbol() == "M"
                && buffer[(x + 2, y)].symbol() == "V"
            {
                bubble_highlighted = buffer[(x, y)].bg == highlight_bg
                    && buffer[(x + 1, y)].bg == highlight_bg
                    && buffer[(x + 2, y)].bg == highlight_bg;
            }
        }
    }
    assert!(
        bubble_highlighted,
        "the @MV avatar bubble should be highlighted for an '@mv' search"
    );
}

#[test]
fn board_grouping_by_assignee_shows_assignee_swimlanes() {
    let backend = TestBackend::new(100, 18);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut assigned = issue("KAN-1", "Assigned card", "Story", None);
    assigned.status = String::from("To Do");
    assigned
        .field_values
        .insert(String::from("assignee"), String::from("Marlo Vlietstra"));
    let mut unassigned = issue("KAN-2", "Unassigned card", "Story", None);
    unassigned.status = String::from("To Do");
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(vec![assigned, unassigned]);
    app.dispatch(Action::Tabs(TabAction::Previous));
    app.handle_key(key('r'), &bindings);
    app.handle_key(ctrl('j'), &bindings);
    app.handle_key(
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::CONTROL,
        ),
        &bindings,
    );

    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");

    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains("group: Assignee"));
    assert!(screen.contains("Marlo Vlietstra"));
    assert!(screen.contains("Unassigned"));
}

#[test]
fn board_cards_use_display_name_avatar_from_issue_search() {
    let backend = TestBackend::new(80, 18);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let credentials = JiraCredentials {
        site: String::from("https://example.atlassian.net"),
        email: String::from("test@example.com"),
        api_key: String::from("test"),
        default_project: String::from("KAN"),
    };
    let mut list_issue = issue("KAN-2", "Assigned list issue", "Story", None);
    list_issue
        .field_values
        .insert(String::from("assignee"), String::from("Marlo Vlietstra"));
    let mut board_issue = issue("KAN-2", "Assigned board issue", "Story", None);
    board_issue
        .field_values
        .insert(String::from("assignee"), String::from("76"));
    let mut app = App::from_credentials(credentials.clone());
    let AppEffect::LoadJiraProject { request_id, .. } = app.take_effects().remove(0) else {
        panic!("expected Jira load effect");
    };
    app.handle_event(AppEvent::JiraProjectLoaded {
        request_id,
        purpose: JiraLoadPurpose::Initial,
        credentials,
        result: JiraProjectLoadResult {
            issues: Ok(vec![list_issue]),
            board: Ok(BoardData {
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
                    issue_keys: vec![String::from("KAN-2")],
                }],
                issues: vec![board_issue],
                sprint: None,
            }),
            next_page_token: None,
            fields: Ok(Vec::new()),
            projects: Ok(Vec::new()),
            users: Ok(Vec::new()),
            current_user: Err(JiraError(String::new())),
            logs: Vec::new(),
        },
    });
    app.dispatch(Action::Tabs(TabAction::Previous));

    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw app");

    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains("@MV"));
    assert!(!screen.contains("@76"));
}

#[test]
fn board_grouping_resolves_assignee_account_ids_from_users() {
    let backend = TestBackend::new(100, 18);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let credentials = JiraCredentials {
        site: String::from("https://example.atlassian.net"),
        email: String::from("test@example.com"),
        api_key: String::from("test"),
        default_project: String::from("KAN"),
    };
    let mut board_issue = issue("KAN-2", "Assigned board issue", "Story", None);
    board_issue.status = String::from("To Do");
    board_issue
        .field_values
        .insert(String::from("assignee"), String::from("7616e38d"));
    let mut app = App::from_credentials(credentials.clone());
    let AppEffect::LoadJiraProject { request_id, .. } = app.take_effects().remove(0) else {
        panic!("expected Jira load effect");
    };
    app.handle_event(AppEvent::JiraProjectLoaded {
        request_id,
        purpose: JiraLoadPurpose::Initial,
        credentials,
        result: JiraProjectLoadResult {
            issues: Ok(Vec::new()),
            board: Ok(BoardData {
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
                    issue_keys: vec![String::from("KAN-2")],
                }],
                issues: vec![board_issue],
                sprint: None,
            }),
            next_page_token: None,
            fields: Ok(Vec::new()),
            projects: Ok(Vec::new()),
            users: Ok(vec![UserSummary {
                account_id: String::from("7616e38d"),
                display_name: String::from("Marlo Vlietstra"),
            }]),
            current_user: Err(JiraError(String::new())),
            logs: Vec::new(),
        },
    });
    app.dispatch(Action::Tabs(TabAction::Previous));
    let bindings = KeyBindings::default();
    app.handle_key(key('r'), &bindings);
    app.handle_key(ctrl('j'), &bindings);
    app.handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
        &bindings,
    );

    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");

    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains("Marlo Vlietstra"));
    assert!(!screen.contains("7616e38d"));
}

#[test]
fn board_tab_renders_swimlanes_and_uses_theme_selection_style() {
    let backend = TestBackend::new(100, 16);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut todo = issue("KAN-1", "Browse catalog", "Story", None);
    todo.status = String::from("To Do");
    todo.field_values
        .insert(String::from("status_id"), String::from("100"));
    let mut done = issue("KAN-2", "Checkout", "Task", None);
    done.status = String::from("Done");
    done.field_values
        .insert(String::from("status_id"), String::from("300"));
    let board = BoardData {
        id: 7,
        name: String::from("Kanban"),
        columns: vec![
            BoardColumnSummary {
                name: String::from("To Do"),
                statuses: vec![String::from("100")],
                max: None,
            },
            BoardColumnSummary {
                name: String::from("Done"),
                statuses: vec![String::from("300")],
                max: None,
            },
        ],
        swimlanes: vec![
            BoardSwimlaneSummary {
                id: Some(String::from("11")),
                name: String::from("Shopping cart"),
                issue_keys: vec![String::from("KAN-1")],
            },
            BoardSwimlaneSummary {
                id: Some(String::from("12")),
                name: String::from("Payments"),
                issue_keys: vec![String::from("KAN-2")],
            },
        ],
        issues: vec![todo, done],
        sprint: None,
    };
    let theme = Theme::named(ThemeName::Catppuccin);
    let selected_bg = theme.selected_bg();
    let mut app = App::with_board_data(board);
    app.set_theme(theme);
    app.dispatch(Action::Tabs(TabAction::Previous));

    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw app");

    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains("Shopping cart"));
    assert!(screen.contains("Payments"));
    assert!(!screen.contains("@UN"));
    assert!(
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|cell| cell.bg == selected_bg)
    );
}

#[test]
fn board_scroll_keeps_swimlane_context_when_returning_to_edges() {
    let backend = TestBackend::new(100, 9);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut issues = Vec::new();
    let mut first_lane_keys = Vec::new();
    for index in 1..=6 {
        let key = format!("KAN-{index}");
        let mut item = issue(&key, &format!("Top lane issue {index}"), "Task", None);
        item.status = String::from("To Do");
        first_lane_keys.push(key);
        issues.push(item);
    }
    let mut bottom = issue("KAN-7", "Bottom lane issue", "Task", None);
    bottom.status = String::from("To Do");
    issues.push(bottom);
    let board = BoardData {
        id: 7,
        name: String::from("Kanban"),
        columns: vec![BoardColumnSummary {
            name: String::from("To Do"),
            statuses: vec![String::from("To Do")],
            max: None,
        }],
        swimlanes: vec![
            BoardSwimlaneSummary {
                id: None,
                name: String::from("Top swimlane"),
                issue_keys: first_lane_keys,
            },
            BoardSwimlaneSummary {
                id: None,
                name: String::from("Bottom swimlane"),
                issue_keys: vec![String::from("KAN-7")],
            },
        ],
        issues,
        sprint: None,
    };
    let bindings = KeyBindings::default();
    let mut app = App::with_board_data(board);
    app.dispatch(Action::Tabs(TabAction::Previous));

    for _ in 0..4 {
        app.handle_key(key('j'), &bindings);
    }
    for _ in 0..4 {
        app.handle_key(key('k'), &bindings);
    }
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");
    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains("Top swimlane"));
    assert!(screen.contains("TO DO"));

    app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), &bindings);
    // First draw sets the scroll target; tick lets the glide settle, then draw
    // the settled frame to assert on.
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");
    app.tick(std::time::Duration::from_secs(1));
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");
    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains("Bottom swimlane"));
    assert!(screen.contains("╚") || screen.contains("└"));
}

#[test]
fn grouped_board_heading_sticks_while_scrolling_cards() {
    let backend = TestBackend::new(100, 9);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut issues = Vec::new();
    let mut top_lane_keys = Vec::new();
    for index in 1..=7 {
        let key = format!("KAN-{index}");
        let mut item = issue(&key, &format!("Assigned issue {index}"), "Task", None);
        item.status = String::from("To Do");
        item.field_values
            .insert(String::from("assignee"), String::from("Marlo Vlietstra"));
        top_lane_keys.push(key);
        issues.push(item);
    }
    let mut bottom = issue("KAN-8", "Bottom swimlane issue", "Task", None);
    bottom.status = String::from("To Do");
    bottom
        .field_values
        .insert(String::from("assignee"), String::from("Marlo Vlietstra"));
    issues.push(bottom);
    let board = BoardData {
        id: 7,
        name: String::from("Kanban"),
        columns: vec![BoardColumnSummary {
            name: String::from("To Do"),
            statuses: vec![String::from("To Do")],
            max: None,
        }],
        swimlanes: vec![
            BoardSwimlaneSummary {
                id: None,
                name: String::from("Top swimlane"),
                issue_keys: top_lane_keys,
            },
            BoardSwimlaneSummary {
                id: None,
                name: String::from("Bottom swimlane"),
                issue_keys: vec![String::from("KAN-8")],
            },
        ],
        issues,
        sprint: None,
    };
    let bindings = KeyBindings::default();
    let mut app = App::with_board_data(board);
    app.dispatch(Action::Tabs(TabAction::Previous));
    app.handle_key(key('r'), &bindings);
    app.handle_key(ctrl('j'), &bindings);
    app.handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
        &bindings,
    );

    for _ in 0..5 {
        app.handle_key(key('j'), &bindings);
    }
    // Draw to set the scroll target, settle the glide, then draw the result.
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");
    app.tick(std::time::Duration::from_secs(1));
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");

    let (screen, _) = rendered_text(&terminal);
    assert_eq!(app.selected_board_issue_key(), Some("KAN-5"));
    assert!(screen.contains("Top swimlane"));
    assert!(screen.contains("Marlo Vlietstra"));
    assert!(screen.contains("TO DO"));
    assert!(screen.contains("KAN-5"));
}

#[test]
fn board_cards_render_without_blank_gaps() {
    let backend = TestBackend::new(80, 14);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut first = issue("KAN-1", "First card", "Task", None);
    first.status = String::from("To Do");
    let mut second = issue("KAN-2", "Second card", "Task", None);
    second.status = String::from("To Do");
    let board = BoardData {
        id: 7,
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
        issues: vec![first, second],
        sprint: None,
    };
    let app = App::with_board_data(board);

    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw app");

    let buffer = terminal.backend().buffer();
    let rows = buffer
        .content()
        .chunks(buffer.area().width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>();
    let first_key_row = rows
        .iter()
        .position(|row| row.contains("KAN-1"))
        .expect("first card key row");
    assert!(
        rows.get(first_key_row + 1)
            .is_some_and(|row| row.contains("Second card"))
    );
}

#[test]
fn board_tab_shows_visible_fallback_when_board_load_fails() {
    let backend = TestBackend::new(100, 12);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let credentials = JiraCredentials {
        site: String::from("https://example.atlassian.net"),
        email: String::from("test@example.com"),
        api_key: String::from("test"),
        default_project: String::from("KAN"),
    };
    let mut app = App::from_credentials(credentials.clone());
    let AppEffect::LoadJiraProject { request_id, .. } = app.take_effects().remove(0) else {
        panic!("expected Jira load effect");
    };
    app.handle_event(AppEvent::JiraProjectLoaded {
        request_id,
        purpose: JiraLoadPurpose::Initial,
        credentials,
        result: JiraProjectLoadResult {
            issues: Ok(vec![issue("KAN-1", "Fallback task", "Task", None)]),
            board: Err(JiraError(String::from("board unavailable"))),
            next_page_token: None,
            fields: Ok(Vec::new()),
            projects: Ok(Vec::new()),
            users: Ok(Vec::new()),
            current_user: Err(JiraError(String::new())),
            logs: Vec::new(),
        },
    });
    app.dispatch(Action::Tabs(TabAction::Previous));

    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw app");

    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains("Board endpoint failed"));
    assert!(screen.contains("KAN-1"));
}

#[test]
fn board_help_overlay_shows_board_keybindings() {
    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let mut app = App::with_issues(vec![issue("KAN-1", "Help task", "Task", None)]);
    let bindings = KeyBindings::default();
    app.dispatch(Action::Tabs(TabAction::Previous));
    app.handle_key(key('?'), &bindings);

    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");

    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains("Search board"));
    assert!(screen.contains("Sprint details"));
    assert!(screen.contains("Move columns"));
    assert!(screen.contains("Move cards"));
    assert!(screen.contains("Page cards"));
    assert!(screen.contains("Start / End"));
    assert!(screen.contains("Reload board"));
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
    let buffer_no_exp = terminal.backend().buffer();
    let rows_no_exp = buffer_no_exp
        .content()
        .chunks(buffer_no_exp.area().width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>();
    let header_no_exp = rows_no_exp
        .iter()
        .find(|row| row.contains("Work"))
        .expect("work column header row");
    assert!(header_no_exp.contains("Work "));

    let app_has_exp = App::with_issues(vec![
        issue("KAN-2", "Epic 1", "Epic", None),
        issue("KAN-3", "Story 1", "Story", Some("KAN-2")),
    ]);
    terminal
        .draw(|frame| draw(frame, &app_has_exp, &KeyBindings::default()))
        .expect("draw");
    let buffer_has_exp = terminal.backend().buffer();
    let rows_has_exp = buffer_has_exp
        .content()
        .chunks(buffer_has_exp.area().width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>();
    let header_has_exp = rows_has_exp
        .iter()
        .find(|row| row.contains("Work"))
        .expect("work column header row");
    assert!(header_has_exp.contains("  Work "));
}

#[test]
fn board_mouse_wheel_scrolls_viewport_without_moving_selection() {
    let mut issues = Vec::new();
    let mut keys = Vec::new();
    for index in 1..=20 {
        let key = format!("KAN-{index}");
        let mut item = issue(&key, &format!("Card number {index}"), "Task", None);
        item.status = String::from("To Do");
        keys.push(key);
        issues.push(item);
    }
    let board = BoardData {
        id: 9,
        name: String::from("Kanban"),
        columns: vec![BoardColumnSummary {
            name: String::from("To Do"),
            statuses: vec![String::from("To Do")],
            max: None,
        }],
        swimlanes: vec![BoardSwimlaneSummary {
            id: None,
            name: String::from("Issues"),
            issue_keys: keys,
        }],
        issues,
        sprint: None,
    };
    let bindings = KeyBindings::default();
    let mut app = App::with_board_data(board);
    app.dispatch(Action::Tabs(TabAction::Previous));
    // Select the first card.
    app.handle_key(key('j'), &bindings);
    let before = app.selected_board_issue_key().map(str::to_owned);
    assert!(before.is_some());

    // Wheel down over the board content must scroll the viewport, not select.
    let area = ratatui::layout::Rect::new(0, 0, 100, 12);
    let wheel = MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 10,
        row: 6,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse(wheel, area, &bindings);

    assert_eq!(
        app.selected_board_issue_key().map(str::to_owned),
        before,
        "mouse wheel should not move the board selection"
    );
}

#[test]
fn board_left_click_still_selects_a_card() {
    // Guard: the wheel-vs-select change must not break click selection.
    let mut issues = Vec::new();
    let mut keys = Vec::new();
    for index in 1..=5 {
        let key = format!("KAN-{index}");
        let mut item = issue(&key, &format!("Card {index}"), "Task", None);
        item.status = String::from("To Do");
        keys.push(key);
        issues.push(item);
    }
    let board = BoardData {
        id: 9,
        name: String::from("Kanban"),
        columns: vec![BoardColumnSummary {
            name: String::from("To Do"),
            statuses: vec![String::from("To Do")],
            max: None,
        }],
        swimlanes: vec![BoardSwimlaneSummary {
            id: None,
            name: String::from("Issues"),
            issue_keys: keys,
        }],
        issues,
        sprint: None,
    };
    let bindings = KeyBindings::default();
    let mut app = App::with_board_data(board);
    app.dispatch(Action::Tabs(TabAction::Previous));
    let backend = TestBackend::new(100, 16);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");
    // Wheel scroll then verify selection is unchanged by scroll but a click selects.
    let area = ratatui::layout::Rect::new(0, 0, 100, 16);
    app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
        area,
        &bindings,
    );
    assert!(app.selected_board_issue_key().is_some());
}

#[test]
fn board_group_header_label_is_horizontally_sticky_when_scrolled() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let names = [
        "To Do",
        "In Progress",
        "In Review",
        "In Staging",
        "Ready For Prod",
        "Done",
    ];
    let columns = names
        .iter()
        .map(|n| BoardColumnSummary {
            name: String::from(*n),
            statuses: vec![String::from(*n)],
            max: None,
        })
        .collect();
    let mut done = issue("KAN-1", "Done card", "Task", None);
    done.status = String::from("Done");
    done.field_values
        .insert(String::from("assignee"), String::from("Alice"));
    let board = BoardData {
        id: 1,
        name: String::from("Kanban"),
        columns,
        swimlanes: vec![BoardSwimlaneSummary {
            id: None,
            name: String::from("Issues"),
            issue_keys: vec![String::from("KAN-1")],
        }],
        issues: vec![done],
        sprint: None,
    };
    let bindings = KeyBindings::default();
    let mut app = App::with_board_data(board);
    app.dispatch(Action::Tabs(TabAction::Previous));
    // Group by assignee, then move right to the far-right "Done" card to scroll right.
    app.handle_key(key('r'), &bindings);
    app.handle_key(ctrl('j'), &bindings);
    app.handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
        &bindings,
    );
    for _ in 0..names.len() {
        app.dispatch(Action::Board(tira::BoardAction::MoveRight));
    }

    let backend = TestBackend::new(120, 10);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    // Let the horizontal glide settle.
    for _ in 0..30 {
        terminal.draw(|f| draw(f, &app, &bindings)).expect("draw");
        app.tick(std::time::Duration::from_millis(40));
    }
    terminal.draw(|f| draw(f, &app, &bindings)).expect("draw");

    let (screen, _) = rendered_text(&terminal);
    let chars: Vec<char> = screen.chars().collect();
    let row = |i: usize| -> String { chars[i * 120..(i + 1) * 120].iter().collect() };
    // We are scrolled right: the rightmost column shows; leftmost is gone.
    let board_text = screen.clone();
    assert!(
        board_text.contains("DONE"),
        "scrolled to show the Done column"
    );
    // The group header label stays pinned near the left edge while scrolled
    // (a leading space + avatar glyph precede the name).
    let header_row = (0..10)
        .map(row)
        .find(|r| r.contains("Alice"))
        .expect("header row");
    let label_at = header_row.find("Alice").expect("label present");
    assert!(
        label_at < 6,
        "group label should be sticky at the left, got: {header_row:?}"
    );
}

#[test]
fn sprint_details_dialog_shows_sprint_summary() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let bindings = KeyBindings::default();
    let board = BoardData {
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
        issues: vec![issue("KAN-1", "Sprint task", "Task", None)],
        sprint: Some(SprintSummary {
            name: String::from("DICE Sprint 196"),
            goal: Some(String::from(
                "Deal Makers can create, edit, delete, and publish offer drafts end-to-end.",
            )),
            days_remaining: Some(4),
            start_date: Some(String::from("Jun 3, 2026")),
            end_date: Some(String::from("Jun 17, 2026")),
        }),
    };
    let mut app = App::with_board_data(board);
    app.dispatch(Action::Tabs(TabAction::Previous));
    app.handle_key(key('d'), &bindings);

    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");

    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains("Sprint details"));
    assert!(screen.contains("DICE Sprint 196"));
    assert!(screen.contains("4 days left"));
    assert!(screen.contains("Start date"));
    assert!(screen.contains("End date"));
    assert!(screen.contains("Jun 3, 2026"));
    assert!(screen.contains("Jun 17, 2026"));
}

#[test]
fn sprint_details_dialog_shows_no_active_sprint_message() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let bindings = KeyBindings::default();
    let board = BoardData {
        id: 9,
        name: String::from("DICE Upstream"),
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
        issues: vec![issue("KAN-1", "Kanban task", "Task", None)],
        sprint: None,
    };
    let mut app = App::with_board_data(board);
    app.dispatch(Action::Tabs(TabAction::Previous));
    app.handle_key(key('d'), &bindings);

    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");

    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains("DICE Upstream"));
    assert!(screen.contains("No active sprint"));
}

#[test]
fn board_top_bar_shows_details_days_left_and_lowercase_group() {
    let backend = TestBackend::new(100, 12);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let bindings = KeyBindings::default();
    let board = BoardData {
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
        issues: vec![issue("KAN-1", "Sprint task", "Task", None)],
        sprint: Some(SprintSummary {
            name: String::from("DICE Sprint 196"),
            goal: None,
            days_remaining: Some(4),
            start_date: None,
            end_date: None,
        }),
    };
    let mut app = App::with_board_data(board);
    app.dispatch(Action::Tabs(TabAction::Previous));

    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw app");

    // Trigger reads "details: 4 days left" and the group label is lowercase.
    let (screen, _) = rendered_text(&terminal);
    assert!(screen.contains("details: 4 days left"));
    assert!(screen.contains("group: None"));
}

#[test]
fn board_preserves_scroll_when_pane_is_temporarily_shrunk() {
    // A tall single-column board so the viewport must scroll vertically.
    let mut issues = Vec::new();
    let mut issue_keys = Vec::new();
    for index in 1..=15 {
        let key = format!("KAN-{index}");
        let mut item = issue(&key, &format!("Issue number {index}"), "Task", None);
        item.status = String::from("To Do");
        issue_keys.push(key);
        issues.push(item);
    }
    let board = BoardData {
        id: 1,
        name: String::from("Kanban"),
        columns: vec![BoardColumnSummary {
            name: String::from("To Do"),
            statuses: vec![String::from("To Do")],
            max: None,
        }],
        swimlanes: vec![BoardSwimlaneSummary {
            id: None,
            name: String::from("Issues"),
            issue_keys,
        }],
        issues,
        sprint: None,
    };
    let bindings = KeyBindings::default();
    let mut app = App::with_board_data(board);
    app.dispatch(Action::Tabs(TabAction::Previous));

    // Move the selection well down the list so the viewport is scrolled.
    for _ in 0..10 {
        app.handle_key(key('j'), &bindings);
    }

    // Settle at the full pane size.
    let mut full = Terminal::new(TestBackend::new(60, 20)).expect("full terminal");
    full.draw(|frame| draw(frame, &app, &bindings))
        .expect("draw full");
    app.tick(std::time::Duration::from_secs(1));
    full.draw(|frame| draw(frame, &app, &bindings))
        .expect("draw full");
    let before = app.board().scroll_offset.get();
    assert!(
        before > 0,
        "precondition: the board should be scrolled away from the top"
    );

    // Simulate zellij stacking the pane: many renders at a near-zero height.
    let mut tiny = Terminal::new(TestBackend::new(60, 2)).expect("tiny terminal");
    for _ in 0..5 {
        tiny.draw(|frame| draw(frame, &app, &bindings))
            .expect("draw tiny");
        app.tick(std::time::Duration::from_millis(50));
    }

    // Restore the pane and let any animation settle.
    full.draw(|frame| draw(frame, &app, &bindings))
        .expect("draw restored");
    app.tick(std::time::Duration::from_secs(1));
    full.draw(|frame| draw(frame, &app, &bindings))
        .expect("draw restored");
    let after = app.board().scroll_offset.get();

    assert_eq!(
        before, after,
        "scroll position is preserved across a temporary pane shrink, \
         not anchored to the bottom"
    );
}


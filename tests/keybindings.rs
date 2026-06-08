mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tira::{
    Action, App, AppEffect, AppEvent, JiraFilteredTreeAction, KeyBindings, Screen, TabAction,
    services::jira::{CommandLogEntry, UserSummary},
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
fn reload_help_entry_is_list_local_only() {
    let bindings = KeyBindings::default();

    let list_items = bindings.help_items(Screen::Main, "List", false);
    let board_items = bindings.help_items(Screen::Main, "Board", false);

    assert!(list_items.iter().any(|item| item.summary == "Reload list"));
    assert!(!board_items.iter().any(|item| item.summary == "Reload list"));
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
fn quick_switcher_only_lists_reload_on_list_tab() {
    let bindings = KeyBindings::default();
    let mut app = App::with_issues(Vec::new());

    app.handle_key(ctrl('k'), &bindings);
    assert!(
        app.quick_switcher()
            .expect("quick switcher")
            .options()
            .iter()
            .any(|option| option.label == "Reload list")
    );

    app.dispatch(Action::Tabs(TabAction::Next));
    app.handle_key(ctrl('k'), &bindings);
    assert!(
        !app.quick_switcher()
            .expect("quick switcher")
            .options()
            .iter()
            .any(|option| option.label == "Reload list")
    );
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

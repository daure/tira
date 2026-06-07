mod support;

use tira::{
    Action, App, JiraFilteredTreeAction, KeyBindings, Screen, TabAction, ui::theme::ThemeName,
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

    assert_eq!(bindings.global_action_for(shift('r')), None);
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
        bindings.leader_action_for(shift('t')),
        Action::ToggleThemeDropdown
    );
    assert_eq!(
        bindings.global_action_for(key('r')),
        Some(Action::ReloadList)
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

    let items = bindings.help_items(Screen::Main, "List");

    assert!(items.iter().any(|item| item.binding == "Ctrl+G O"));
    assert!(items.iter().any(|item| item.binding == "Ctrl+G M"));
    assert!(items.iter().any(|item| item.binding == "Ctrl+G N"));
    assert!(items.iter().any(|item| item.binding == "R"));
    assert!(items.iter().any(|item| item.binding == "Ctrl+G B"));
    assert!(items.iter().any(|item| item.binding == "Ctrl+G L"));
    assert!(items.iter().any(|item| item.binding == "Ctrl+G T"));
    assert!(items.iter().any(|item| item.binding == "Ctrl+G F"));
}

#[test]
fn reload_help_entry_is_list_local_only() {
    let bindings = KeyBindings::default();

    let list_items = bindings.help_items(Screen::Main, "List");
    let board_items = bindings.help_items(Screen::Main, "Board");

    assert!(list_items.iter().any(|item| item.summary == "Reload list"));
    assert!(!board_items.iter().any(|item| item.summary == "Reload list"));
}

#[test]
fn copy_help_entry_shows_yy_and_respects_custom_binding() {
    let bindings = KeyBindings::default();
    let list_items = bindings.help_items(Screen::Main, "List");
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
    let custom_list_items = custom_bindings.help_items(Screen::Main, "List");
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

    app.handle_key(key('j'), &bindings);
    app.handle_key(
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
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
    app.handle_key(shift('t'), &bindings);
    app.handle_key(
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
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
    app.handle_key(shift('t'), &bindings);
    app.handle_key(key('j'), &bindings);
    assert_eq!(app.theme().name(), ThemeName::Catppuccin);

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
            crossterm::event::KeyModifiers::NONE,
        ),
        &bindings,
    );
    assert!(app.is_command_log_open());

    app.dispatch(Action::CloseCommandLog);
    app.handle_key(ctrl('k'), &bindings);
    for _ in 0..6 {
        app.handle_key(key('j'), &bindings);
    }
    app.handle_key(
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
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
    app.handle_key(key('/'), &bindings);
    assert!(app.is_quick_switcher_filter_focused());
    app.handle_key(key('b'), &bindings);
    app.handle_key(key('o'), &bindings);
    app.handle_key(
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ),
        &bindings,
    );

    assert_eq!(app.active_tab(), "Board");
}

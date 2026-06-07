mod support;

use tira::{Action, App, JiraFilteredTreeAction, KeyBindings, TabAction, ui::theme::ThemeName};

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
fn global_reload_and_command_log_keys_are_mapped() {
    let bindings = KeyBindings::default();

    assert_eq!(
        bindings.global_action_for(shift('r')),
        Some(Action::ReloadList)
    );
    assert_eq!(
        bindings.global_action_for(shift('l')),
        Some(Action::ToggleCommandLog)
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
        switch_theme = "T"
        "##,
    );
    let mut app = App::with_issues(Vec::new());

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

    app.handle_key(ctrl('t'), &bindings);
    app.handle_key(
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ),
        &bindings,
    );

    assert_eq!(app.theme().name(), ThemeName::Catppuccin);
}

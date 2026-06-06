mod support;

use tira::{Action, App, KeyBindings, TabAction};

use support::{key, shift};

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

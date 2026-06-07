mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use tira::{
    App, KeyBindings, Screen,
    config::{self, JiraCredentials},
    draw,
};

use support::{key, rendered_text, temp_config_path};

#[test]
fn missing_credentials_start_on_setup_screen() {
    let app = App::default();

    assert_eq!(app.screen(), Screen::Setup);
    assert!(app.status().contains("No Jira credentials"));
}

#[test]
fn jira_credentials_load_from_atlassian_config() {
    let credentials = config::jira_credentials_from_toml(
        r#"
        [atlassian]
        site = "https://example.atlassian.net"
        email = "agent@example.com"
        api_key = "token"
        default_project = "KAN"
        "#,
    )
    .expect("credentials");

    assert_eq!(credentials.site, "https://example.atlassian.net");
    assert_eq!(credentials.email, "agent@example.com");
    assert_eq!(credentials.default_project, "KAN");
}

#[test]
fn jira_credentials_are_saved_to_config_file() {
    let path = temp_config_path();
    let credentials = JiraCredentials {
        site: String::from("https://example.atlassian.net"),
        email: String::from("agent@example.com"),
        api_key: String::from("token"),
        default_project: String::from("KAN"),
    };

    config::save_jira_credentials_to_path(&path, &credentials).expect("save credentials");
    let loaded = config::load_jira_credentials_from_path(&path)
        .expect("load config")
        .expect("credentials");

    assert_eq!(loaded.site, credentials.site);
    assert_eq!(loaded.email, credentials.email);
    assert_eq!(loaded.api_key, credentials.api_key);
    assert_eq!(loaded.default_project, credentials.default_project);
}

#[test]
fn credential_debug_output_redacts_api_tokens() {
    let credentials = JiraCredentials {
        site: String::from("https://example.atlassian.net"),
        email: String::from("agent@example.com"),
        api_key: String::from("secret-token"),
        default_project: String::from("KAN"),
    };

    let debug = format!("{credentials:?}");

    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("secret-token"));
}

#[test]
fn setup_screen_accepts_text_entry() {
    let bindings = KeyBindings::default();
    let mut app = App::default();

    app.handle_key(key('h'), &bindings);
    app.handle_key(key('t'), &bindings);
    app.handle_key(key('t'), &bindings);
    app.handle_key(key('p'), &bindings);
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), &bindings);
    app.handle_key(key('m'), &bindings);

    let fields = app.setup_form().fields();
    assert_eq!(fields[0].1, "http");
    assert_eq!(fields[1].1, "m");
}

#[test]
fn setup_text_input_owns_printable_theme_picker_binding() {
    let bindings = KeyBindings::from_toml_str(
        r##"
        [global]
        switch_theme = "T"
        "##,
    );
    let mut app = App::default();

    app.handle_key(
        KeyEvent::new(KeyCode::Char('T'), KeyModifiers::SHIFT),
        &bindings,
    );

    assert!(!app.is_theme_dropdown_open());
    assert_eq!(app.setup_form().fields()[0].1, "T");
}

#[test]
fn first_render_shows_setup_form_and_status_below_frame() {
    let backend = TestBackend::new(80, 12);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    let app = App::default();

    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw app");

    let (screen, bottom_row) = rendered_text(&terminal);

    assert!(screen.contains("Board"));
    assert!(screen.contains("List"));
    assert!(screen.contains("Timeline"));
    assert!(screen.contains("Filters"));
    assert!(screen.contains("Jira connection"));
    assert!(screen.contains("Jira site"));
    assert!(screen.contains("API token"));
    assert!(bottom_row.contains("INSERT"));
}

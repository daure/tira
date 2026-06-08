use std::{
    io,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use crossterm::event::{self, Event};
use ratatui::{Terminal, backend::CrosstermBackend};
use tira::{
    App, AppEffect, AppEvent, JiraLoadPurpose, JiraProjectLoadResult, KeyBindings, config, draw,
    services::{clipboard, jira},
    tui::Tui,
    ui::theme,
};

fn main() -> io::Result<()> {
    let mut app = config::load_jira_credentials().map_or_else(App::default, App::from_credentials);
    let theme = theme::load_theme().map_err(|error| io::Error::other(error.0))?;
    app.set_theme(theme);
    let keybindings = KeyBindings::load();
    let mut tui = Tui::enter()?;
    run(tui.terminal_mut(), &keybindings, app)
}

fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    keybindings: &KeyBindings,
    app: App,
) -> io::Result<()> {
    let mut app = app;
    let (event_tx, event_rx) = mpsc::channel();
    spawn_effects(&mut app, &event_tx);
    let mut last_tick = Instant::now();

    while app.is_running() {
        for event in event_rx.try_iter() {
            app.handle_event(event);
        }
        spawn_effects(&mut app, &event_tx);

        let dt = last_tick.elapsed();
        last_tick = Instant::now();
        app.tick(dt);
        terminal.draw(|frame| draw(frame, &app, keybindings))?;

        let timeout = if app.is_animating() {
            Duration::from_millis(16)
        } else {
            Duration::from_millis(250)
        };

        #[allow(clippy::collapsible_if)]
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => app.handle_key(key, keybindings),
                Event::Mouse(mouse) => {
                    let area = terminal.size()?;
                    app.handle_mouse(mouse, area.into(), keybindings);
                }
                _ => {}
            }
            spawn_effects(&mut app, &event_tx);
            last_tick = Instant::now();
        }
    }

    Ok(())
}

fn spawn_effects(app: &mut App, event_tx: &mpsc::Sender<AppEvent>) {
    for effect in app.take_effects() {
        match effect {
            AppEffect::LoadJiraProject {
                request_id,
                purpose,
                credentials,
                fields,
            } => {
                let tx = event_tx.clone();
                thread::spawn(move || {
                    if matches!(purpose, JiraLoadPurpose::Setup)
                        && let Err(error) = config::save_jira_credentials(&credentials)
                    {
                        let _ = tx.send(AppEvent::CredentialsSaveFailed {
                            request_id,
                            purpose,
                            error: error.to_string(),
                        });
                        return;
                    }

                    let result = load_jira_project_data(&credentials, &fields);
                    if matches!(purpose, JiraLoadPurpose::SwitchProject)
                        && result.issues.is_ok()
                        && let Err(error) = config::save_jira_credentials(&credentials)
                    {
                        let _ = tx.send(AppEvent::CredentialsSaveFailed {
                            request_id,
                            purpose,
                            error: error.to_string(),
                        });
                        return;
                    }

                    let _ = tx.send(AppEvent::JiraProjectLoaded {
                        request_id,
                        purpose,
                        credentials,
                        result,
                    });
                });
            }
            AppEffect::LoadMoreRoots {
                request_id,
                credentials,
                fields,
                page_token,
            } => {
                let tx = event_tx.clone();
                thread::spawn(move || {
                    let result =
                        jira::load_root_issues(&credentials, &fields, Some(page_token.as_str()));
                    let _ = tx.send(AppEvent::RootsPageLoaded { request_id, result });
                });
            }
            AppEffect::LoadChildren {
                request_id,
                credentials,
                parent_key,
                fields,
            } => {
                let tx = event_tx.clone();
                thread::spawn(move || {
                    let result =
                        jira::load_child_issues(&credentials, parent_key.as_str(), &fields);
                    let _ = tx.send(AppEvent::ChildrenLoaded {
                        request_id,
                        parent_key,
                        result,
                    });
                });
            }
            AppEffect::SearchIssues {
                request_id,
                credentials,
                term,
                fields,
            } => {
                let tx = event_tx.clone();
                thread::spawn(move || {
                    let result = jira::search_issues(&credentials, term.as_str(), &fields, None);
                    let _ = tx.send(AppEvent::SearchLoaded {
                        request_id,
                        term,
                        result,
                    });
                });
            }
            AppEffect::CopyToClipboard(url) => {
                let tx = event_tx.clone();
                thread::spawn(move || {
                    let event = match clipboard::copy_to_clipboard(&url) {
                        Ok(()) => AppEvent::IssueUrlCopied(url),
                        Err(error) => AppEvent::IssueUrlCopyFailed { url, error },
                    };
                    let _ = tx.send(event);
                });
            }
            AppEffect::AssignIssue {
                request_id,
                issue_key,
                assignee,
                credentials,
            } => {
                let tx = event_tx.clone();
                thread::spawn(move || {
                    let account_id = assignee.as_ref().map(|user| user.account_id.as_str());
                    let result = jira::assign_issue(&credentials, issue_key.as_str(), account_id);
                    let _ = tx.send(AppEvent::IssueAssigned {
                        request_id,
                        issue_key,
                        assignee,
                        result,
                    });
                });
            }
            AppEffect::SaveTheme(name) => {
                let tx = event_tx.clone();
                thread::spawn(move || {
                    if let Err(error) = theme::save_theme_name(name) {
                        let _ = tx.send(AppEvent::ThemeSaveFailed(error.0));
                    }
                });
            }
        }
    }
}

fn load_jira_project_data(
    credentials: &config::JiraCredentials,
    fields: &str,
) -> JiraProjectLoadResult {
    let fields_credentials = credentials.clone();
    let projects_credentials = credentials.clone();
    let users_credentials = credentials.clone();
    let issues_credentials = credentials.clone();
    let issues_fields = fields.to_owned();

    let fields_handle = thread::spawn(move || jira::load_issue_fields(&fields_credentials));
    let projects_handle = thread::spawn(move || jira::load_projects(&projects_credentials));
    let users_handle = thread::spawn(move || jira::load_assignable_users(&users_credentials));
    let issues_handle =
        thread::spawn(move || jira::load_root_issues(&issues_credentials, &issues_fields, None));

    let field_summaries = fields_handle.join().expect("jira fields thread panicked");
    let projects = projects_handle
        .join()
        .expect("jira projects thread panicked");
    let users = users_handle.join().expect("jira users thread panicked");
    let issues = issues_handle.join().expect("jira issues thread panicked");

    let mut logs = vec![field_summaries.log.clone(), projects.log.clone()];
    logs.extend(users.logs.clone());
    logs.extend(issues.logs.clone());

    JiraProjectLoadResult {
        logs,
        fields: field_summaries.fields,
        projects: projects.projects,
        users: users.users,
        current_user: users.current_user,
        issues: issues.issues,
        next_page_token: issues.next_page_token,
    }
}

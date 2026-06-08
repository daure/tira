mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tira::services::jira::{IssueSummary, JiraError, JiraLoadResult};
use tira::{
    App, AppEffect, AppEvent, JiraLoadPurpose, JiraProjectLoadResult, KeyBindings,
    config::JiraCredentials,
};

use support::{issue, key};

fn credentials() -> JiraCredentials {
    JiraCredentials {
        site: String::from("https://example.atlassian.net"),
        email: String::from("test@example.com"),
        api_key: String::from("test"),
        default_project: String::from("KAN"),
    }
}

fn issue_with_children(key: &str, summary: &str, kind: &str, has_children: bool) -> IssueSummary {
    let mut issue = issue(key, summary, kind, None);
    issue.has_children = has_children;
    issue
}

/// Loads a project with the given root issues and pending page token, draining
/// the initial `LoadJiraProject` effect. Returns the app ready on the List tab.
fn loaded_app(issues: Vec<IssueSummary>, next_page_token: Option<String>) -> App {
    let credentials = credentials();
    let mut app = App::from_credentials(credentials.clone());
    let effect = app.take_effects().remove(0);
    let AppEffect::LoadJiraProject { request_id, .. } = effect else {
        panic!("expected initial Jira load effect, got {effect:?}");
    };
    app.handle_event(AppEvent::JiraProjectLoaded {
        request_id,
        purpose: JiraLoadPurpose::Initial,
        credentials,
        result: JiraProjectLoadResult {
            issues: Ok(issues),
            next_page_token,
            fields: Ok(Vec::new()),
            projects: Ok(Vec::new()),
            users: Ok(Vec::new()),
            current_user: Err(JiraError(String::new())),
            logs: Vec::new(),
        },
    });
    app
}

#[test]
fn issue_with_children_shows_chevron_and_childless_issue_does_not() {
    let app = loaded_app(
        vec![
            issue_with_children("KAN-1", "Epic with children", "Epic", true),
            issue_with_children("KAN-2", "Lonely task", "Task", false),
        ],
        None,
    );

    let rows = app.visible_issue_rows();
    assert_eq!(rows.len(), 2);
    assert!(
        rows[0].expandable,
        "epic with children should be expandable"
    );
    assert!(
        !rows[1].expandable,
        "childless task should not be expandable"
    );
}

#[test]
fn pending_page_token_queues_background_root_pagination() {
    let mut app = loaded_app(
        vec![issue_with_children("KAN-1", "First page", "Task", false)],
        Some(String::from("page2")),
    );

    let effects = app.take_effects();
    let load_more = effects
        .iter()
        .find_map(|effect| match effect {
            AppEffect::LoadMoreRoots {
                request_id,
                page_token,
                ..
            } => Some((*request_id, page_token.clone())),
            _ => None,
        })
        .expect("expected a LoadMoreRoots effect");
    assert_eq!(load_more.1, "page2");

    // Background pagination must NOT keep the footer spinner running: the first
    // page is already shown, so loading is "done" from the user's perspective.
    assert!(
        !app.is_loading(),
        "background root pagination does not drive the loading spinner"
    );

    // Deliver the second page; its issues append and pagination stops.
    app.handle_event(AppEvent::RootsPageLoaded {
        request_id: load_more.0,
        result: JiraLoadResult {
            issues: Ok(vec![issue_with_children(
                "KAN-2",
                "Second page",
                "Task",
                false,
            )]),
            next_page_token: None,
            logs: Vec::new(),
        },
    });

    let ids: Vec<&str> = app.issues().iter().map(|item| item.id.as_str()).collect();
    assert_eq!(ids, vec!["KAN-1", "KAN-2"]);
    assert!(
        app.take_effects().is_empty(),
        "no more pages should be requested"
    );
}

#[test]
fn expanding_unloaded_epic_requests_and_splices_children() {
    let bindings = KeyBindings::default();
    let mut app = loaded_app(
        vec![issue_with_children("KAN-1", "Epic", "Epic", true)],
        None,
    );

    // Expand the selected epic with space.
    app.handle_key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        &bindings,
    );

    let effects = app.take_effects();
    let (request_id, parent_key) = effects
        .iter()
        .find_map(|effect| match effect {
            AppEffect::LoadChildren {
                request_id,
                parent_key,
                ..
            } => Some((*request_id, parent_key.clone())),
            _ => None,
        })
        .expect("expected a LoadChildren effect");
    assert_eq!(parent_key, "KAN-1");

    // The epic row shows a loading indicator while the fetch is in flight.
    assert!(app.visible_issue_rows()[0].loading);

    app.handle_event(AppEvent::ChildrenLoaded {
        request_id,
        parent_key,
        result: JiraLoadResult {
            issues: Ok(vec![{
                let mut child = issue("KAN-2", "Child story", "Story", Some("KAN-1"));
                child.has_children = false;
                child
            }]),
            next_page_token: None,
            logs: Vec::new(),
        },
    });

    let rows = app.visible_issue_rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(app.issues()[rows[1].item_index].id, "KAN-2");
    assert_eq!(rows[1].depth, 1);
    assert!(!rows[0].loading);
}

#[test]
fn stale_child_result_for_collapsed_node_is_ignored() {
    let bindings = KeyBindings::default();
    let mut app = loaded_app(
        vec![issue_with_children("KAN-1", "Epic", "Epic", true)],
        None,
    );

    app.handle_key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        &bindings,
    );
    let request_id = app
        .take_effects()
        .iter()
        .find_map(|effect| match effect {
            AppEffect::LoadChildren { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .expect("expected LoadChildren effect");

    // Collapse before the children arrive.
    app.handle_key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        &bindings,
    );

    // A different (newer) request id would mean this result is stale.
    app.handle_event(AppEvent::ChildrenLoaded {
        request_id: request_id + 999,
        parent_key: String::from("KAN-1"),
        result: JiraLoadResult {
            issues: Ok(vec![issue("KAN-2", "Child", "Story", Some("KAN-1"))]),
            next_page_token: None,
            logs: Vec::new(),
        },
    });

    // The stale result must not splice children into the tree.
    assert_eq!(app.issues().len(), 1);
}

#[test]
fn typing_filter_runs_server_search_and_shows_flat_results() {
    let bindings = KeyBindings::default();
    let mut app = loaded_app(
        vec![
            issue_with_children("KAN-1", "Checkout", "Task", false),
            issue_with_children("KAN-2", "Catalog", "Epic", true),
        ],
        None,
    );

    app.handle_key(key('/'), &bindings);
    for ch in "cat".chars() {
        app.handle_key(key(ch), &bindings);
    }

    let effects = app.take_effects();
    let (request_id, term) = effects
        .iter()
        .rev()
        .find_map(|effect| match effect {
            AppEffect::SearchIssues {
                request_id, term, ..
            } => Some((*request_id, term.clone())),
            _ => None,
        })
        .expect("expected a SearchIssues effect");
    assert_eq!(term, "cat");

    app.handle_event(AppEvent::SearchLoaded {
        request_id,
        term,
        result: JiraLoadResult {
            issues: Ok(vec![{
                // A search hit that is itself a child of another issue is shown
                // flat (no hierarchy) so its missing parent does not hide it.
                let mut hit = issue("KAN-9", "Cat toy", "Story", Some("KAN-2"));
                hit.has_children = false;
                hit
            }]),
            next_page_token: None,
            logs: Vec::new(),
        },
    });

    let rows = app.visible_issue_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(app.issues()[rows[0].item_index].id, "KAN-9");
    assert_eq!(rows[0].depth, 0, "search results render flat");
    assert!(!rows[0].expandable);
}

#[test]
fn only_latest_search_result_is_applied() {
    let bindings = KeyBindings::default();
    let mut app = loaded_app(
        vec![issue_with_children("KAN-1", "Root", "Task", false)],
        None,
    );

    app.handle_key(key('/'), &bindings);
    app.handle_key(key('a'), &bindings);
    app.handle_key(key('b'), &bindings);
    app.handle_key(key('c'), &bindings);

    // The most recent keystroke produced the latest search effect; only this
    // request's result should be applied.
    let latest_id = app
        .take_effects()
        .iter()
        .rev()
        .find_map(|effect| match effect {
            AppEffect::SearchIssues { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .expect("expected search effect");

    // A result for an older request id must be ignored.
    app.handle_event(AppEvent::SearchLoaded {
        request_id: latest_id - 1,
        term: String::from("ab"),
        result: JiraLoadResult {
            issues: Ok(vec![issue("STALE", "Stale", "Task", None)]),
            next_page_token: None,
            logs: Vec::new(),
        },
    });
    assert!(
        app.issues().iter().all(|item| item.id != "STALE"),
        "stale search result must not be applied"
    );

    // The latest result is applied.
    app.handle_event(AppEvent::SearchLoaded {
        request_id: latest_id,
        term: String::from("abc"),
        result: JiraLoadResult {
            issues: Ok(vec![issue("FRESH", "Fresh", "Task", None)]),
            next_page_token: None,
            logs: Vec::new(),
        },
    });
    assert_eq!(app.issues().len(), 1);
    assert_eq!(app.issues()[0].id, "FRESH");
}

#[test]
fn clearing_filter_restores_browse_view() {
    let bindings = KeyBindings::default();
    let mut app = loaded_app(
        vec![issue_with_children("KAN-1", "Root", "Task", false)],
        None,
    );

    app.handle_key(key('/'), &bindings);
    app.handle_key(key('a'), &bindings);
    app.take_effects();

    // Clear the query with backspace; this should queue a browse reload.
    app.handle_key(
        KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
        &bindings,
    );

    let reloads = app
        .take_effects()
        .into_iter()
        .filter(|effect| matches!(effect, AppEffect::LoadJiraProject { .. }))
        .count();
    assert_eq!(reloads, 1, "clearing the filter reloads the browse view");
}

#[test]
fn highlight_term_follows_displayed_results_not_live_input() {
    let bindings = KeyBindings::default();
    let mut app = loaded_app(
        vec![issue_with_children("KAN-1", "Root", "Task", false)],
        None,
    );

    // No search yet: nothing is highlighted.
    assert_eq!(app.highlight_term(), "");

    // Type "ab" -> a search for "ab" is queued, but no result has arrived.
    app.handle_key(key('/'), &bindings);
    app.handle_key(key('a'), &bindings);
    app.handle_key(key('b'), &bindings);
    let (request_id, term) = app
        .take_effects()
        .iter()
        .rev()
        .find_map(|effect| match effect {
            AppEffect::SearchIssues {
                request_id, term, ..
            } => Some((*request_id, term.clone())),
            _ => None,
        })
        .expect("expected search effect");
    assert_eq!(term, "ab");
    // Highlight must NOT jump to "ab" while results are still loading.
    assert_eq!(app.highlight_term(), "");

    // Result for "ab" arrives.
    app.handle_event(AppEvent::SearchLoaded {
        request_id,
        term,
        result: JiraLoadResult {
            issues: Ok(vec![issue("KAN-2", "ab hit", "Task", None)]),
            next_page_token: None,
            logs: Vec::new(),
        },
    });
    // Now highlights match the term that produced the visible rows.
    assert_eq!(app.highlight_term(), "ab");

    // Type another char: live input is "abc" but highlight stays "ab" until the
    // new result lands.
    app.handle_key(key('c'), &bindings);
    assert_eq!(app.filter(), "abc");
    assert_eq!(app.highlight_term(), "ab");
}

#[test]
fn child_load_in_flight_when_search_starts_does_not_corrupt_flat_results() {
    let bindings = KeyBindings::default();
    let mut app = loaded_app(
        vec![issue_with_children("KAN-1", "Epic", "Epic", true)],
        None,
    );

    // Begin expanding the epic, capturing its in-flight child request.
    app.handle_key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        &bindings,
    );
    let child_request = app
        .take_effects()
        .into_iter()
        .find_map(|effect| match effect {
            AppEffect::LoadChildren { request_id, .. } => Some(request_id),
            _ => None,
        })
        .expect("expected child load");

    // Switch to search before the children come back.
    app.handle_key(key('/'), &bindings);
    app.handle_key(key('x'), &bindings);
    let search_request = app
        .take_effects()
        .into_iter()
        .find_map(|effect| match effect {
            AppEffect::SearchIssues {
                request_id, term, ..
            } if term == "x" => Some(request_id),
            _ => None,
        })
        .expect("expected search effect");

    // The stale child result arrives now — it must be ignored (the node was
    // abandoned when search started), not spliced into the flat view.
    app.handle_event(AppEvent::ChildrenLoaded {
        request_id: child_request,
        parent_key: String::from("KAN-1"),
        result: JiraLoadResult {
            issues: Ok(vec![{
                let mut c = issue("KAN-2", "Stale child", "Story", Some("KAN-1"));
                c.has_children = false;
                c
            }]),
            next_page_token: None,
            logs: Vec::new(),
        },
    });
    assert!(
        app.issues().iter().all(|item| item.id != "KAN-2"),
        "stale child must not enter the search view"
    );

    // The search result lands cleanly as a flat list.
    app.handle_event(AppEvent::SearchLoaded {
        request_id: search_request,
        term: String::from("x"),
        result: JiraLoadResult {
            issues: Ok(vec![issue("KAN-9", "x match", "Task", None)]),
            next_page_token: None,
            logs: Vec::new(),
        },
    });
    let rows = app.visible_issue_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(app.issues()[rows[0].item_index].id, "KAN-9");
    assert_eq!(rows[0].depth, 0, "search results stay flat");
}

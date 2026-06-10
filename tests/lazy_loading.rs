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

/// `n` childless root issues (KAN-1..=KAN-n), enough rows to exceed the lazy
/// prefetch look-ahead so the next page isn't pulled until the user scrolls.
fn many_issues(n: usize) -> Vec<IssueSummary> {
    issues_range(1, n)
}

/// Childless root issues KAN-{start}..=KAN-{end}.
fn issues_range(start: usize, end: usize) -> Vec<IssueSummary> {
    (start..=end)
        .map(|i| issue_with_children(&format!("KAN-{i}"), "Item", "Task", false))
        .collect()
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
            board: Err(JiraError(String::from("board unavailable"))),
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
fn list_reload_fetches_roots_only_sized_to_extent() {
    let bindings = KeyBindings::default();
    let mut app = loaded_app(many_issues(50), Some(String::from("page2")));
    let _ = app.take_effects();

    // Load page 2 so 100 roots are loaded.
    app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), &bindings);
    let p2 = app
        .take_effects()
        .into_iter()
        .find_map(|e| match e {
            AppEffect::LoadMoreRoots { request_id, .. } => Some(request_id),
            _ => None,
        })
        .expect("page 2 requested");
    app.handle_event(AppEvent::RootsPageLoaded {
        request_id: p2,
        result: JiraLoadResult {
            issues: Ok(issues_range(51, 100)),
            next_page_token: Some(String::from("page3")),
            logs: Vec::new(),
        },
    });
    let _ = app.take_effects();

    // Shift+R issues a single project-load whose root query is sized to the
    // loaded extent (100), so the list comes back in one page.
    app.handle_key(
        KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT),
        &bindings,
    );
    let load = app
        .take_effects()
        .into_iter()
        .find_map(|e| match e {
            AppEffect::LoadJiraProject {
                purpose,
                root_max_results,
                ..
            } => Some((purpose, root_max_results)),
            _ => None,
        })
        .expect("list reload issues a project load");
    assert_eq!(load.0, JiraLoadPurpose::Reload);
    assert_eq!(
        load.1, 100,
        "the reload's root query is sized to the loaded extent"
    );
}

#[test]
fn reload_preserves_selection_on_a_later_page() {
    let bindings = KeyBindings::default();
    let mut app = loaded_app(many_issues(50), Some(String::from("page2")));
    let _ = app.take_effects();

    // Load page 2 (KAN-51..100).
    app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), &bindings);
    let p2 = app
        .take_effects()
        .into_iter()
        .find_map(|e| match e {
            AppEffect::LoadMoreRoots { request_id, .. } => Some(request_id),
            _ => None,
        })
        .expect("page 2 requested");
    app.handle_event(AppEvent::RootsPageLoaded {
        request_id: p2,
        result: JiraLoadResult {
            issues: Ok(issues_range(51, 100)),
            next_page_token: Some(String::from("page3")),
            logs: Vec::new(),
        },
    });

    // Select a root that lives on page 2.
    app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), &bindings);
    assert_eq!(app.selected_issue_key(), Some("KAN-100"));
    let _ = app.take_effects();

    // Shift+R, then settle the reload's first page and its remainder.
    app.handle_key(
        KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT),
        &bindings,
    );
    let reload = app
        .take_effects()
        .into_iter()
        .find_map(|e| match e {
            AppEffect::LoadJiraProject { request_id, .. } => Some(request_id),
            _ => None,
        })
        .expect("reload");
    app.handle_event(AppEvent::JiraProjectLoaded {
        request_id: reload,
        purpose: JiraLoadPurpose::Reload,
        credentials: credentials(),
        result: JiraProjectLoadResult {
            issues: Ok(many_issues(50)),
            board: Err(JiraError(String::from("board unavailable"))),
            next_page_token: Some(String::from("page2")),
            fields: Ok(Vec::new()),
            projects: Ok(Vec::new()),
            users: Ok(Vec::new()),
            current_user: Err(JiraError(String::new())),
            logs: Vec::new(),
        },
    });
    let rem = app
        .take_effects()
        .into_iter()
        .find_map(|e| match e {
            AppEffect::LoadMoreRoots { request_id, .. } => Some(request_id),
            _ => None,
        })
        .expect("remainder requested");
    app.handle_event(AppEvent::RootsPageLoaded {
        request_id: rem,
        result: JiraLoadResult {
            issues: Ok(issues_range(51, 100)),
            next_page_token: Some(String::from("page3")),
            logs: Vec::new(),
        },
    });

    // The page-2 selection (and thus the scroll position) survives the reload.
    assert_eq!(
        app.selected_issue_key(),
        Some("KAN-100"),
        "selection on a later page must be preserved across reload"
    );
}

#[test]
fn reload_without_page_token_keeps_loaded_extent() {
    // Reproduces the bug where a reload whose first page came back without a
    // next-page token pruned the whole loaded extent down to one page.
    let bindings = KeyBindings::default();
    let mut app = loaded_app(many_issues(50), Some(String::from("page2")));
    let _ = app.take_effects();

    // Lazily load page 2 (KAN-51..100), so 100 roots are loaded.
    app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), &bindings);
    let page2 = app
        .take_effects()
        .into_iter()
        .find_map(|e| match e {
            AppEffect::LoadMoreRoots { request_id, .. } => Some(request_id),
            _ => None,
        })
        .expect("page 2 requested");
    app.handle_event(AppEvent::RootsPageLoaded {
        request_id: page2,
        result: JiraLoadResult {
            issues: Ok(issues_range(51, 100)),
            next_page_token: Some(String::from("page3")),
            logs: Vec::new(),
        },
    });
    let _ = app.take_effects();

    // Shift+R, but the reload's first page comes back WITHOUT a next-page token.
    app.handle_key(
        KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT),
        &bindings,
    );
    let reload_id = app
        .take_effects()
        .into_iter()
        .find_map(|e| match e {
            AppEffect::LoadJiraProject { request_id, .. } => Some(request_id),
            _ => None,
        })
        .expect("reload triggers project load");
    app.handle_event(AppEvent::JiraProjectLoaded {
        request_id: reload_id,
        purpose: JiraLoadPurpose::Reload,
        credentials: credentials(),
        result: JiraProjectLoadResult {
            issues: Ok(many_issues(50)),
            board: Err(JiraError(String::from("board unavailable"))),
            next_page_token: None,
            fields: Ok(Vec::new()),
            projects: Ok(Vec::new()),
            users: Ok(Vec::new()),
            current_user: Err(JiraError(String::new())),
            logs: Vec::new(),
        },
    });

    // The previously-loaded lower extent must survive — not be pruned to page 1.
    assert!(
        app.issues().iter().any(|item| item.id == "KAN-100"),
        "reload without a page token must keep the loaded extent, not drop it"
    );
}

#[test]
fn reload_after_expanding_keeps_the_loaded_extent() {
    let bindings = KeyBindings::default();
    // Page 1: 100 roots; KAN-1 is an epic with children.
    let mut page1 = issues_range(2, 100);
    page1.insert(0, issue_with_children("KAN-1", "Epic", "Epic", true));
    let mut app = loaded_app(page1, Some(String::from("page2")));
    let _ = app.take_effects();

    // Expand KAN-1 (selected by default) and deliver a child.
    app.handle_key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        &bindings,
    );
    let child_req = app
        .take_effects()
        .into_iter()
        .find_map(|e| match e {
            AppEffect::LoadChildren { request_id, .. } => Some(request_id),
            _ => None,
        })
        .expect("expand requests children");
    app.handle_event(AppEvent::ChildrenLoaded {
        request_id: child_req,
        parent_key: String::from("KAN-1"),
        result: JiraLoadResult {
            issues: Ok(vec![issue("KAN-1-c", "Child", "Story", Some("KAN-1"))]),
            next_page_token: None,
            logs: Vec::new(),
        },
    });

    // Scroll to lazily load page 2 (KAN-101..200).
    app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), &bindings);
    let page2_req = app
        .take_effects()
        .into_iter()
        .find_map(|e| match e {
            AppEffect::LoadMoreRoots { request_id, .. } => Some(request_id),
            _ => None,
        })
        .expect("page 2 requested");
    app.handle_event(AppEvent::RootsPageLoaded {
        request_id: page2_req,
        result: JiraLoadResult {
            issues: Ok(issues_range(101, 200)),
            next_page_token: Some(String::from("page3")),
            logs: Vec::new(),
        },
    });
    let _ = app.take_effects();
    assert!(
        app.issues().iter().any(|i| i.id == "KAN-200"),
        "page 2 loaded before reload"
    );

    // Shift+R: deliver the reload's fresh first page.
    app.handle_key(
        KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT),
        &bindings,
    );
    let reload_id = app
        .take_effects()
        .into_iter()
        .find_map(|e| match e {
            AppEffect::LoadJiraProject { request_id, .. } => Some(request_id),
            _ => None,
        })
        .expect("reload triggers project load");
    let mut reload_page1 = issues_range(2, 100);
    reload_page1.insert(0, issue_with_children("KAN-1", "Epic", "Epic", true));
    app.handle_event(AppEvent::JiraProjectLoaded {
        request_id: reload_id,
        purpose: JiraLoadPurpose::Reload,
        credentials: credentials(),
        result: JiraProjectLoadResult {
            issues: Ok(reload_page1),
            board: Err(JiraError(String::from("board unavailable"))),
            next_page_token: Some(String::from("page2")),
            fields: Ok(Vec::new()),
            projects: Ok(Vec::new()),
            users: Ok(Vec::new()),
            current_user: Err(JiraError(String::new())),
            logs: Vec::new(),
        },
    });

    // The reload must refetch the loaded extent (100 roots beyond page 1), not
    // prune back to a single page.
    let remainder = app.take_effects().into_iter().find_map(|e| match e {
        AppEffect::LoadMoreRoots {
            request_id,
            max_results,
            ..
        } => Some((request_id, max_results)),
        _ => None,
    });
    let (rid, max) = remainder.expect("reload refetches the loaded extent in one query");
    assert_eq!(max, 100, "remainder sized to roots beyond page 1");
    app.handle_event(AppEvent::RootsPageLoaded {
        request_id: rid,
        result: JiraLoadResult {
            issues: Ok(issues_range(101, 200)),
            next_page_token: Some(String::from("page3")),
            logs: Vec::new(),
        },
    });
    assert!(
        app.issues().iter().any(|i| i.id == "KAN-200"),
        "loaded extent restored after reload (not shrunk to one page)"
    );
}

#[test]
fn reload_refetches_loaded_extent_in_one_query_not_page_by_page() {
    let bindings = KeyBindings::default();
    // Page 1 (50 roots) with more pages available.
    let mut app = loaded_app(many_issues(50), Some(String::from("page2")));
    let _ = app.take_effects();

    // Lazily load a second page (50 more roots); the user has now paged in 100.
    app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), &bindings);
    let page2 = app
        .take_effects()
        .into_iter()
        .find_map(|effect| match effect {
            AppEffect::LoadMoreRoots { request_id, .. } => Some(request_id),
            _ => None,
        })
        .expect("scrolling should request page 2");
    app.handle_event(AppEvent::RootsPageLoaded {
        request_id: page2,
        result: JiraLoadResult {
            issues: Ok(issues_range(51, 100)),
            next_page_token: Some(String::from("page3")),
            logs: Vec::new(),
        },
    });
    let _ = app.take_effects();

    // Shift+R: full reload. Deliver the project load's fresh first page (50).
    app.handle_key(
        KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT),
        &bindings,
    );
    let reload_id = app
        .take_effects()
        .into_iter()
        .find_map(|effect| match effect {
            AppEffect::LoadJiraProject { request_id, .. } => Some(request_id),
            _ => None,
        })
        .expect("Shift+R triggers a project reload");
    app.handle_event(AppEvent::JiraProjectLoaded {
        request_id: reload_id,
        purpose: JiraLoadPurpose::Reload,
        credentials: credentials(),
        result: JiraProjectLoadResult {
            issues: Ok(many_issues(50)),
            board: Err(JiraError(String::from("board unavailable"))),
            next_page_token: Some(String::from("page2")),
            fields: Ok(Vec::new()),
            projects: Ok(Vec::new()),
            users: Ok(Vec::new()),
            current_user: Err(JiraError(String::new())),
            logs: Vec::new(),
        },
    });

    // The reload refetches the REST of the loaded extent (100 loaded - 50 in the
    // first page = 50) in a SINGLE query, not 50 page-by-page calls.
    let max_results: Vec<u32> = app
        .take_effects()
        .into_iter()
        .filter_map(|effect| match effect {
            AppEffect::LoadMoreRoots { max_results, .. } => Some(max_results),
            _ => None,
        })
        .collect();
    assert_eq!(
        max_results.len(),
        1,
        "reload pulls the loaded extent in one query, not page-by-page"
    );
    assert_eq!(
        max_results[0], 50,
        "the single query is sized to the remaining loaded extent"
    );
}

#[test]
fn pending_page_token_does_not_eager_paginate() {
    // A pending page token alone must NOT pull more pages: paging is lazy and
    // waits until the user scrolls toward the bottom.
    let mut app = loaded_app(many_issues(50), Some(String::from("page2")));

    assert!(
        app.take_effects()
            .iter()
            .all(|effect| !matches!(effect, AppEffect::LoadMoreRoots { .. })),
        "initial load must not eagerly request the next page"
    );
    assert!(
        !app.is_loading(),
        "lazy paging does not drive the loading spinner"
    );
}

#[test]
fn scrolling_near_bottom_lazily_requests_next_page() {
    let bindings = KeyBindings::default();
    let mut app = loaded_app(many_issues(50), Some(String::from("page2")));
    let _ = app.take_effects();

    // Jump to the end of the loaded rows; nearing the bottom pulls the next page.
    app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), &bindings);

    let load_more = app
        .take_effects()
        .into_iter()
        .find_map(|effect| match effect {
            AppEffect::LoadMoreRoots {
                request_id,
                page_token,
                ..
            } => Some((request_id, page_token)),
            _ => None,
        })
        .expect("scrolling near the bottom should request the next page");
    assert_eq!(load_more.1, "page2");

    // While the lazy page is in flight the footer spinner runs and the status
    // reflects the load.
    assert!(
        app.is_loading(),
        "footer spinner runs while the next page loads"
    );
    assert_eq!(app.status(), "Loading more issues…");

    // Deliver the second page; its issues append and no further page is chased.
    app.handle_event(AppEvent::RootsPageLoaded {
        request_id: load_more.0,
        result: JiraLoadResult {
            issues: Ok(vec![issue_with_children(
                "KAN-200",
                "Second page",
                "Task",
                false,
            )]),
            next_page_token: None,
            logs: Vec::new(),
        },
    });

    assert!(
        app.issues().iter().any(|item| item.id == "KAN-200"),
        "second page appended"
    );
    assert!(!app.is_loading(), "spinner stops once the page lands");
    assert_eq!(app.status(), "Jira issues loaded.");
    assert!(
        app.take_effects().is_empty(),
        "no eager follow-up paging once the page lands"
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

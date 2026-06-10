mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;
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

fn loaded_app(issues: Vec<IssueSummary>) -> App {
    let credentials = credentials();
    let mut app = App::from_credentials(credentials.clone());
    let effect = app.take_effects().remove(0);
    let AppEffect::LoadJiraProject { request_id, .. } = effect else {
        panic!("expected initial Jira load effect");
    };
    app.handle_event(AppEvent::JiraProjectLoaded {
        request_id,
        purpose: JiraLoadPurpose::Initial,
        credentials,
        result: JiraProjectLoadResult {
            issues: Ok(issues),
            board: Err(JiraError(String::from("board unavailable"))),
            next_page_token: None,
            fields: Ok(Vec::new()),
            projects: Ok(Vec::new()),
            users: Ok(Vec::new()),
            current_user: Err(JiraError(String::new())),
            logs: Vec::new(),
        },
    });
    app
}

fn child(key: &str, parent: &str) -> IssueSummary {
    let mut c = issue(key, "Child", "Story", Some(parent));
    c.has_children = false;
    c
}

/// A child that itself has children (an expandable nested node).
fn parent_child(key: &str, parent: &str) -> IssueSummary {
    let mut c = issue(key, "Sub-epic", "Story", Some(parent));
    c.has_children = true;
    c
}

/// Drains the pending `LoadChildren` effect for `parent` and delivers `children`.
fn deliver_children(app: &mut App, parent: &str, children: Vec<IssueSummary>) {
    let request_id = app
        .take_effects()
        .into_iter()
        .find_map(|effect| match effect {
            AppEffect::LoadChildren {
                request_id,
                parent_key,
                ..
            } if parent_key == parent => Some(request_id),
            _ => None,
        })
        .unwrap_or_else(|| panic!("expected LoadChildren effect for {parent}"));
    app.handle_event(AppEvent::ChildrenLoaded {
        request_id,
        parent_key: parent.to_owned(),
        result: JiraLoadResult {
            issues: Ok(children),
            next_page_token: None,
            logs: Vec::new(),
        },
    });
}

/// Drains the pending `LoadChildrenBatch` effect, returning its `(parent, id)` pairs.
fn take_child_batch(app: &mut App) -> Vec<(String, u64)> {
    app.take_effects()
        .into_iter()
        .find_map(|effect| match effect {
            AppEffect::LoadChildrenBatch { parents, .. } => Some(parents),
            _ => None,
        })
        .expect("expected LoadChildrenBatch effect")
}

/// Delivers batched children results for the given parent->children sets,
/// matching each parent to the request id drained from the batch effect.
fn deliver_child_batch(
    app: &mut App,
    parents: &[(String, u64)],
    groups: Vec<(&str, Vec<IssueSummary>)>,
) {
    let results = groups
        .into_iter()
        .map(|(parent, children)| {
            let request_id = parents
                .iter()
                .find(|(key, _)| key == parent)
                .map(|(_, id)| *id)
                .unwrap_or_else(|| panic!("batch missing parent {parent}"));
            (
                request_id,
                parent.to_owned(),
                Ok::<Vec<IssueSummary>, JiraError>(children),
            )
        })
        .collect();
    app.handle_event(AppEvent::ChildrenBatchLoaded {
        results,
        logs: Vec::new(),
    });
}

/// Expands an epic and delivers its first child set, returning the app with the
/// epic expanded and loaded.
fn expand_and_load(
    app: &mut App,
    parent: &str,
    children: Vec<IssueSummary>,
    bindings: &KeyBindings,
) {
    app.handle_key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        bindings,
    );
    let request_id = app
        .take_effects()
        .iter()
        .find_map(|effect| match effect {
            AppEffect::LoadChildren { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .expect("expected initial LoadChildren effect");
    app.handle_event(AppEvent::ChildrenLoaded {
        request_id,
        parent_key: parent.to_owned(),
        result: JiraLoadResult {
            issues: Ok(children),
            next_page_token: None,
            logs: Vec::new(),
        },
    });
}

#[test]
fn reload_node_refetches_children_of_selected_node() {
    let bindings = KeyBindings::default();
    let mut app = loaded_app(vec![issue_with_children("KAN-1", "Epic", "Epic", true)]);

    expand_and_load(&mut app, "KAN-1", vec![child("KAN-2", "KAN-1")], &bindings);
    assert_eq!(app.visible_issue_rows().len(), 2);

    // Reload the selected node (the epic) with `r`.
    app.handle_key(key('r'), &bindings);

    // The stale child stays visible (greyed) while the node refreshes in place.
    let rows = app.visible_issue_rows();
    assert_eq!(
        rows.len(),
        2,
        "subtree stays visible during in-place reload"
    );
    assert!(rows[0].loading, "node shows loading indicator");
    assert!(rows[1].reloading, "stale child greyed while refreshing");

    let parents = take_child_batch(&mut app);
    assert_eq!(
        parents
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        vec!["KAN-1"],
        "reload refetches the selected node"
    );

    // Deliver fresh children.
    deliver_child_batch(
        &mut app,
        &parents,
        vec![(
            "KAN-1",
            vec![child("KAN-3", "KAN-1"), child("KAN-4", "KAN-1")],
        )],
    );

    let rows = app.visible_issue_rows();
    assert_eq!(rows.len(), 3, "refreshed children appear");
    assert!(!rows[0].loading, "spinner cleared after load");
    let ids: Vec<&str> = rows
        .iter()
        .map(|row| app.issues()[row.item_index].id.as_str())
        .collect();
    assert_eq!(ids, vec!["KAN-1", "KAN-3", "KAN-4"]);
}

#[test]
fn reload_node_on_leaf_does_nothing() {
    let bindings = KeyBindings::default();
    let mut app = loaded_app(vec![issue_with_children("KAN-1", "Leaf", "Task", false)]);

    app.handle_key(key('r'), &bindings);

    // A childless node has nothing to reload; `r` must not queue any work and
    // must not trigger a full list reload (only Shift+R does that).
    assert!(
        app.take_effects().is_empty(),
        "r on a childless node is a no-op"
    );
}

#[test]
fn shift_r_reloads_the_whole_list() {
    let bindings = KeyBindings::default();
    let mut app = loaded_app(vec![issue_with_children("KAN-1", "Epic", "Epic", true)]);

    app.handle_key(
        KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT),
        &bindings,
    );

    let reloads = app
        .take_effects()
        .into_iter()
        .filter(|effect| matches!(effect, AppEffect::LoadJiraProject { .. }))
        .count();
    assert_eq!(reloads, 1, "Shift+R reloads the whole project");
}

#[test]
fn spinner_animates_while_a_node_is_loading() {
    let bindings = KeyBindings::default();
    let mut app = loaded_app(vec![issue_with_children("KAN-1", "Epic", "Epic", true)]);

    // Expand to start a child load (no result delivered yet).
    app.handle_key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        &bindings,
    );
    assert!(
        app.take_effects()
            .iter()
            .any(|effect| matches!(effect, AppEffect::LoadChildren { .. }))
    );

    // While loading, the app reports animating so the main loop keeps redrawing.
    assert!(app.is_animating(), "loading node keeps the UI animating");

    // The spinner glyph advances across ticks.
    let first = app.spinner_glyph();
    app.tick(Duration::from_millis(200));
    let later = app.spinner_glyph();
    assert_ne!(first, later, "spinner glyph advances over time");
}

#[test]
fn spinner_stops_animating_once_children_arrive() {
    let bindings = KeyBindings::default();
    let mut app = loaded_app(vec![issue_with_children("KAN-1", "Epic", "Epic", true)]);

    expand_and_load(&mut app, "KAN-1", vec![child("KAN-2", "KAN-1")], &bindings);

    // With no outstanding child load, the spinner is frozen even though scroll
    // easing may still be settling.
    let before = app.spinner_glyph();
    app.tick(Duration::from_millis(200));
    assert_eq!(
        before,
        app.spinner_glyph(),
        "spinner frozen when no node loads"
    );
}

#[test]
fn full_reload_restores_previously_expanded_nodes() {
    let bindings = KeyBindings::default();
    let mut app = loaded_app(vec![
        issue_with_children("KAN-1", "Epic", "Epic", true),
        issue_with_children("KAN-9", "Other epic", "Epic", true),
    ]);
    // Expand only KAN-1.
    expand_and_load(&mut app, "KAN-1", vec![child("KAN-2", "KAN-1")], &bindings);
    assert_eq!(
        app.visible_issue_rows().len(),
        3,
        "KAN-1 expanded with a child"
    );

    // Shift+R reloads the whole project. The expanded subtree refreshes
    // immediately, in parallel with the root query, so the child batch is
    // queued alongside the root load rather than after its response.
    app.handle_key(
        KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT),
        &bindings,
    );
    let effects = app.take_effects();
    let request_id = effects
        .iter()
        .find_map(|effect| match effect {
            AppEffect::LoadJiraProject { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .expect("expected full reload effect");
    let parents = effects
        .iter()
        .find_map(|effect| match effect {
            AppEffect::LoadChildrenBatch { parents, .. } => Some(parents.clone()),
            _ => None,
        })
        .expect("expected child batch queued with the reload");
    app.handle_event(AppEvent::JiraProjectLoaded {
        request_id,
        purpose: JiraLoadPurpose::Reload,
        credentials: credentials(),
        result: JiraProjectLoadResult {
            issues: Ok(vec![
                issue_with_children("KAN-1", "Epic", "Epic", true),
                issue_with_children("KAN-9", "Other epic", "Epic", true),
            ]),
            board: Err(JiraError(String::from("board unavailable"))),
            next_page_token: None,
            fields: Ok(Vec::new()),
            projects: Ok(Vec::new()),
            users: Ok(Vec::new()),
            current_user: Err(JiraError(String::new())),
            logs: Vec::new(),
        },
    });

    // KAN-1's subtree stays visible (greyed) while it refreshes in place: the
    // child is not dropped, KAN-1 shows the spinner, and only KAN-1 refetches.
    let rows = app.visible_issue_rows();
    assert_eq!(
        rows.len(),
        3,
        "subtree stays visible during in-place reload"
    );
    assert!(rows[0].loading, "refreshing epic shows loading spinner");
    assert_eq!(app.issues()[rows[1].item_index].id, "KAN-2");
    assert!(rows[1].reloading, "stale child greyed while refreshing");
    assert_eq!(
        parents
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        vec!["KAN-1"],
        "only the expanded epic refetches"
    );

    // Deliver KAN-1's children; it settles with the fresh child visible.
    deliver_child_batch(
        &mut app,
        &parents,
        vec![("KAN-1", vec![child("KAN-2", "KAN-1")])],
    );
    let rows = app.visible_issue_rows();
    assert_eq!(rows.len(), 3, "subtree intact after refresh");
    assert!(rows[0].expanded && !rows[0].loading);
    assert!(!rows[1].reloading, "child no longer greyed");
    assert_eq!(app.issues()[rows[1].item_index].id, "KAN-2");
}

#[test]
fn full_reload_batches_all_expanded_nodes_into_one_query() {
    let bindings = KeyBindings::default();
    let mut app = loaded_app(vec![
        issue_with_children("KAN-1", "Epic", "Epic", true),
        issue_with_children("KAN-9", "Other epic", "Epic", true),
    ]);
    // Expand both sibling epics.
    expand_and_load(&mut app, "KAN-1", vec![child("KAN-2", "KAN-1")], &bindings);
    app.handle_key(key('j'), &bindings); // KAN-2
    app.handle_key(key('j'), &bindings); // KAN-9
    assert_eq!(app.selected_issue_key(), Some("KAN-9"));
    expand_and_load(&mut app, "KAN-9", vec![child("KAN-10", "KAN-9")], &bindings);

    // Full reload — the two expanded subtrees refresh immediately, in parallel
    // with the root query, rather than waiting for the root response.
    app.handle_key(
        KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT),
        &bindings,
    );

    // The two expanded subtrees refresh in a SINGLE batched query, not one
    // request per node — this is the whole point of batching.
    let effects = app.take_effects();
    let singles = effects
        .iter()
        .filter(|effect| matches!(effect, AppEffect::LoadChildren { .. }))
        .count();
    let batches: Vec<&Vec<(String, u64)>> = effects
        .iter()
        .filter_map(|effect| match effect {
            AppEffect::LoadChildrenBatch { parents, .. } => Some(parents),
            _ => None,
        })
        .collect();
    assert_eq!(singles, 0, "no per-parent child loads");
    assert_eq!(batches.len(), 1, "exactly one batched query for all nodes");
    let names: std::collections::HashSet<&str> =
        batches[0].iter().map(|(key, _)| key.as_str()).collect();
    assert!(
        names.contains("KAN-1") && names.contains("KAN-9"),
        "the single batch covers both expanded nodes"
    );
}

#[test]
fn full_reload_restores_nested_expanded_subtree_in_parallel() {
    let bindings = KeyBindings::default();
    let mut app = loaded_app(vec![issue_with_children("KAN-1", "Epic", "Epic", true)]);
    // Expand KAN-1, revealing a nested expandable node KAN-2; expand that too.
    expand_and_load(
        &mut app,
        "KAN-1",
        vec![parent_child("KAN-2", "KAN-1")],
        &bindings,
    );
    // Select and expand KAN-2 (row 1).
    app.handle_key(key('j'), &bindings);
    assert_eq!(app.selected_issue_key(), Some("KAN-2"));
    app.handle_key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        &bindings,
    );
    deliver_children(&mut app, "KAN-2", vec![child("KAN-3", "KAN-2")]);
    assert_eq!(
        app.visible_issue_rows().len(),
        3,
        "KAN-1 > KAN-2 > KAN-3 open"
    );

    // Full reload. The expanded subtrees refresh immediately, in parallel with
    // the root query, so their child batch is queued alongside the root load.
    app.handle_key(
        KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT),
        &bindings,
    );
    let effects = app.take_effects();
    let request_id = effects
        .iter()
        .find_map(|effect| match effect {
            AppEffect::LoadJiraProject { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .expect("expected full reload effect");
    let parents = effects
        .iter()
        .find_map(|effect| match effect {
            AppEffect::LoadChildrenBatch { parents, .. } => Some(parents.clone()),
            _ => None,
        })
        .expect("expected child batch queued with the reload");
    app.handle_event(AppEvent::JiraProjectLoaded {
        request_id,
        purpose: JiraLoadPurpose::Reload,
        credentials: credentials(),
        result: JiraProjectLoadResult {
            issues: Ok(vec![issue_with_children("KAN-1", "Epic", "Epic", true)]),
            board: Err(JiraError(String::from("board unavailable"))),
            next_page_token: None,
            fields: Ok(Vec::new()),
            projects: Ok(Vec::new()),
            users: Ok(Vec::new()),
            current_user: Err(JiraError(String::new())),
            logs: Vec::new(),
        },
    });

    // Both expanded levels refresh together in one batched query: KAN-1 and
    // KAN-2 refetch at once while their stale rows stay visible.
    let names: std::collections::HashSet<&str> =
        parents.iter().map(|(key, _)| key.as_str()).collect();
    assert!(
        names.contains("KAN-1") && names.contains("KAN-2"),
        "both expanded subtrees refresh in one batch"
    );

    // Deliver both refreshes; each subtree swaps in place.
    deliver_child_batch(
        &mut app,
        &parents,
        vec![
            ("KAN-1", vec![parent_child("KAN-2", "KAN-1")]),
            ("KAN-2", vec![child("KAN-3", "KAN-2")]),
        ],
    );

    let rows = app.visible_issue_rows();
    let ids: Vec<&str> = rows
        .iter()
        .map(|row| app.issues()[row.item_index].id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec!["KAN-1", "KAN-2", "KAN-3"],
        "nested subtree fully restored"
    );
    assert!(rows.iter().all(|row| !row.loading), "no spinners left");
    assert!(rows.iter().all(|row| !row.reloading), "nothing left greyed");
}

#[test]
fn full_reload_holds_subtree_dimmed_until_roots_land_when_children_arrive_first() {
    let bindings = KeyBindings::default();
    let mut app = loaded_app(vec![issue_with_children("KAN-1", "Epic", "Epic", true)]);
    expand_and_load(&mut app, "KAN-1", vec![child("KAN-2", "KAN-1")], &bindings);

    // Shift+R fires the root query and the expanded subtree's child batch in
    // parallel; capture both.
    app.handle_key(
        KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT),
        &bindings,
    );
    let effects = app.take_effects();
    let request_id = effects
        .iter()
        .find_map(|effect| match effect {
            AppEffect::LoadJiraProject { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .expect("expected full reload effect");
    let parents = effects
        .iter()
        .find_map(|effect| match effect {
            AppEffect::LoadChildrenBatch { parents, .. } => Some(parents.clone()),
            _ => None,
        })
        .expect("expected child batch queued with the reload");

    // The child batch returns BEFORE the roots. The subtree must stay dimmed —
    // its result is held back so the reload settles as one unit, not flicker.
    deliver_child_batch(
        &mut app,
        &parents,
        vec![("KAN-1", vec![child("KAN-2", "KAN-1")])],
    );
    let rows = app.visible_issue_rows();
    assert_eq!(rows.len(), 2, "subtree stays visible while held");
    assert!(
        rows[0].loading,
        "node stays dimmed until the root query lands"
    );

    // The roots land: the held children apply and everything settles together.
    app.handle_event(AppEvent::JiraProjectLoaded {
        request_id,
        purpose: JiraLoadPurpose::Reload,
        credentials: credentials(),
        result: JiraProjectLoadResult {
            issues: Ok(vec![issue_with_children("KAN-1", "Epic", "Epic", true)]),
            board: Err(JiraError(String::from("board unavailable"))),
            next_page_token: None,
            fields: Ok(Vec::new()),
            projects: Ok(Vec::new()),
            users: Ok(Vec::new()),
            current_user: Err(JiraError(String::new())),
            logs: Vec::new(),
        },
    });
    let rows = app.visible_issue_rows();
    assert_eq!(rows.len(), 2, "subtree intact after the reload");
    assert!(!rows[0].loading, "node settles once the reload completes");
    assert!(!rows[1].reloading, "child no longer greyed");
}

#[test]
fn node_reload_restores_expanded_grandchildren() {
    let bindings = KeyBindings::default();
    let mut app = loaded_app(vec![issue_with_children("KAN-1", "Epic", "Epic", true)]);
    expand_and_load(
        &mut app,
        "KAN-1",
        vec![parent_child("KAN-2", "KAN-1")],
        &bindings,
    );
    // Expand the nested node KAN-2.
    app.handle_key(key('j'), &bindings);
    app.handle_key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        &bindings,
    );
    deliver_children(&mut app, "KAN-2", vec![child("KAN-3", "KAN-2")]);
    assert_eq!(app.visible_issue_rows().len(), 3);

    // Reload KAN-1 (select it first); its subtree (including the expanded
    // grandchild KAN-2) refreshes in place, in parallel.
    app.handle_key(key('k'), &bindings);
    assert_eq!(app.selected_issue_key(), Some("KAN-1"));
    app.handle_key(key('r'), &bindings);

    // KAN-1 and KAN-2 refetch together in one batch; deliver both.
    let parents = take_child_batch(&mut app);
    let names: std::collections::HashSet<&str> =
        parents.iter().map(|(key, _)| key.as_str()).collect();
    assert!(
        names.contains("KAN-1") && names.contains("KAN-2"),
        "node reload refreshes the open subtree in one batch"
    );
    deliver_child_batch(
        &mut app,
        &parents,
        vec![
            ("KAN-1", vec![parent_child("KAN-2", "KAN-1")]),
            ("KAN-2", vec![child("KAN-3", "KAN-2")]),
        ],
    );

    let ids: Vec<&str> = app
        .visible_issue_rows()
        .iter()
        .map(|row| app.issues()[row.item_index].id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec!["KAN-1", "KAN-2", "KAN-3"],
        "grandchild subtree restored"
    );
}

/// Drains the pending full-reload effect and delivers `issues` as the result.
fn deliver_full_reload(app: &mut App, issues: Vec<IssueSummary>) {
    let request_id = app
        .take_effects()
        .into_iter()
        .find_map(|effect| match effect {
            AppEffect::LoadJiraProject { request_id, .. } => Some(request_id),
            _ => None,
        })
        .expect("expected full reload effect");
    app.handle_event(AppEvent::JiraProjectLoaded {
        request_id,
        purpose: JiraLoadPurpose::Reload,
        credentials: credentials(),
        result: JiraProjectLoadResult {
            issues: Ok(issues),
            board: Err(JiraError(String::from("board unavailable"))),
            next_page_token: None,
            fields: Ok(Vec::new()),
            projects: Ok(Vec::new()),
            users: Ok(Vec::new()),
            current_user: Err(JiraError(String::new())),
            logs: Vec::new(),
        },
    });
}

#[test]
fn full_reload_preserves_selection_and_recenters() {
    let bindings = KeyBindings::default();
    let roots: Vec<IssueSummary> = (1..=30)
        .map(|n| issue_with_children(&format!("KAN-{n}"), "Item", "Task", false))
        .collect();
    let mut app = loaded_app(roots.clone());

    // Establish a viewport and navigate down into the list.
    let _ = app.visible_issue_range(15);
    for _ in 0..20 {
        app.handle_key(key('j'), &bindings);
    }
    let _ = app.visible_issue_range(15);
    assert_eq!(app.selected_issue_key(), Some("KAN-21"));
    assert!(
        app.issue_scroll_offset() > 0,
        "selection is centered, not at the top"
    );

    // Full reload with the same data.
    app.handle_key(
        KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT),
        &bindings,
    );
    deliver_full_reload(&mut app, roots);
    let _ = app.visible_issue_range(15);

    // Selection (a root) is preserved and re-centered in the viewport.
    assert_eq!(
        app.selected_issue_key(),
        Some("KAN-21"),
        "selection preserved across reload"
    );
    let rows = app.visible_issue_rows();
    let selected_row = rows
        .iter()
        .position(|r| app.issues()[r.item_index].id == "KAN-21")
        .expect("selection present");
    // The settled scroll target centers the selection (selection minus roughly
    // half the viewport). The on-screen window glides to this across ticks.
    assert_eq!(app.issue_scroll_offset(), selected_row.saturating_sub(7));
}

#[test]
fn full_reload_keeps_child_selection_in_place() {
    let bindings = KeyBindings::default();
    let mut app = loaded_app(vec![
        issue_with_children("KAN-1", "Epic", "Epic", true),
        issue_with_children("KAN-2", "Other", "Task", false),
    ]);
    // Open KAN-1, select its child.
    expand_and_load(&mut app, "KAN-1", vec![child("KAN-3", "KAN-1")], &bindings);
    app.handle_key(key('j'), &bindings);
    assert_eq!(app.selected_issue_key(), Some("KAN-3"));

    // Reload the whole list from the nested child.
    app.handle_key(
        KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT),
        &bindings,
    );
    deliver_full_reload(
        &mut app,
        vec![
            issue_with_children("KAN-1", "Epic", "Epic", true),
            issue_with_children("KAN-2", "Other", "Task", false),
        ],
    );

    // The seamless reload keeps the expanded subtree in place, so the child
    // selection is preserved instead of jumping up to the root ancestor.
    assert_eq!(app.selected_issue_key(), Some("KAN-3"));
}

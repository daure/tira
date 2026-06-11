mod support;

use ratatui::{Terminal, backend::TestBackend};
use tira::{
    App, AppEffect, AppEvent, KeyBindings, draw,
    domain::date::{civil_from_days, today_days},
    services::jira::{
        SprintState, TimelineData, TimelineEpic, TimelineEpicStats, TimelineIssue, TimelineSprint,
    },
};

use support::{ctrl, key, project, rendered_text};

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Builds an app on the Timeline tab with a load queued but not yet delivered,
/// returning the request id the load was issued under.
fn timeline_app_pending() -> (App, u64) {
    let mut app = App::with_issues_projects_and_users(
        Vec::new(),
        vec![project("DPP", "DICE")],
        Vec::new(),
        "DPP",
    );
    let bindings = KeyBindings::default();

    // Leader + t switches to the Timeline tab, which lazily queues the load.
    app.handle_key(ctrl('x'), &bindings);
    app.handle_key(key('t'), &bindings);

    let request_id = app
        .take_effects()
        .into_iter()
        .find_map(|effect| match effect {
            AppEffect::LoadTimeline { request_id, .. } => Some(request_id),
            _ => None,
        })
        .expect("timeline load was queued on tab switch");
    (app, request_id)
}

/// Builds an app on the Timeline tab with one epic loaded via the real
/// load/event path, plus the request id the load was issued under.
fn timeline_app() -> (App, Terminal<TestBackend>) {
    let (mut app, request_id) = timeline_app_pending();

    let today = today_days();
    let data = TimelineData {
        epics: vec![TimelineEpic {
            key: String::from("DPP-1"),
            summary: String::from("Checkout revamp"),
            status: String::from("In Progress"),
            done: false,
            stats: TimelineEpicStats {
                to_do: 1,
                in_progress: 1,
                done: 2,
            },
            children: vec![TimelineIssue {
                key: String::from("DPP-2"),
                summary: String::from("Shopping cart"),
                status: String::from("In Progress"),
                issue_type: String::from("Story"),
                done: false,
                sprint_ids: vec![196],
            }],
        }],
        sprints: vec![TimelineSprint {
            id: 196,
            name: String::from("DICE Sprint 196"),
            state: SprintState::Active,
            start_day: Some(today - 7),
            end_day: Some(today + 7),
        }],
    };
    app.handle_event(AppEvent::TimelineLoaded {
        request_id,
        timeline: Ok(data),
        logs: Vec::new(),
    });

    let terminal = Terminal::new(TestBackend::new(160, 20)).expect("test terminal");
    (app, terminal)
}

#[test]
fn timeline_renders_epic_row_sprint_pill_today_marker_and_current_month() {
    let (app, mut terminal) = timeline_app();
    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw timeline");
    let (screen, _) = rendered_text(&terminal);

    assert!(screen.contains("DPP-1"), "epic key shown: {screen:?}");
    assert!(screen.contains("Checkout revamp"), "epic summary shown");
    // Numbered sprint renders with the short S<n> label, not the full name.
    assert!(screen.contains("S196"), "sprint short label shown");
    assert!(screen.contains('▼'), "today triangle shown");
    // The proportional month header renders neighbouring months around today.
    // (The current month's own label can sit under the today triangle, so
    // assert on the adjacent months, which never are.)
    let (_, month, _) = civil_from_days(today_days());
    let prev = MONTHS[((month + 10) % 12) as usize];
    let next = MONTHS[(month % 12) as usize];
    assert!(
        screen.contains(prev) || screen.contains(next),
        "neighbouring month ({prev} or {next}) in header"
    );
    // The active sprint straddles today, so the epic draws a bar.
    assert!(screen.contains('█'), "epic bar drawn");
    // Children are hidden until the epic is expanded.
    assert!(!screen.contains("DPP-2"), "child hidden while collapsed");
}

#[test]
fn expanding_epic_reveals_child_rows() {
    let (mut app, mut terminal) = timeline_app();
    let bindings = KeyBindings::default();

    // `l` expands the selected epic (tree semantics), revealing its children.
    app.handle_key(key('l'), &bindings);

    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw timeline");
    let (screen, _) = rendered_text(&terminal);

    assert!(screen.contains("DPP-2"), "child shown after expand: {screen:?}");
    assert!(screen.contains("Shopping cart"), "child summary shown");
}

#[test]
fn stale_timeline_response_is_dropped() {
    let (mut app, request_id) = timeline_app_pending();

    // A response under a request id that is not the pending one (e.g. a
    // superseded load) must be ignored entirely.
    let today = today_days();
    let stale = TimelineData {
        epics: vec![TimelineEpic {
            key: String::from("STALE-9"),
            summary: String::from("Should never render"),
            status: String::from("To Do"),
            done: false,
            stats: TimelineEpicStats::default(),
            children: Vec::new(),
        }],
        sprints: vec![TimelineSprint {
            id: 1,
            name: String::from("DICE Sprint 1"),
            state: SprintState::Active,
            start_day: Some(today - 7),
            end_day: Some(today + 7),
        }],
    };
    app.handle_event(AppEvent::TimelineLoaded {
        request_id: request_id + 1000,
        timeline: Ok(stale),
        logs: Vec::new(),
    });

    // The stale payload was discarded: nothing loaded, still awaiting the real
    // response.
    assert!(app.timeline().data().is_none(), "stale data not stored");
    assert!(app.timeline().is_loading(), "still awaiting pending load");

    let mut terminal = Terminal::new(TestBackend::new(160, 20)).expect("test terminal");
    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw timeline");
    let (screen, _) = rendered_text(&terminal);
    assert!(!screen.contains("STALE-9"), "stale epic not rendered: {screen:?}");
}
